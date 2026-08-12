#!/usr/bin/env bash

# Runtime helpers for minikube_podman_smoke.sh. This file is sourced after the
# main script establishes its immutable configuration and command wrappers.

wait_for_default_service_account() {
  local attempt

  for ((attempt = 1; attempt <= 60; attempt++)); do
    if kube get serviceaccount/default -n default >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done

  die "default ServiceAccount was not created within 60 seconds"
}

wait_for_cluster_network_components() {
  kube rollout status daemonset/kube-proxy \
    -n kube-system --timeout=120s
  kube rollout status deployment/coredns \
    -n kube-system --timeout=120s
}

wait_for_pod_completion() {
  local namespace="$1"
  local pod="$2"
  local attempts="${3:-60}"
  local attempt phase

  for ((attempt = 1; attempt <= attempts; attempt++)); do
    phase="$(kube get pod -n "${namespace}" "${pod}" \
      -o jsonpath='{.status.phase}' 2>/dev/null || true)"
    case "${phase}" in
      Succeeded) return 0 ;;
      Failed)
        kube logs -n "${namespace}" "${pod}" >&2 || true
        return 1
        ;;
    esac
    sleep 1
  done
  kube describe pod -n "${namespace}" "${pod}" >&2 || true
  kube logs -n "${namespace}" "${pod}" >&2 || true
  return 1
}

cluster_service_probe() {
  local probe="clusterip-probe-${RUN_ID}"
  kube delete pod -n default "${probe}" --ignore-not-found >/dev/null 2>&1 || true
  kube run -n default "${probe}" \
    --image=busybox:1.36 \
    --restart=Never \
    --command -- sh -ec \
    'nslookup kubernetes.default.svc.cluster.local. >/dev/null && nc -z -w 5 kubernetes.default.svc.cluster.local. 443' \
    >/dev/null
  if wait_for_pod_completion default "${probe}" 60; then
    kube delete pod -n default "${probe}" --wait=true >/dev/null
    return 0
  fi
  kube delete pod -n default "${probe}" --ignore-not-found --wait=true \
    >/dev/null 2>&1 || true
  return 1
}

enable_kube_proxy_masquerade() {
  local original="${TEMP_DIR}/kube-proxy.yaml"
  local patched="${TEMP_DIR}/kube-proxy-patched.yaml"

  kube get configmap kube-proxy -n kube-system -o yaml >"${original}"
  if grep -F 'masqueradeAll: true' "${original}" >/dev/null; then
    log "kube-proxy masqueradeAll is already enabled"
    return 0
  fi
  grep -F 'masqueradeAll: false' "${original}" >/dev/null ||
    die "kube-proxy config does not contain a supported masqueradeAll setting"
  sed 's/masqueradeAll: false/masqueradeAll: true/' \
    "${original}" >"${patched}"
  kube apply -f "${patched}" >/dev/null
  kube rollout restart daemonset/kube-proxy -n kube-system >/dev/null
  kube rollout status daemonset/kube-proxy -n kube-system --timeout=120s
  kube rollout restart deployment/coredns -n kube-system >/dev/null
  kube rollout status deployment/coredns -n kube-system --timeout=120s
}

build_and_load_images() {
  local engine_archive="${TEMP_DIR}/streamforge.oci"
  local operator_archive="${TEMP_DIR}/streamforge-operator.oci"

  log "building ${ENGINE_IMAGE}"
  "${PODMAN_BIN}" build \
    --file "${REPO_ROOT}/Dockerfile" \
    --tag "${ENGINE_IMAGE}" \
    "${REPO_ROOT}"
  ENGINE_IMAGE_CREATED=1

  log "building ${OPERATOR_IMAGE}"
  "${PODMAN_BIN}" build \
    --file "${REPO_ROOT}/operator/Dockerfile" \
    --tag "${OPERATOR_IMAGE}" \
    "${REPO_ROOT}"
  OPERATOR_IMAGE_CREATED=1

  "${PODMAN_BIN}" save --format oci-archive \
    --output "${engine_archive}" "${ENGINE_IMAGE}"
  "${PODMAN_BIN}" save --format oci-archive \
    --output "${operator_archive}" "${OPERATOR_IMAGE}"
  "${MINIKUBE_BIN}" --profile "${PROFILE}" image load "${engine_archive}"
  "${MINIKUBE_BIN}" --profile "${PROFILE}" image load "${operator_archive}"

  "${MINIKUBE_BIN}" --profile "${PROFILE}" image ls |
    grep -F "${ENGINE_IMAGE}" >/dev/null ||
    die "engine image was not loaded into Minikube"
  "${MINIKUBE_BIN}" --profile "${PROFILE}" image ls |
    grep -F "${OPERATOR_IMAGE}" >/dev/null ||
    die "operator image was not loaded into Minikube"
}

assert_equal() {
  local actual="$1"
  local expected="$2"
  local description="$3"
  [[ "${actual}" == "${expected}" ]] ||
    die "${description}: expected '${expected}', found '${actual}'"
}

wait_for_kafka() {
  local attempt
  for ((attempt = 1; attempt <= 60; attempt++)); do
    if kube exec -n "${KAFKA_NAMESPACE}" deployment/kafka -- \
      /opt/kafka/bin/kafka-topics.sh \
      --bootstrap-server localhost:9092 --list >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  kube logs -n "${KAFKA_NAMESPACE}" deployment/kafka >&2 || true
  die "Kafka did not become ready"
}

start_port_forward() {
  local pipeline_pod="$1"
  local log_file="${TEMP_DIR}/port-forward.log"
  local attempt

  kube port-forward -n "${OPERATOR_NAMESPACE}" \
    "pod/${pipeline_pod}" "${LOCAL_METRICS_PORT}:9090" \
    >"${log_file}" 2>&1 &
  PORT_FORWARD_PID=$!

  for ((attempt = 1; attempt <= 30; attempt++)); do
    if "${CURL_BIN}" --fail --silent \
      "http://127.0.0.1:${LOCAL_METRICS_PORT}/health" >/dev/null 2>&1; then
      return 0
    fi
    if ! kill -0 "${PORT_FORWARD_PID}" >/dev/null 2>&1; then
      sed -n '1,200p' "${log_file}" >&2
      die "pipeline port-forward exited"
    fi
    sleep 1
  done
  sed -n '1,200p' "${log_file}" >&2
  die "pipeline observability endpoint did not become reachable"
}

validate_observability() {
  local health ready metrics
  health="$("${CURL_BIN}" --fail --silent \
    "http://127.0.0.1:${LOCAL_METRICS_PORT}/health")"
  assert_equal "${health}" "OK" "/health response"

  ready="$("${CURL_BIN}" --fail --silent \
    "http://127.0.0.1:${LOCAL_METRICS_PORT}/ready")"
  [[ "${ready}" == *'"status":"ready"'* ]] ||
    die "/ready did not report ready: ${ready}"
  [[ "${ready}" == *'"component":"runtime","ready":true'* ]] ||
    die "/ready did not report runtime ready: ${ready}"
  [[ "${ready}" == *'"component":"kafka","ready":true'* ]] ||
    die "/ready did not report Kafka ready: ${ready}"

  metrics="$("${CURL_BIN}" --fail --silent \
    "http://127.0.0.1:${LOCAL_METRICS_PORT}/metrics")"
  grep -E "^streamforge_build_info\\{version=\"${CHART_VERSION}\"\\} 1$" \
    <<<"${metrics}" >/dev/null ||
    die "streamforge_build_info for version ${CHART_VERSION} is missing"
  grep -E '^streamforge_ready 1$' <<<"${metrics}" >/dev/null ||
    die "streamforge_ready is not 1"
}

validate_security_contexts() {
  local operator_deployment="$1"

  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${operator_deployment}" \
      -o jsonpath='{.spec.template.spec.securityContext.runAsUser}'
  )" "65532" "operator runAsUser"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${operator_deployment}" \
      -o jsonpath='{.spec.template.spec.containers[0].securityContext.allowPrivilegeEscalation}'
  )" "false" "operator allowPrivilegeEscalation"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${operator_deployment}" \
      -o jsonpath='{.spec.template.spec.containers[0].securityContext.readOnlyRootFilesystem}'
  )" "true" "operator readOnlyRootFilesystem"

  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.spec.template.spec.securityContext.runAsUser}'
  )" "65532" "pipeline runAsUser"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.spec.template.spec.automountServiceAccountToken}'
  )" "false" "pipeline automountServiceAccountToken"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.spec.template.spec.containers[0].securityContext.allowPrivilegeEscalation}'
  )" "false" "pipeline allowPrivilegeEscalation"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.spec.template.spec.containers[0].securityContext.readOnlyRootFilesystem}'
  )" "true" "pipeline readOnlyRootFilesystem"
  assert_equal "$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.spec.template.spec.containers[0].securityContext.capabilities.drop[0]}'
  )" "ALL" "pipeline dropped capabilities"
}

validate_owner_cleanup() {
  local pipeline_uid deployment_owners configmap_owners

  pipeline_uid="$(
    kube get streamforgepipeline -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.metadata.uid}'
  )"
  deployment_owners="$(
    kube get deployment -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}" \
      -o jsonpath='{.metadata.ownerReferences[*].uid}'
  )"
  configmap_owners="$(
    kube get configmap -n "${OPERATOR_NAMESPACE}" "${PIPELINE_NAME}-config" \
      -o jsonpath='{.metadata.ownerReferences[*].uid}'
  )"
  [[ " ${deployment_owners} " == *" ${pipeline_uid} "* ]] ||
    die "pipeline Deployment is not owned by the StreamforgePipeline"
  [[ " ${configmap_owners} " == *" ${pipeline_uid} "* ]] ||
    die "pipeline ConfigMap is not owned by the StreamforgePipeline"

  kube delete streamforgepipeline -n "${OPERATOR_NAMESPACE}" \
    "${PIPELINE_NAME}" --wait=true >/dev/null
  kube wait --for=delete deployment/"${PIPELINE_NAME}" \
    -n "${OPERATOR_NAMESPACE}" --timeout=90s
  kube wait --for=delete configmap/"${PIPELINE_NAME}-config" \
    -n "${OPERATOR_NAMESPACE}" --timeout=90s
}
