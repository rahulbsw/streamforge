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

network_id() {
  local template id
  for template in '{{.Id}}' '{{.ID}}' '{{.id}}'; do
    if id="$("${RUNTIME}" network inspect --format "${template}" "${NETWORK_NAME}" 2>/dev/null)" &&
      [[ -n "${id}" && "${id}" != "<no value>" && "${id}" != "<nil>" ]]; then
      printf '%s\n' "${id}"
      return 0
    fi
  done
  return 1
}

network_owner_token() {
  local token
  if ! token="$("${RUNTIME}" network inspect \
    --format '{{ index .Labels "com.streamforge.quickstart.owner" }}' \
    "${NETWORK_NAME}" 2>/dev/null)"; then
    return 1
  fi
  [[ -n "${token}" && "${token}" != "<no value>" && "${token}" != "<nil>" ]] || return 2
  printf '%s\n' "${token}"
}

cleanup_resources() {
  local cleanup_failed=0 expected_network_id expected_owner_token actual_network_id actual_owner_token
  if container_exists "${ENGINE_NAME}"; then
    "${RUNTIME}" rm --force "${ENGINE_NAME}" >/dev/null 2>&1 || true
  fi
  if container_exists "${BROKER_NAME}"; then
    "${RUNTIME}" rm --force "${BROKER_NAME}" >/dev/null 2>&1 || true
  fi
  if [[ -f "${NETWORK_OWNERSHIP_FILE}" ]]; then
    expected_network_id="$(sed -n '1p' "${NETWORK_OWNERSHIP_FILE}")"
    expected_owner_token="$(sed -n '2p' "${NETWORK_OWNERSHIP_FILE}")"
    if network_exists; then
      if ! actual_network_id="$(network_id)"; then
        printf 'quickstart: could not inspect network %s; ownership record retained\n' \
          "${NETWORK_NAME}" >&2
        cleanup_failed=1
      elif actual_owner_token="$(network_owner_token)"; then
        if [[ "${actual_owner_token}" == "${expected_owner_token}" &&
          ( "${expected_network_id}" == "pending" ||
            "${actual_network_id}" == "${expected_network_id}" ) ]]; then
          if ! "${RUNTIME}" network rm "${actual_network_id}" >/dev/null 2>&1; then
            printf 'quickstart: failed to remove owned network %s; ownership record retained\n' \
              "${NETWORK_NAME}" >&2
            cleanup_failed=1
          else
            rm -f "${NETWORK_OWNERSHIP_FILE}"
          fi
        else
          printf 'quickstart: network %s no longer matches its ownership record; leaving it in place\n' \
            "${NETWORK_NAME}" >&2
          rm -f "${NETWORK_OWNERSHIP_FILE}"
        fi
      else
        if [[ "$?" == "2" ]]; then
          printf 'quickstart: network %s has no ownership label; leaving it in place\n' \
            "${NETWORK_NAME}" >&2
          rm -f "${NETWORK_OWNERSHIP_FILE}"
        else
          printf 'quickstart: could not verify network %s; ownership record retained\n' \
            "${NETWORK_NAME}" >&2
          cleanup_failed=1
        fi
      fi
    else
      rm -f "${NETWORK_OWNERSHIP_FILE}"
    fi
  fi
  if [[ -f "${CONFIG_FILE}" ]]; then
    rm -f "${CONFIG_FILE}"
  fi
  if [[ -d "${STATE_DIR}" ]]; then
    rmdir "${STATE_DIR}" 2>/dev/null || true
  fi
  return "${cleanup_failed}"
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
  local up_complete=0 created_network_id create_result owner_token current_owner_token current_network_id

  if container_exists "${BROKER_NAME}" || container_exists "${ENGINE_NAME}"; then
    die "quickstart resources already exist; run '$0 down' first"
  fi
  if network_exists; then
    die "quickstart network already exists; remove it or choose a different prefix"
  fi

  trap 'if [[ "${up_complete}" != "1" ]]; then cleanup_resources; fi' EXIT
  write_config
  owner_token="${UID}-${BASHPID}-${RANDOM}-${RANDOM}-$(date +%s)"
  # Save the token first so EXIT cleanup can recover the network even if the
  # runtime creates it but the CLI then exits unsuccessfully.
  printf '%s\n%s\n' "pending" "${owner_token}" >"${NETWORK_OWNERSHIP_FILE}"
  if ! create_result="$("${RUNTIME}" network create \
    --label "com.streamforge.quickstart.owner=${owner_token}" "${NETWORK_NAME}")"; then
    die "network create failed; ownership record retained for cleanup"
  fi
  if [[ "${create_result}" =~ ^[[:xdigit:]]{12,64}$ ]]; then
    created_network_id="${create_result}"
  else
    # Podman returns the name rather than the ID. Record a pending ID so the
    # EXIT cleanup can recover it by verifying this run's unique owner label.
    created_network_id="pending"
  fi
  printf '%s\n%s\n' "${created_network_id}" "${owner_token}" >"${NETWORK_OWNERSHIP_FILE}"
  if [[ "${created_network_id}" == "pending" ]] &&
    ! created_network_id="$(network_id)"; then
    die "could not inspect the created network; ownership record retained for cleanup"
  fi
  if ! current_network_id="$(network_id)" || [[ "${current_network_id}" != "${created_network_id}" ]] ||
    ! current_owner_token="$(network_owner_token)" || [[ "${current_owner_token}" != "${owner_token}" ]]; then
    die "created network identity could not be verified; ownership record retained for cleanup"
  fi
  printf '%s\n%s\n' "${created_network_id}" "${owner_token}" >"${NETWORK_OWNERSHIP_FILE}"

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
readonly NETWORK_OWNERSHIP_FILE="${STATE_DIR}/.network-created-by-quickstart"
readonly VERSION="${STREAMFORGE_VERSION:-${DEFAULT_VERSION}}"
readonly ENGINE_IMAGE="${STREAMFORGE_IMAGE:-ghcr.io/rahulbsw/streamforge:${VERSION}}"
readonly BROKER_IMAGE="${STREAMFORGE_BROKER_IMAGE:-${DEFAULT_BROKER_IMAGE}}"

"${ACTION}"
