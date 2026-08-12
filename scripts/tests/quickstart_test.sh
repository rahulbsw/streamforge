#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly QUICKSTART="${REPO_ROOT}/scripts/quickstart.sh"
readonly TEST_ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT

fail() {
  printf 'quickstart_test: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local expected="$2"
  grep -F -- "${expected}" "${file}" >/dev/null ||
    fail "expected '${expected}' in ${file}"
}

make_fake_runtime() {
  local runtime="$1"
  local bin_dir="$2"
  mkdir -p "${bin_dir}"
  cp "${TEST_ROOT}/fake-runtime" "${bin_dir}/${runtime}"
  chmod +x "${bin_dir}/${runtime}"
}

cat >"${TEST_ROOT}/fake-runtime" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail

printf '%q ' "$@" >>"${FAKE_RUNTIME_LOG}"
printf '\n' >>"${FAKE_RUNTIME_LOG}"

command_name="${1:-}"
shift || true

case "${command_name}" in
  info)
    exit 0
    ;;
  container)
    [[ "${1:-}" == "inspect" ]] || exit 1
    name="${@: -1}"
    if [[ -f "${FAKE_RUNTIME_STATE}/container-${name}" ]]; then
      if [[ "$*" == *"--format"* ]]; then
        printf 'true\n'
      fi
      exit 0
    fi
    exit 1
    ;;
  network)
    operation="${1:-}"
    name="${2:-}"
    case "${operation}" in
      inspect) [[ -f "${FAKE_RUNTIME_STATE}/network-${name}" ]] ;;
      create) touch "${FAKE_RUNTIME_STATE}/network-${name}" ;;
      rm) rm -f "${FAKE_RUNTIME_STATE}/network-${name}" ;;
      *) exit 1 ;;
    esac
    ;;
  run)
    name=""
    while [[ $# -gt 0 ]]; do
      if [[ "$1" == "--name" ]]; then
        name="$2"
        shift 2
        continue
      fi
      shift
    done
    [[ -n "${name}" ]] || exit 1
    touch "${FAKE_RUNTIME_STATE}/container-${name}"
    ;;
  exec)
    if [[ "${1:-}" == "--interactive" ]]; then
      shift
    fi
    container_name="${1:-}"
    shift || true
    if [[ "$*" == *"cluster health"* && "${FAKE_BROKER_FAILURE:-0}" == "1" ]]; then
      exit 1
    elif [[ "$*" == *"topic produce"* ]]; then
      cat >"${FAKE_RUNTIME_STATE}/payload"
    elif [[ "$*" == *"topic consume"* ]]; then
      cat "${FAKE_RUNTIME_STATE}/payload"
    fi
    ;;
  rm)
    if [[ "${1:-}" == "--force" ]]; then
      shift
    fi
    rm -f "${FAKE_RUNTIME_STATE}/container-${1}"
    ;;
  logs)
    exit 0
    ;;
  *)
    exit 1
    ;;
esac
EOF
chmod +x "${TEST_ROOT}/fake-runtime"

for runtime in docker podman; do
  case_root="${TEST_ROOT}/${runtime}"
  bin_dir="${case_root}/bin"
  runtime_state="${case_root}/runtime"
  quickstart_state="${case_root}/quickstart"
  runtime_log="${case_root}/runtime.log"
  mkdir -p "${runtime_state}"
  : >"${runtime_log}"
  make_fake_runtime "${runtime}" "${bin_dir}"

  export PATH="${bin_dir}:${PATH}"
  export FAKE_RUNTIME_LOG="${runtime_log}"
  export FAKE_RUNTIME_STATE="${runtime_state}"
  export STREAMFORGE_CONTAINER_RUNTIME="${runtime}"
  export STREAMFORGE_QUICKSTART_PREFIX="test-${runtime}"
  export STREAMFORGE_QUICKSTART_STATE_DIR="${quickstart_state}"
  export STREAMFORGE_QUICKSTART_READY_ATTEMPTS=1
  export STREAMFORGE_QUICKSTART_ENGINE_WAIT_SECONDS=0
  export STREAMFORGE_QUICKSTART_VERIFY_ATTEMPTS=1

  "${QUICKSTART}" up >/dev/null
  [[ -f "${quickstart_state}/streamforge.yaml" ]] ||
    fail "${runtime}: generated config is missing"
  assert_contains "${quickstart_state}/streamforge.yaml" "bootstrap: redpanda:9092"
  assert_contains "${runtime_log}" "ghcr.io/rahulbsw/streamforge:1.1.0"
  assert_contains "${runtime_log}" "rpk topic create quickstart-input quickstart-output"
  if [[ "${runtime}" == "podman" ]]; then
    assert_contains "${runtime_log}" ":ro\\,Z"
  fi

  verify_output="$("${QUICKSTART}" verify)"
  [[ "${verify_output}" == *"Verified replication"* ]] ||
    fail "${runtime}: verification did not succeed"

  "${QUICKSTART}" down >/dev/null
  [[ ! -e "${quickstart_state}/streamforge.yaml" ]] ||
    fail "${runtime}: generated config was not removed"
  [[ ! -e "${runtime_state}/container-test-${runtime}-engine" ]] ||
    fail "${runtime}: engine container was not removed"
  [[ ! -e "${runtime_state}/container-test-${runtime}-redpanda" ]] ||
    fail "${runtime}: broker container was not removed"
  [[ ! -e "${runtime_state}/network-test-${runtime}-network" ]] ||
    fail "${runtime}: network was not removed"
done

# A failed startup must clean up everything created by that invocation.
failure_root="${TEST_ROOT}/failure"
mkdir -p "${failure_root}/runtime"
: >"${failure_root}/runtime.log"
make_fake_runtime docker "${failure_root}/bin"
export PATH="${failure_root}/bin:${PATH}"
export FAKE_RUNTIME_LOG="${failure_root}/runtime.log"
export FAKE_RUNTIME_STATE="${failure_root}/runtime"
export FAKE_BROKER_FAILURE=1
export STREAMFORGE_CONTAINER_RUNTIME=docker
export STREAMFORGE_QUICKSTART_PREFIX=test-failure
export STREAMFORGE_QUICKSTART_STATE_DIR="${failure_root}/quickstart"
export STREAMFORGE_QUICKSTART_READY_ATTEMPTS=1
if "${QUICKSTART}" up >/dev/null 2>&1; then
  fail "failed broker readiness unexpectedly succeeded"
fi
[[ ! -e "${failure_root}/runtime/container-test-failure-redpanda" ]] ||
  fail "failed startup left the broker container behind"
[[ ! -e "${failure_root}/runtime/network-test-failure-network" ]] ||
  fail "failed startup left the network behind"
[[ ! -e "${failure_root}/quickstart/streamforge.yaml" ]] ||
  fail "failed startup left generated configuration behind"

printf 'quickstart tests passed for Docker and Podman\n'
