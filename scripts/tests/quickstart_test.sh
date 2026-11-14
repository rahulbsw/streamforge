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
    case "${operation}" in
      inspect)
        name="${@: -1}"
        if [[ "$*" == *"--format"* ]]; then
          [[ -f "${FAKE_RUNTIME_STATE}/network-${name}" ]] || exit 1
          if [[ "$*" == *".Labels"* ]]; then
            [[ -f "${FAKE_RUNTIME_STATE}/network-label-${name}" ]] || printf '<no value>\n'
            [[ ! -f "${FAKE_RUNTIME_STATE}/network-label-${name}" ]] ||
              cat "${FAKE_RUNTIME_STATE}/network-label-${name}"
          else
            if [[ "${FAKE_NETWORK_ID_INSPECT_FAILURES:-0}" -gt 0 ]]; then
              inspect_count=0
              [[ ! -f "${FAKE_RUNTIME_STATE}/network-id-inspect-count" ]] ||
                inspect_count="$(cat "${FAKE_RUNTIME_STATE}/network-id-inspect-count")"
              inspect_count=$((inspect_count + 1))
              printf '%s\n' "${inspect_count}" >"${FAKE_RUNTIME_STATE}/network-id-inspect-count"
              if [[ "${inspect_count}" -le "${FAKE_NETWORK_ID_INSPECT_FAILURES}" ]]; then
                exit 1
              fi
            fi
            cat "${FAKE_RUNTIME_STATE}/network-${name}"
          fi
        else
          [[ -f "${FAKE_RUNTIME_STATE}/network-${name}" ]]
        fi
        ;;
      create)
        name="${@: -1}"
        label="${3#*=}"
        printf '0123456789abcdef01234567\n' >"${FAKE_RUNTIME_STATE}/network-${name}"
        printf '%s\n' "${label}" >"${FAKE_RUNTIME_STATE}/network-label-${name}"
        if [[ "${0##*/}" == "podman" ]]; then
          printf '%s\n' "${name}"
        else
          printf '0123456789abcdef01234567\n'
        fi
        [[ "${FAKE_NETWORK_CREATE_FAILURE_AFTER_CREATE:-0}" != "1" ]] || exit 1
        ;;
      rm)
        [[ "${FAKE_NETWORK_REMOVE_FAILURE:-0}" != "1" ]] || exit 1
        target="${2:-}"
        name="${target}"
        if [[ -f "${FAKE_RUNTIME_STATE}/network-${name}" ]]; then
          rm -f "${FAKE_RUNTIME_STATE}/network-${name}"
          rm -f "${FAKE_RUNTIME_STATE}/network-label-${name}"
        else
          for network_file in "${FAKE_RUNTIME_STATE}"/network-*; do
            [[ "${network_file}" == *"network-label-"* ]] && continue
            [[ -f "${network_file}" ]] || continue
            if [[ "$(cat "${network_file}")" == "${name}" ]]; then
              network_name="${network_file##*/network-}"
              rm -f "${network_file}"
              rm -f "${FAKE_RUNTIME_STATE}/network-label-${network_name}"
            fi
          done
        fi
        ;;
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

# A runtime may create the network and still return an error. The ownership
# token is written first so the failure trap can safely identify and clean it.
for runtime in docker podman; do
  create_failure_root="${TEST_ROOT}/create-failure-${runtime}"
  mkdir -p "${create_failure_root}/runtime"
  : >"${create_failure_root}/runtime.log"
  make_fake_runtime "${runtime}" "${create_failure_root}/bin"
  export PATH="${create_failure_root}/bin:${PATH}"
  export FAKE_RUNTIME_LOG="${create_failure_root}/runtime.log"
  export FAKE_RUNTIME_STATE="${create_failure_root}/runtime"
  export STREAMFORGE_CONTAINER_RUNTIME="${runtime}"
  export STREAMFORGE_QUICKSTART_PREFIX="test-create-failure-${runtime}"
  export STREAMFORGE_QUICKSTART_STATE_DIR="${create_failure_root}/quickstart"
  export FAKE_NETWORK_CREATE_FAILURE_AFTER_CREATE=1
  if "${QUICKSTART}" up >/dev/null 2>&1; then
    fail "${runtime}: up unexpectedly succeeded after network create returned an error"
  fi
  [[ ! -e "${create_failure_root}/runtime/network-test-create-failure-${runtime}-network" ]] ||
    fail "${runtime}: failed network create left the created network behind"
  unset FAKE_NETWORK_CREATE_FAILURE_AFTER_CREATE
done

# If post-create inspection fails, the ownership record must still let EXIT
# cleanup recover and remove the network created by this invocation.
for runtime in docker podman; do
  inspect_failure_root="${TEST_ROOT}/inspect-failure-${runtime}"
  mkdir -p "${inspect_failure_root}/runtime"
  : >"${inspect_failure_root}/runtime.log"
  make_fake_runtime "${runtime}" "${inspect_failure_root}/bin"
  export PATH="${inspect_failure_root}/bin:${PATH}"
  export FAKE_RUNTIME_LOG="${inspect_failure_root}/runtime.log"
  export FAKE_RUNTIME_STATE="${inspect_failure_root}/runtime"
  export STREAMFORGE_CONTAINER_RUNTIME="${runtime}"
  export STREAMFORGE_QUICKSTART_PREFIX="test-inspect-failure-${runtime}"
  export STREAMFORGE_QUICKSTART_STATE_DIR="${inspect_failure_root}/quickstart"
  export FAKE_NETWORK_ID_INSPECT_FAILURES=3
  if "${QUICKSTART}" up >/dev/null 2>&1; then
    fail "${runtime}: up unexpectedly passed after network ID inspection failure"
  fi
  [[ ! -e "${inspect_failure_root}/runtime/network-test-inspect-failure-${runtime}-network" ]] ||
    fail "${runtime}: failed inspection left the created network behind"
  [[ ! -e "${inspect_failure_root}/quickstart/.network-created-by-quickstart" ]] ||
    fail "${runtime}: successful failure cleanup retained the ownership record"
  unset FAKE_NETWORK_ID_INSPECT_FAILURES
done

# An unrelated network with the configured name must not be adopted or
# removed by a failed up/down cleanup cycle.
existing_root="${TEST_ROOT}/existing-network"
mkdir -p "${existing_root}/runtime" "${existing_root}/quickstart"
: >"${existing_root}/runtime.log"
make_fake_runtime docker "${existing_root}/bin"
printf 'unrelated-network-id\n' >"${existing_root}/runtime/network-test-existing-network-network"
export PATH="${existing_root}/bin:${PATH}"
export FAKE_RUNTIME_LOG="${existing_root}/runtime.log"
export FAKE_RUNTIME_STATE="${existing_root}/runtime"
export STREAMFORGE_CONTAINER_RUNTIME=docker
export STREAMFORGE_QUICKSTART_PREFIX=test-existing-network
export STREAMFORGE_QUICKSTART_STATE_DIR="${existing_root}/quickstart"
if "${QUICKSTART}" up >/dev/null 2>&1; then
  fail "up unexpectedly reused a pre-existing network"
fi
[[ -f "${existing_root}/runtime/network-test-existing-network-network" ]] ||
  fail "failed up removed the pre-existing network"
if grep -F 'network rm test-existing-network-network' "${existing_root}/runtime.log" >/dev/null; then
  fail "failed up attempted to remove the pre-existing network"
fi
"${QUICKSTART}" down >/dev/null
[[ -f "${existing_root}/runtime/network-test-existing-network-network" ]] ||
  fail "down removed a network not owned by quickstart"

# If an owned network is replaced under the same name, cleanup must preserve
# the replacement instead of trusting the old ownership marker.
replacement_root="${TEST_ROOT}/replacement-network"
mkdir -p "${replacement_root}/runtime"
: >"${replacement_root}/runtime.log"
make_fake_runtime docker "${replacement_root}/bin"
export PATH="${replacement_root}/bin:${PATH}"
export FAKE_RUNTIME_LOG="${replacement_root}/runtime.log"
export FAKE_RUNTIME_STATE="${replacement_root}/runtime"
export STREAMFORGE_CONTAINER_RUNTIME=docker
export STREAMFORGE_QUICKSTART_PREFIX=test-replacement-network
export STREAMFORGE_QUICKSTART_STATE_DIR="${replacement_root}/quickstart"
unset FAKE_BROKER_FAILURE
"${QUICKSTART}" up >/dev/null
printf 'replacement-network-id\n' >"${replacement_root}/runtime/network-test-replacement-network-network"
rm -f "${replacement_root}/runtime/network-label-test-replacement-network-network"
"${QUICKSTART}" down >/dev/null 2>&1
[[ -f "${replacement_root}/runtime/network-test-replacement-network-network" ]] ||
  fail "down removed a same-name replacement network"

# A transient removal failure must keep the ownership ID so cleanup can retry.
retry_root="${TEST_ROOT}/network-removal-retry"
mkdir -p "${retry_root}/runtime"
: >"${retry_root}/runtime.log"
make_fake_runtime docker "${retry_root}/bin"
export PATH="${retry_root}/bin:${PATH}"
export FAKE_RUNTIME_LOG="${retry_root}/runtime.log"
export FAKE_RUNTIME_STATE="${retry_root}/runtime"
export STREAMFORGE_CONTAINER_RUNTIME=docker
export STREAMFORGE_QUICKSTART_PREFIX=test-network-removal-retry
export STREAMFORGE_QUICKSTART_STATE_DIR="${retry_root}/quickstart"
"${QUICKSTART}" up >/dev/null
export FAKE_NETWORK_REMOVE_FAILURE=1
if "${QUICKSTART}" down >/dev/null 2>&1; then
  fail "down unexpectedly succeeded after network removal failure"
fi
[[ -f "${retry_root}/quickstart/.network-created-by-quickstart" ]] ||
  fail "failed network removal discarded the ownership ID"
unset FAKE_NETWORK_REMOVE_FAILURE
"${QUICKSTART}" down >/dev/null
[[ ! -e "${retry_root}/runtime/network-test-network-removal-retry-network" ]] ||
  fail "retry did not remove the owned network"

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
