#!/usr/bin/env bash
set -Eeuo pipefail

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly MINIMUM_MINIKUBE_VERSION="1.38.1"
readonly CHART_VERSION="$(
  awk '/^appVersion:/ {gsub(/["[:space:]]/, "", $2); print $2; exit}' \
    "${REPO_ROOT}/helm/streamforge-operator/Chart.yaml"
)"
readonly MINIKUBE_BIN="${MINIKUBE_BIN:-minikube}"
readonly PODMAN_BIN="${PODMAN_BIN:-podman}"
readonly KUBECTL_BIN="${KUBECTL_BIN:-kubectl}"
readonly HELM_BIN="${HELM_BIN:-helm}"
readonly CURL_BIN="${CURL_BIN:-curl}"
readonly PROFILE="${STREAMFORGE_SMOKE_PROFILE:-streamforge-podman-$$}"
readonly OPERATOR_NAMESPACE="streamforge-system"
readonly KAFKA_NAMESPACE="kafka"
readonly RELEASE="${STREAMFORGE_SMOKE_RELEASE:-streamforge-smoke}"
readonly PIPELINE_NAME="simple-mirror"
readonly KUBERNETES_VERSION="${STREAMFORGE_SMOKE_KUBERNETES_VERSION:-v1.31.0}"
readonly LOCAL_METRICS_PORT="${STREAMFORGE_SMOKE_METRICS_PORT:-19090}"
readonly MAX_RECONCILES="${STREAMFORGE_SMOKE_MAX_RECONCILES:-5}"
readonly RUN_ID="$(date -u +%Y%m%d%H%M%S)-$$"
readonly ENGINE_IMAGE="localhost/streamforge-smoke:${RUN_ID}"
readonly OPERATOR_IMAGE="localhost/streamforge-operator-smoke:${RUN_ID}"
readonly KAFKA_BOOTSTRAP="kafka.${KAFKA_NAMESPACE}.svc.cluster.local.:9092"
readonly KAFKA_MANIFEST="${REPO_ROOT}/examples/kubernetes/kafka/kafka-standalone.yaml"
readonly PIPELINE_MANIFEST="${REPO_ROOT}/examples/pipelines/simple-mirror.yaml"
readonly CHART="${REPO_ROOT}/helm/streamforge-operator"

TEMP_DIR=""
ORIGINAL_CONTEXT=""
PROFILE_CREATED=0
ENGINE_IMAGE_CREATED=0
OPERATOR_IMAGE_CREATED=0
HELM_INSTALLED=0
PORT_FORWARD_PID=""

usage() {
  cat <<'EOF'
Usage: scripts/tests/minikube_podman_smoke.sh [--check-prerequisites]

Runs an isolated, destructive-to-its-own-profile-only integration test using
rootless Podman, Minikube, containerd, and bridge CNI. The default behavior
builds local engine/operator images, deploys Kafka and StreamForge, validates
one exact replicated record and the operational contracts, then removes every
resource it created.

Options:
  --check-prerequisites  Validate local tools and versions without mutation.
  -h, --help             Show this help.

Environment:
  MINIKUBE_BIN, PODMAN_BIN, KUBECTL_BIN, HELM_BIN, CURL_BIN
  STREAMFORGE_SMOKE_PROFILE              New profile name; must not exist.
  STREAMFORGE_SMOKE_KUBERNETES_VERSION   Default: v1.31.0.
  STREAMFORGE_SMOKE_METRICS_PORT         Default: 19090.
  STREAMFORGE_SMOKE_MAX_RECONCILES       Max reconciles in 30s; default: 5.
  STREAMFORGE_SMOKE_KEEP_CLUSTER=1       Keep the new profile for debugging.
EOF
}

log() {
  printf 'minikube_podman_smoke: %s\n' "$*"
}

die() {
  printf 'minikube_podman_smoke: ERROR: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 ||
    die "required command is unavailable: $1"
}

version_at_least() {
  local actual="${1#v}"
  local required="${2#v}"
  local actual_major actual_minor actual_patch
  local required_major required_minor required_patch

  actual="${actual%%-*}"
  required="${required%%-*}"
  IFS=. read -r actual_major actual_minor actual_patch <<EOF
${actual}
EOF
  IFS=. read -r required_major required_minor required_patch <<EOF
${required}
EOF
  actual_major="${actual_major:-0}"
  actual_minor="${actual_minor:-0}"
  actual_patch="${actual_patch:-0}"
  required_major="${required_major:-0}"
  required_minor="${required_minor:-0}"
  required_patch="${required_patch:-0}"

  if ((actual_major != required_major)); then
    ((actual_major > required_major))
  elif ((actual_minor != required_minor)); then
    ((actual_minor > required_minor))
  else
    ((actual_patch >= required_patch))
  fi
}

check_prerequisites() {
  local minikube_version rootless

  require_command "${MINIKUBE_BIN}"
  require_command "${PODMAN_BIN}"
  require_command "${KUBECTL_BIN}"
  require_command "${HELM_BIN}"
  require_command "${CURL_BIN}"
  require_command awk
  require_command grep
  require_command sed
  require_command mktemp
  require_command tr

  minikube_version="$("${MINIKUBE_BIN}" version --short 2>/dev/null)" ||
    die "unable to read Minikube version"
  version_at_least "${minikube_version}" "${MINIMUM_MINIKUBE_VERSION}" ||
    die "Minikube ${MINIMUM_MINIKUBE_VERSION} or newer is required; found ${minikube_version}"

  "${PODMAN_BIN}" info >/dev/null 2>&1 ||
    die "Podman is unavailable; start the Podman machine first"
  rootless="$("${PODMAN_BIN}" info --format '{{.Host.Security.Rootless}}' 2>/dev/null)" ||
    die "unable to determine whether Podman is rootless"
  [[ "${rootless}" == "true" ]] ||
    die "this smoke test requires rootless Podman; reported rootless=${rootless}"

  "${KUBECTL_BIN}" version --client >/dev/null 2>&1 ||
    die "kubectl client is unavailable"
  "${HELM_BIN}" version --short >/dev/null 2>&1 ||
    die "Helm is unavailable"
  "${CURL_BIN}" --version >/dev/null 2>&1 ||
    die "curl is unavailable"
  [[ -n "${CHART_VERSION}" ]] || die "Helm chart appVersion is empty"

  log "prerequisites passed (Minikube ${minikube_version}, rootless Podman)"
}

profile_exists() {
  local profiles
  profiles="$("${MINIKUBE_BIN}" profile list --output=json 2>/dev/null || true)"
  profiles="$(printf '%s' "${profiles}" | tr -d '[:space:]')"
  [[ "${profiles}" == *"\"Name\":\"${PROFILE}\""* ||
    "${profiles}" == *"\"name\":\"${PROFILE}\""* ]]
}

kube() {
  "${KUBECTL_BIN}" --context "${PROFILE}" "$@"
}

collect_diagnostics() {
  [[ "${PROFILE_CREATED}" == "1" ]] || return 0
  log "collecting failure diagnostics"
  kube get nodes -o wide >&2 || true
  kube get pods -A -o wide >&2 || true
  kube get events -A --sort-by=.lastTimestamp >&2 || true
  kube get streamforgepipelines -A -o yaml >&2 || true
  if [[ "${HELM_INSTALLED}" == "1" ]]; then
    kube logs -n "${OPERATOR_NAMESPACE}" \
      -l "app.kubernetes.io/instance=${RELEASE}" \
      --all-containers --tail=300 >&2 || true
  fi
  kube logs -n "${OPERATOR_NAMESPACE}" \
    -l "streamforge.io/pipeline=${PIPELINE_NAME}" \
    --all-containers --tail=300 >&2 || true
  kube logs -n "${KAFKA_NAMESPACE}" deployment/kafka --tail=300 >&2 || true
}

cleanup() {
  local status=$?
  local cleanup_failed=0
  trap - EXIT
  set +e

  if [[ -n "${PORT_FORWARD_PID}" ]]; then
    kill "${PORT_FORWARD_PID}" >/dev/null 2>&1
    wait "${PORT_FORWARD_PID}" >/dev/null 2>&1
  fi

  if [[ "${status}" != "0" ]]; then
    collect_diagnostics
  fi

  if [[ "${PROFILE_CREATED}" == "1" &&
    "${STREAMFORGE_SMOKE_KEEP_CLUSTER:-0}" != "1" ]]; then
    if [[ "${HELM_INSTALLED}" == "1" ]]; then
      "${HELM_BIN}" uninstall "${RELEASE}" \
        --namespace "${OPERATOR_NAMESPACE}" \
        --kube-context "${PROFILE}" >/dev/null 2>&1 || cleanup_failed=1
    fi
    kube delete namespace "${OPERATOR_NAMESPACE}" "${KAFKA_NAMESPACE}" \
      --ignore-not-found --wait=true --timeout=120s >/dev/null 2>&1 ||
      cleanup_failed=1
    "${MINIKUBE_BIN}" delete --profile "${PROFILE}" >/dev/null 2>&1 ||
      cleanup_failed=1
    if profile_exists; then
      printf 'minikube_podman_smoke: ERROR: profile cleanup failed: %s\n' \
        "${PROFILE}" >&2
      cleanup_failed=1
    fi
  elif [[ "${PROFILE_CREATED}" == "1" ]]; then
    log "preserving profile ${PROFILE} because STREAMFORGE_SMOKE_KEEP_CLUSTER=1"
  fi

  if [[ "${ENGINE_IMAGE_CREATED}" == "1" ]]; then
    "${PODMAN_BIN}" image rm --force "${ENGINE_IMAGE}" >/dev/null 2>&1 ||
      cleanup_failed=1
  fi
  if [[ "${OPERATOR_IMAGE_CREATED}" == "1" ]]; then
    "${PODMAN_BIN}" image rm --force "${OPERATOR_IMAGE}" >/dev/null 2>&1 ||
      cleanup_failed=1
  fi
  if [[ -n "${TEMP_DIR}" && -d "${TEMP_DIR}" ]]; then
    rm -rf "${TEMP_DIR}" || cleanup_failed=1
  fi

  if [[ -n "${ORIGINAL_CONTEXT}" ]]; then
    "${KUBECTL_BIN}" config use-context "${ORIGINAL_CONTEXT}" >/dev/null 2>&1 ||
      cleanup_failed=1
  else
    "${KUBECTL_BIN}" config unset current-context >/dev/null 2>&1 || true
  fi

  if [[ "${status}" == "0" && "${cleanup_failed}" != "0" ]]; then
    status=1
    printf 'minikube_podman_smoke: ERROR: cleanup was incomplete\n' >&2
  fi
  exit "${status}"
}

# shellcheck source=scripts/tests/lib/minikube_podman_smoke_lib.sh
source "${REPO_ROOT}/scripts/tests/lib/minikube_podman_smoke_lib.sh"

run_smoke() {
  local kafka_runtime_manifest="${TEMP_DIR}/kafka.yaml"
  local pipeline_runtime_manifest="${TEMP_DIR}/pipeline.yaml"
  local operator_deployment pipeline_pod payload consumed
  local reconcile_count

  [[ "${PROFILE}" =~ ^[a-z0-9][a-z0-9-]{0,49}$ ]] ||
    die "invalid isolated profile name: ${PROFILE}"
  profile_exists &&
    die "refusing to reuse existing Minikube profile: ${PROFILE}"
  if [[ "$("${KUBECTL_BIN}" config get-contexts "${PROFILE}" \
    --output=name 2>/dev/null || true)" == "${PROFILE}" ]]; then
    die "refusing to overwrite existing Kubernetes context: ${PROFILE}"
  fi

  ORIGINAL_CONTEXT="$("${KUBECTL_BIN}" config current-context 2>/dev/null || true)"
  log "starting isolated profile ${PROFILE}"
  # Reserve cleanup ownership before start because a failed start can leave a
  # partial profile. The absence check above makes this profile safe to delete.
  PROFILE_CREATED=1
  # Use the environment override instead of changing Minikube's global
  # `rootless` property; the caller's Minikube configuration stays untouched.
  MINIKUBE_ROOTLESS=true "${MINIKUBE_BIN}" start \
    --profile "${PROFILE}" \
    --driver=podman \
    --container-runtime=containerd \
    --cni=bridge \
    --cpus=2 \
    --memory=4096mb \
    --kubernetes-version="${KUBERNETES_VERSION}"

  kube wait --for=condition=Ready nodes --all --timeout=180s
  wait_for_default_service_account
  wait_for_cluster_network_components
  if ! cluster_service_probe; then
    log "ClusterIP return traffic failed; enabling kube-proxy masqueradeAll"
    enable_kube_proxy_masquerade
    cluster_service_probe ||
      die "ClusterIP connectivity still fails after kube-proxy update"
  fi

  build_and_load_images

  log "installing the StreamForge operator"
  "${HELM_BIN}" upgrade --install "${RELEASE}" "${CHART}" \
    --namespace "${OPERATOR_NAMESPACE}" \
    --create-namespace \
    --kube-context "${PROFILE}" \
    --set "operator.image.repository=${OPERATOR_IMAGE%:*}" \
    --set "operator.image.tag=${OPERATOR_IMAGE##*:}" \
    --set "operator.image.pullPolicy=Never" \
    --set "defaults.image.repository=${ENGINE_IMAGE%:*}" \
    --set "defaults.image.tag=${ENGINE_IMAGE##*:}" \
    --set "defaults.image.pullPolicy=Never" \
    --wait --timeout=5m
  HELM_INSTALLED=1

  operator_deployment="$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" \
      -l "app.kubernetes.io/instance=${RELEASE},app.kubernetes.io/name=streamforge-operator" \
      -o jsonpath='{.items[0].metadata.name}'
  )"
  [[ -n "${operator_deployment}" ]] ||
    die "operator Deployment was not created"
  kube rollout status deployment/"${operator_deployment}" \
    -n "${OPERATOR_NAMESPACE}" --timeout=180s

  sed "s#kafka.kafka.svc.cluster.local:9092#${KAFKA_BOOTSTRAP}#g" \
    "${KAFKA_MANIFEST}" >"${kafka_runtime_manifest}"
  kube apply -f "${kafka_runtime_manifest}" >/dev/null
  kube rollout status deployment/kafka \
    -n "${KAFKA_NAMESPACE}" --timeout=240s
  wait_for_kafka
  kube exec -n "${KAFKA_NAMESPACE}" deployment/kafka -- \
    /opt/kafka/bin/kafka-topics.sh \
    --bootstrap-server localhost:9092 \
    --create --if-not-exists \
    --topic input-topic --partitions 3 --replication-factor 1 >/dev/null
  kube exec -n "${KAFKA_NAMESPACE}" deployment/kafka -- \
    /opt/kafka/bin/kafka-topics.sh \
    --bootstrap-server localhost:9092 \
    --create --if-not-exists \
    --topic output-topic --partitions 3 --replication-factor 1 >/dev/null

  sed \
    -e "s#namespace: streamforge-system#namespace: ${OPERATOR_NAMESPACE}#" \
    -e "s#kafka.kafka.svc.cluster.local:9092#${KAFKA_BOOTSTRAP}#g" \
    "${PIPELINE_MANIFEST}" >"${pipeline_runtime_manifest}"
  kube apply --dry-run=server -f "${pipeline_runtime_manifest}" >/dev/null
  kube apply -f "${pipeline_runtime_manifest}" >/dev/null
  kube wait --for=condition=Ready \
    streamforgepipeline/"${PIPELINE_NAME}" \
    -n "${OPERATOR_NAMESPACE}" --timeout=240s
  kube rollout status deployment/"${PIPELINE_NAME}" \
    -n "${OPERATOR_NAMESPACE}" --timeout=240s
  assert_equal "$(
    kube get streamforgepipeline -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.status.phase}'
  )" "Running" "pipeline phase"

  payload="{\"test_id\":\"podman-minikube-${RUN_ID}\",\"value\":42}"
  printf '%s\n' "${payload}" |
    kube exec -i -n "${KAFKA_NAMESPACE}" deployment/kafka -- \
      /opt/kafka/bin/kafka-console-producer.sh \
      --bootstrap-server localhost:9092 \
      --topic input-topic >/dev/null
  consumed="$(
    kube exec -n "${KAFKA_NAMESPACE}" deployment/kafka -- \
      /opt/kafka/bin/kafka-console-consumer.sh \
      --bootstrap-server localhost:9092 \
      --topic output-topic \
      --from-beginning \
      --max-messages 1 \
      --timeout-ms 60000 2>/dev/null
  )"
  assert_equal "${consumed}" "${payload}" "replicated Kafka record"

  pipeline_pod="$(
    kube get pods -n "${OPERATOR_NAMESPACE}" \
      -l "streamforge.io/pipeline=${PIPELINE_NAME}" \
      -o jsonpath='{.items[0].metadata.name}'
  )"
  [[ -n "${pipeline_pod}" ]] || die "pipeline Pod was not found"
  start_port_forward "${pipeline_pod}"
  validate_observability
  validate_security_contexts "${operator_deployment}"

  log "checking for a reconcile storm over 30 seconds"
  sleep 30
  reconcile_count="$(
    kube logs -n "${OPERATOR_NAMESPACE}" \
      deployment/"${operator_deployment}" \
      --since=35s 2>/dev/null |
      grep -c 'Reconciling pipeline' || true
  )"
  [[ "${reconcile_count}" =~ ^[0-9]+$ ]] ||
    die "unable to count operator reconciliations"
  if ((reconcile_count > MAX_RECONCILES)); then
    die "reconcile storm detected: ${reconcile_count} reconciles in 35 seconds (maximum ${MAX_RECONCILES})"
  fi

  validate_owner_cleanup
  log "all Podman-backed Minikube checks passed"
}

case "${1:-}" in
  --check-prerequisites)
    [[ $# -eq 1 ]] || {
      usage
      exit 2
    }
    check_prerequisites
    exit 0
    ;;
  -h | --help)
    usage
    exit 0
    ;;
  "")
    ;;
  *)
    usage
    die "unknown argument: $1"
    ;;
esac

[[ $# -eq 0 ]] || {
  usage
  exit 2
}

check_prerequisites
TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/streamforge-minikube-smoke.XXXXXX")"
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
run_smoke
