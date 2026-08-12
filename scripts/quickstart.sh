#!/usr/bin/env bash
set -Eeuo pipefail

readonly DEFAULT_VERSION="1.1.0"
readonly DEFAULT_BROKER_IMAGE="docker.redpanda.com/redpandadata/redpanda:v25.1.2"

usage() {
  cat <<'EOF'
Usage: scripts/quickstart.sh <up|verify|down> [--runtime docker|podman]

Commands:
  up       Start Redpanda and StreamForge, then create demo topics.
  verify   Publish a unique record and verify it reaches the destination.
  down     Remove all containers, the network, and generated configuration.

Environment:
  STREAMFORGE_CONTAINER_RUNTIME  Container CLI path or name (docker/podman).
  STREAMFORGE_VERSION            Versioned image tag (default: 1.1.0).
  STREAMFORGE_IMAGE              Complete engine image override.
  STREAMFORGE_BROKER_IMAGE       Complete Redpanda image override.
  STREAMFORGE_QUICKSTART_PREFIX  Resource name prefix.
  STREAMFORGE_QUICKSTART_STATE_DIR
                                 Generated state directory.
EOF
}

die() {
  printf 'quickstart: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '==> %s\n' "$*"
}

resolve_runtime() {
  if [[ -n "${STREAMFORGE_CONTAINER_RUNTIME:-}" ]]; then
    RUNTIME="${STREAMFORGE_CONTAINER_RUNTIME}"
  elif command -v docker >/dev/null 2>&1; then
    RUNTIME="docker"
  elif command -v podman >/dev/null 2>&1; then
    RUNTIME="podman"
  else
    die "Docker or Podman is required"
  fi

  command -v "${RUNTIME}" >/dev/null 2>&1 ||
    die "container runtime not found: ${RUNTIME}"
  "${RUNTIME}" info >/dev/null 2>&1 ||
    die "container runtime is unavailable: ${RUNTIME}"
}

container_exists() {
  "${RUNTIME}" container inspect "$1" >/dev/null 2>&1
}

container_running() {
  [[ "$("${RUNTIME}" container inspect \
    --format '{{.State.Running}}' "$1" 2>/dev/null)" == "true" ]]
}

network_exists() {
  "${RUNTIME}" network inspect "${NETWORK_NAME}" >/dev/null 2>&1
}

cleanup_resources() {
  if container_exists "${ENGINE_NAME}"; then
    "${RUNTIME}" rm --force "${ENGINE_NAME}" >/dev/null 2>&1 || true
  fi
  if container_exists "${BROKER_NAME}"; then
    "${RUNTIME}" rm --force "${BROKER_NAME}" >/dev/null 2>&1 || true
  fi
  if network_exists; then
    "${RUNTIME}" network rm "${NETWORK_NAME}" >/dev/null 2>&1 || true
  fi
  if [[ -f "${CONFIG_FILE}" ]]; then
    rm -f "${CONFIG_FILE}"
  fi
  if [[ -d "${STATE_DIR}" ]]; then
    rmdir "${STATE_DIR}" 2>/dev/null || true
  fi
}

wait_for_broker() {
  local attempt
  local attempts="${STREAMFORGE_QUICKSTART_READY_ATTEMPTS:-30}"
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    if "${RUNTIME}" exec "${BROKER_NAME}" \
      rpk cluster health --brokers localhost:9092 >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  "${RUNTIME}" logs "${BROKER_NAME}" >&2 || true
  die "Redpanda did not become ready after ${attempts} seconds"
}

write_config() {
  mkdir -p "${STATE_DIR}"
  cat >"${CONFIG_FILE}" <<'EOF'
appid: streamforge-quickstart
bootstrap: redpanda:9092
input: quickstart-input
offset: earliest
threads: 1
routing:
  routing_type: filter
  destinations:
    - output: quickstart-output
observability:
  metrics_enabled: true
  metrics_port: 8080
  metrics_path: /metrics
EOF
}

up() {
  local up_complete=0

  if container_exists "${BROKER_NAME}" || container_exists "${ENGINE_NAME}"; then
    die "quickstart resources already exist; run '$0 down' first"
  fi

  trap 'if [[ "${up_complete}" != "1" ]]; then cleanup_resources; fi' EXIT
  write_config
  if ! network_exists; then
    "${RUNTIME}" network create "${NETWORK_NAME}" >/dev/null
  fi

  log "Starting Redpanda with ${RUNTIME}"
  "${RUNTIME}" run --detach \
    --name "${BROKER_NAME}" \
    --network "${NETWORK_NAME}" \
    --network-alias redpanda \
    "${BROKER_IMAGE}" \
    redpanda start \
    --mode dev-container \
    --smp 1 \
    --memory 768M \
    --reserve-memory 0M \
    --node-id 0 \
    --kafka-addr internal://0.0.0.0:9092 \
    --advertise-kafka-addr internal://redpanda:9092 >/dev/null

  wait_for_broker
  "${RUNTIME}" exec "${BROKER_NAME}" \
    rpk topic create quickstart-input quickstart-output \
    --brokers localhost:9092 >/dev/null

  local volume="${CONFIG_FILE}:/etc/streamforge/config.yaml:ro"
  if [[ "$(basename "${RUNTIME}")" == "podman" ]]; then
    volume="${volume},Z"
  fi

  log "Starting ${ENGINE_IMAGE}"
  "${RUNTIME}" run --detach \
    --name "${ENGINE_NAME}" \
    --network "${NETWORK_NAME}" \
    --volume "${volume}" \
    --env CONFIG_FILE=/etc/streamforge/config.yaml \
    --env RUST_LOG=info \
    "${ENGINE_IMAGE}" >/dev/null

  sleep "${STREAMFORGE_QUICKSTART_ENGINE_WAIT_SECONDS:-2}"
  if ! container_running "${ENGINE_NAME}"; then
    "${RUNTIME}" logs "${ENGINE_NAME}" >&2 || true
    die "StreamForge container is not running"
  fi

  up_complete=1
  trap - EXIT
  log "Quickstart is running"
  printf 'Verify replication with: %s verify\n' "$0"
}

verify() {
  container_running "${BROKER_NAME}" ||
    die "Redpanda is not running; run '$0 up' first"
  container_running "${ENGINE_NAME}" ||
    die "StreamForge is not running; run '$0 up' first"

  local record_id="quickstart-$(date +%s)-$$"
  local payload
  payload="{\"id\":\"${record_id}\",\"message\":\"StreamForge quickstart\"}"

  log "Publishing ${record_id}"
  printf '%s\n' "${payload}" |
    "${RUNTIME}" exec --interactive "${BROKER_NAME}" \
      rpk topic produce quickstart-input --brokers localhost:9092 >/dev/null

  local attempt output=""
  local attempts="${STREAMFORGE_QUICKSTART_VERIFY_ATTEMPTS:-10}"
  for ((attempt = 1; attempt <= attempts; attempt++)); do
    output="$("${RUNTIME}" exec "${BROKER_NAME}" \
      timeout 3s rpk topic consume quickstart-output \
      --brokers localhost:9092 \
      --offset -1 \
      --num 1 \
      --format '%v' 2>/dev/null || true)"
    if [[ "${output}" == *"${record_id}"* ]]; then
      log "Verified replication to quickstart-output"
      printf '%s\n' "${output}"
      return 0
    fi
    sleep 1
  done

  "${RUNTIME}" logs "${ENGINE_NAME}" >&2 || true
  die "record ${record_id} was not observed in quickstart-output"
}

down() {
  log "Removing quickstart resources"
  cleanup_resources
  log "Quickstart resources removed"
}

[[ $# -ge 1 ]] || {
  usage
  exit 2
}

ACTION="$1"
shift

if [[ $# -gt 0 ]]; then
  if [[ "$1" != "--runtime" || $# -ne 2 ]]; then
    usage
    exit 2
  fi
  STREAMFORGE_CONTAINER_RUNTIME="$2"
fi

case "${ACTION}" in
  up | verify | down) ;;
  -h | --help)
    usage
    exit 0
    ;;
  *)
    usage
    die "unknown command: ${ACTION}"
    ;;
esac

resolve_runtime

readonly RUNTIME
readonly PREFIX="${STREAMFORGE_QUICKSTART_PREFIX:-streamforge-quickstart}"
readonly NETWORK_NAME="${PREFIX}-network"
readonly BROKER_NAME="${PREFIX}-redpanda"
readonly ENGINE_NAME="${PREFIX}-engine"
readonly STATE_DIR="${STREAMFORGE_QUICKSTART_STATE_DIR:-${TMPDIR:-/tmp}/${PREFIX}-${UID}}"
readonly CONFIG_FILE="${STATE_DIR}/streamforge.yaml"
readonly VERSION="${STREAMFORGE_VERSION:-${DEFAULT_VERSION}}"
readonly ENGINE_IMAGE="${STREAMFORGE_IMAGE:-ghcr.io/rahulbsw/streamforge:${VERSION}}"
readonly BROKER_IMAGE="${STREAMFORGE_BROKER_IMAGE:-${DEFAULT_BROKER_IMAGE}}"

"${ACTION}"
