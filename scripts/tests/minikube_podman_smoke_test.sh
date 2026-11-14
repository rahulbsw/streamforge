#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly SMOKE="${REPO_ROOT}/scripts/tests/minikube_podman_smoke.sh"
readonly SMOKE_LIB="${REPO_ROOT}/scripts/tests/lib/minikube_podman_smoke_lib.sh"
readonly KAFKA_MANIFEST="${REPO_ROOT}/examples/kubernetes/kafka/kafka-standalone.yaml"
readonly TEST_ROOT="$(mktemp -d)"

cleanup() {
  rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT

fail() {
  printf 'minikube_podman_smoke_test: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1"
  local expected="$2"
  grep -F -- "${expected}" "${file}" >/dev/null ||
    fail "expected '${expected}' in ${file}"
}

bash -n "${SMOKE}"
bash -n "${SMOKE_LIB}"
assert_contains "${KAFKA_MANIFEST}" \
  'value: "PLAINTEXT://:9092,CONTROLLER://:9093"'
assert_contains "${KAFKA_MANIFEST}" \
  'name: KAFKA_INTER_BROKER_LISTENER_NAME'
if grep -F 'PLAINTEXT://0.0.0.0' "${KAFKA_MANIFEST}" >/dev/null; then
  fail "Kafka listeners must not advertise or bind a 0.0.0.0 meta-address"
fi

mkdir -p "${TEST_ROOT}/bin"
cat >"${TEST_ROOT}/bin/minikube" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ -n "${FAKE_MINIKUBE_LOG:-}" ]]; then
  printf '%q ' "$@" >>"${FAKE_MINIKUBE_LOG}"
  printf '\n' >>"${FAKE_MINIKUBE_LOG}"
fi
if [[ "${1:-}" == "version" && "${2:-}" == "--short" ]]; then
  printf '%s\n' "${FAKE_MINIKUBE_VERSION}"
  exit 0
fi
if [[ "${1:-}" == "profile" && "${2:-}" == "list" ]]; then
  if [[ -n "${FAKE_EXISTING_PROFILE:-}" ]]; then
    printf '{"invalid":[],"valid":[{"Name":"%s"}]}\n' \
      "${FAKE_EXISTING_PROFILE}"
  else
    printf '{"invalid":[],"valid":[]}\n'
  fi
  exit 0
fi
if [[ "${1:-}" == "start" && "${FAKE_MINIKUBE_START_FAILURE:-0}" == "1" ]]; then
  exit 23
fi
if [[ "${1:-}" == "delete" ]]; then
  exit 0
fi
exit 1
EOF
cat >"${TEST_ROOT}/bin/podman" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${1:-}" == "info" && "${2:-}" == "--format" ]]; then
  printf 'true\n'
fi
exit 0
EOF
cat >"${TEST_ROOT}/bin/kubectl" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "${1:-}" == "version" && "${2:-}" == "--client" ]]
EOF
cat >"${TEST_ROOT}/bin/helm" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "${1:-}" == "version" && "${2:-}" == "--short" ]]
EOF
cat >"${TEST_ROOT}/bin/curl" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "${1:-}" == "--version" ]]
EOF
chmod +x "${TEST_ROOT}/bin/"*

export PATH="${TEST_ROOT}/bin:${PATH}"
export FAKE_MINIKUBE_VERSION="v1.38.1"
"${SMOKE}" --check-prerequisites >/dev/null

export FAKE_MINIKUBE_VERSION="v1.37.0"
if "${SMOKE}" --check-prerequisites >"${TEST_ROOT}/old-version.out" 2>&1; then
  fail "unsupported Minikube version unexpectedly passed"
fi
assert_contains "${TEST_ROOT}/old-version.out" \
  "Minikube 1.38.1 or newer is required"

export FAKE_MINIKUBE_VERSION="v1.38.1"
export FAKE_MINIKUBE_LOG="${TEST_ROOT}/minikube.log"
export STREAMFORGE_SMOKE_PROFILE="preexisting-profile"
export FAKE_EXISTING_PROFILE="${STREAMFORGE_SMOKE_PROFILE}"
: >"${FAKE_MINIKUBE_LOG}"
if "${SMOKE}" >"${TEST_ROOT}/existing-profile.out" 2>&1; then
  fail "an existing profile was unexpectedly reused"
fi
assert_contains "${TEST_ROOT}/existing-profile.out" \
  "refusing to reuse existing Minikube profile: preexisting-profile"
if grep -F 'delete --profile preexisting-profile' \
  "${FAKE_MINIKUBE_LOG}" >/dev/null; then
  fail "the existing profile was deleted"
fi

unset FAKE_EXISTING_PROFILE
export FAKE_MINIKUBE_START_FAILURE=1
export STREAMFORGE_SMOKE_PROFILE="new-partial-profile"
: >"${FAKE_MINIKUBE_LOG}"
if "${SMOKE}" >"${TEST_ROOT}/failed-start.out" 2>&1; then
  fail "the simulated failed Minikube start unexpectedly passed"
fi
assert_contains "${FAKE_MINIKUBE_LOG}" \
  "start --profile new-partial-profile"
assert_contains "${FAKE_MINIKUBE_LOG}" \
  "delete --profile new-partial-profile"

printf 'Minikube Podman smoke static tests passed\n'
