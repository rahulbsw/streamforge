#!/usr/bin/env bash
#
# Startup-free, duration-controlled Kafka passthrough benchmark.
#
# Usage:
#   scripts/benchmarks/run_throughput_test.sh \
#       [dataset_records] [partitions] [threads] [repetitions]
#
# Environment overrides:
#   BENCHMARK_DURATION_SECONDS       Measured window per repetition (default: 180)
#   BENCHMARK_WARMUP_MESSAGES        Untimed warm-up records (default: 10000)
#   BENCHMARK_INGRESS_TARGET_RATE    Messages/second; 0 is unbounded (default: 0)
#   BENCHMARK_STARTUP_TIMEOUT        Readiness timeout (default: 120)
#   BENCHMARK_DRAIN_TIMEOUT          Post-window drain timeout (default: 180)
#   BENCHMARK_POLL_INTERVAL_MS       Validator sample interval (default: 500)
#   BENCHMARK_RESULTS_ROOT           Artifact root
#   BENCHMARK_PROCESSING_MODE        legacy_batch or partition_ordered
#   BENCHMARK_DELIVERY_MODE          acknowledged or queued
#   BENCHMARK_MAX_IN_FLIGHT          Queued-delivery bound (default: 10000)
#   BENCHMARK_EXECUTION_MODE         container or direct (default: container)
#   BENCHMARK_INGRESS_WORKERS        Parallel producer processes (default: 1)
#   BENCHMARK_KAFKA_BOOTSTRAP        Broker address for direct execution
#   BENCHMARK_KAFKA_DATA_DIR         Shared broker data directory in direct mode
#   BENCHMARK_KAFKA_IMAGE            Pinned image reference in direct mode
#   BENCHMARK_RUST_VERSION           Build toolchain string when rustc is absent
#   BENCHMARK_GIT_SHA                Source revision when .git is unavailable
#   BENCHMARK_GIT_DIRTY              true or false; required with GIT_SHA
#   CONTAINER_RUNTIME                Container CLI (default: podman)
#   KAFKA_CONTAINER                  Broker container (default: benchmark-kafka)
#   INGRESS_CONTAINER                Ingress job container (default: benchmark-ingress)
#   OUTPUT_CONTAINER                 Output job container (default: benchmark-output)
set -Eeuo pipefail
DATASET_RECORDS=${1:-10000}
PARTITIONS=${2:-8}
THREADS=${3:-8}
REPETITIONS=${4:-3}
BENCHMARK_SEED=${BENCHMARK_SEED:-0}
DURATION_SECONDS=${BENCHMARK_DURATION_SECONDS:-180}
WARMUP_MESSAGES=${BENCHMARK_WARMUP_MESSAGES:-10000}
INGRESS_TARGET_RATE=${BENCHMARK_INGRESS_TARGET_RATE:-0}
STARTUP_TIMEOUT=${BENCHMARK_STARTUP_TIMEOUT:-120}
DRAIN_TIMEOUT=${BENCHMARK_DRAIN_TIMEOUT:-180}
POLL_INTERVAL_MS=${BENCHMARK_POLL_INTERVAL_MS:-500}
METRICS_PORT=${BENCHMARK_METRICS_PORT:-19090}
PROCESSING_MODE=${BENCHMARK_PROCESSING_MODE:-partition_ordered}
DELIVERY_MODE=${BENCHMARK_DELIVERY_MODE:-queued}
MAX_IN_FLIGHT=${BENCHMARK_MAX_IN_FLIGHT:-10000}
EXECUTION_MODE=${BENCHMARK_EXECUTION_MODE:-container}
INGRESS_WORKERS=${BENCHMARK_INGRESS_WORKERS:-1}
CONTAINER_RUNTIME=${CONTAINER_RUNTIME:-podman}
KAFKA_CONTAINER=${KAFKA_CONTAINER:-benchmark-kafka}
INGRESS_CONTAINER=${INGRESS_CONTAINER:-benchmark-ingress}
OUTPUT_CONTAINER=${OUTPUT_CONTAINER:-benchmark-output}
KAFKA_BOOTSTRAP=${BENCHMARK_KAFKA_BOOTSTRAP:-}
KAFKA_DATA_DIR=${BENCHMARK_KAFKA_DATA_DIR:-}
KAFKA_IMAGE_OVERRIDE=${BENCHMARK_KAFKA_IMAGE:-}
KAFKA_IMAGE_ID_OVERRIDE=${BENCHMARK_KAFKA_IMAGE_ID:-}
RUST_VERSION_OVERRIDE=${BENCHMARK_RUST_VERSION:-}
BENCHMARK_GIT_SHA=${BENCHMARK_GIT_SHA:-}
BENCHMARK_GIT_DIRTY=${BENCHMARK_GIT_DIRTY:-}

for argument in \
    "dataset_records:$DATASET_RECORDS" \
    "partitions:$PARTITIONS" \
    "threads:$THREADS" \
    "repetitions:$REPETITIONS" \
    "duration_seconds:$DURATION_SECONDS" \
    "warmup_messages:$WARMUP_MESSAGES" \
    "startup_timeout:$STARTUP_TIMEOUT" \
    "drain_timeout:$DRAIN_TIMEOUT" \
    "poll_interval_ms:$POLL_INTERVAL_MS" \
    "metrics_port:$METRICS_PORT" \
    "max_in_flight:$MAX_IN_FLIGHT" \
    "ingress_workers:$INGRESS_WORKERS"; do
    name=${argument%%:*}
    value=${argument#*:}
    if [[ ! "$value" =~ ^[1-9][0-9]*$ ]]; then
        echo "error: $name must be a positive integer" >&2
        exit 2
    fi
done
if [[ "$EXECUTION_MODE" != "container" &&
      "$EXECUTION_MODE" != "direct" ]]; then
    echo "error: BENCHMARK_EXECUTION_MODE must be container or direct" >&2
    exit 2
fi
if [[ "$PROCESSING_MODE" != "legacy_batch" &&
      "$PROCESSING_MODE" != "partition_ordered" ]]; then
    echo "error: BENCHMARK_PROCESSING_MODE must be legacy_batch or partition_ordered" >&2
    exit 2
fi
if [[ "$DELIVERY_MODE" != "acknowledged" &&
      "$DELIVERY_MODE" != "queued" ]]; then
    echo "error: BENCHMARK_DELIVERY_MODE must be acknowledged or queued" >&2
    exit 2
fi
if [[ "$PROCESSING_MODE" == "legacy_batch" && "$DELIVERY_MODE" == "queued" ]]; then
    echo "error: legacy_batch + queued lacks a safe final delivery drain" >&2
    exit 2
fi
if [[ "$METRICS_PORT" -gt 65535 ]]; then
    echo "error: metrics_port must be at most 65535" >&2
    exit 2
fi
if [[ ! "$BENCHMARK_SEED" =~ ^[0-9]+$ ]]; then
    echo "error: BENCHMARK_SEED must be a non-negative integer" >&2
    exit 2
fi
if [[ ! "$INGRESS_TARGET_RATE" =~ ^[0-9]+$ ]]; then
    echo "error: BENCHMARK_INGRESS_TARGET_RATE must be a non-negative integer" >&2
    exit 2
fi
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/release/streamforge"
GENERATOR="$SCRIPT_DIR/generate_json_test_data.sh"
RESULT_HELPER="$SCRIPT_DIR/throughput_results.py"
CONTROL_HELPER="$SCRIPT_DIR/benchmark_jobs.py"
INGRESS_JOB="$SCRIPT_DIR/benchmark_ingress_job.py"
OUTPUT_JOB="$SCRIPT_DIR/benchmark_output_job.py"
METRICS_JOB="$SCRIPT_DIR/benchmark_metrics_job.py"
RESULTS_ROOT=${BENCHMARK_RESULTS_ROOT:-"$PROJECT_ROOT/target/performance-results/throughput"}
RUN_STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_ID="${RUN_STAMP}-$$"
RESULTS_DIR="$RESULTS_ROOT/sustained-$RUN_ID"
DATA_FILE="$RESULTS_DIR/payloads.jsonl"
AGGREGATE_RESULT="$RESULTS_DIR/result.json"
METRICS_URL="http://127.0.0.1:${METRICS_PORT}/metrics"
SAMPLE_INTERVAL_SECONDS="$((POLL_INTERVAL_MS / 1000)).$(printf '%03d' "$((POLL_INTERVAL_MS % 1000))")"
CONTROL_TIMEOUT=$((STARTUP_TIMEOUT + DURATION_SECONDS + DRAIN_TIMEOUT))

CURRENT_STREAMFORGE_PID=""
CURRENT_JOB_PIDS=()
CREATED_TOPICS=()
RUN_RESULT_FILES=()

log() {
    printf '[benchmark] %s\n' "$*"
}
require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "error: required command not found: $1" >&2
        exit 1
    fi
}
kafka_topics() {
    if [[ "$EXECUTION_MODE" == "direct" ]]; then
        kafka-topics --bootstrap-server "$KAFKA_BOOTSTRAP" "$@"
    else
        "$CONTAINER_RUNTIME" exec "$KAFKA_CONTAINER" \
            kafka-topics --bootstrap-server localhost:9092 "$@"
    fi
}
topic_end_offset_sum() {
    local topic=$1
    if [[ "$EXECUTION_MODE" == "direct" ]]; then
        kafka-get-offsets \
            --bootstrap-server "$KAFKA_BOOTSTRAP" \
            --topic "$topic" \
            --time -1 2>/dev/null |
            awk -F: '{ total += $3 } END { printf "%.0f", total + 0 }'
    else
        "$CONTAINER_RUNTIME" exec "$KAFKA_CONTAINER" \
            kafka-get-offsets \
            --bootstrap-server localhost:9092 \
            --topic "$topic" \
            --time -1 2>/dev/null |
            awk -F: '{ total += $3 } END { printf "%.0f", total + 0 }'
    fi
}
topic_data_removed() {
    local topic=$1
    if [[ "$EXECUTION_MODE" == "direct" ]]; then
        ! compgen -G "${KAFKA_DATA_DIR}/${topic}-*" >/dev/null
    else
        "$CONTAINER_RUNTIME" exec "$KAFKA_CONTAINER" bash -lc \
            'shopt -s nullglob; paths=(/var/lib/kafka/data/"$1"-*); ((${#paths[@]} == 0))' \
            benchmark-delete "$topic"
    fi
}
delete_topics_and_wait() {
    local topics=("$@")
    local topic
    for topic in "${topics[@]}"; do
        kafka_topics --delete --topic "$topic" >/dev/null
    done
    local deadline=$((SECONDS + STARTUP_TIMEOUT))
    for topic in "${topics[@]}"; do
        while [[ "$SECONDS" -le "$deadline" ]]; do
            if ! kafka_topics --describe --topic "$topic" >/dev/null 2>&1; then
                break
            fi
            sleep 0.25
        done
        if kafka_topics --describe --topic "$topic" >/dev/null 2>&1; then
            echo "error: timed out deleting benchmark topic metadata: $topic" >&2
            return 1
        fi
    done
    for topic in "${topics[@]}"; do
        while [[ "$SECONDS" -le "$deadline" ]]; do
            if topic_data_removed "$topic"; then
                break
            fi
            sleep 0.5
        done
        if ! topic_data_removed "$topic"; then
            echo "error: timed out reclaiming benchmark topic files: $topic" >&2
            return 1
        fi
    done
}
json_field() {
    python3 "$CONTROL_HELPER" json-field --path "$1" --field "$2"
}
cleanup() {
    local status=$?
    trap - EXIT
    for pid in "${CURRENT_JOB_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    if [[ "$EXECUTION_MODE" == "direct" ]]; then
        pkill -f kafka-console-producer >/dev/null 2>&1 || true
        pkill -f 'kafka-consumer-perf-test|kafka-console-consumer' \
            >/dev/null 2>&1 || true
    else
        "$CONTAINER_RUNTIME" exec "$INGRESS_CONTAINER" \
            pkill -f kafka-console-producer >/dev/null 2>&1 || true
        "$CONTAINER_RUNTIME" exec "$OUTPUT_CONTAINER" \
            pkill -f 'kafka-consumer-perf-test|kafka-console-consumer' \
            >/dev/null 2>&1 || true
    fi
    if [[ -n "$CURRENT_STREAMFORGE_PID" ]] &&
       kill -0 "$CURRENT_STREAMFORGE_PID" 2>/dev/null; then
        kill "$CURRENT_STREAMFORGE_PID" 2>/dev/null || true
        wait "$CURRENT_STREAMFORGE_PID" 2>/dev/null || true
    fi
    for topic in "${CREATED_TOPICS[@]}"; do
        kafka_topics --delete --topic "$topic" >/dev/null 2>&1 || true
    done
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

wait_for_file() {
    local target=$1
    local deadline=$((SECONDS + STARTUP_TIMEOUT))
    while [[ "$SECONDS" -le "$deadline" ]]; do
        if [[ -f "$target" ]]; then
            return 0
        fi
        if [[ -n "$CURRENT_STREAMFORGE_PID" ]] &&
           ! kill -0 "$CURRENT_STREAMFORGE_PID" 2>/dev/null; then
            echo "error: StreamForge exited while waiting for $target" >&2
            return 1
        fi
        for pid in "${CURRENT_JOB_PIDS[@]}"; do
            if ! kill -0 "$pid" 2>/dev/null; then
                echo "error: benchmark job $pid exited while waiting for $target" >&2
                return 1
            fi
        done
        sleep 0.1
    done
    echo "error: timed out waiting for $target" >&2
    return 1
}

wait_for_warmup() {
    local input_topic=$1
    local output_topic=$2
    local metrics_progress=$3
    local deadline=$((SECONDS + STARTUP_TIMEOUT))
    local stable_samples=0
    while [[ "$SECONDS" -le "$deadline" ]]; do
        local input_count output_count consumed produced delivered errors
        input_count=$(topic_end_offset_sum "$input_topic")
        output_count=$(topic_end_offset_sum "$output_topic")
        consumed=$(json_field "$metrics_progress" consumed)
        produced=$(json_field "$metrics_progress" produced)
        delivered=$(json_field "$metrics_progress" delivered)
        errors=$(json_field "$metrics_progress" errors)
        if [[ "$input_count" -gt "$WARMUP_MESSAGES" ||
              "$output_count" -gt "$WARMUP_MESSAGES" ||
              "$consumed" -gt "$WARMUP_MESSAGES" ||
              "$produced" -gt "$WARMUP_MESSAGES" ||
              "$delivered" -gt "$WARMUP_MESSAGES" ||
              "$errors" -ne 0 ]]; then
            echo "error: warm-up exceeded exact accounting bounds" >&2
            return 1
        fi
        if [[ "$input_count" -eq "$WARMUP_MESSAGES" &&
              "$output_count" -eq "$WARMUP_MESSAGES" &&
              "$consumed" -eq "$WARMUP_MESSAGES" &&
              "$produced" -eq "$WARMUP_MESSAGES" &&
              "$delivered" -eq "$WARMUP_MESSAGES" ]]; then
            stable_samples=$((stable_samples + 1))
            if [[ "$stable_samples" -ge 2 ]]; then
                return 0
            fi
        else
            stable_samples=0
        fi
        sleep "$SAMPLE_INTERVAL_SECONDS"
    done
    echo "error: warm-up did not reach stable exact counts" >&2
    return 1
}

REQUIRED_COMMANDS=(curl python3 awk ps)
if [[ "$EXECUTION_MODE" == "container" ]]; then
    REQUIRED_COMMANDS+=("$CONTAINER_RUNTIME")
else
    REQUIRED_COMMANDS+=(
        kafka-topics
        kafka-get-offsets
        kafka-broker-api-versions
        kafka-console-producer
        kafka-consumer-perf-test
    )
fi
if [[ -z "$BENCHMARK_GIT_SHA" ]]; then
    REQUIRED_COMMANDS+=(git)
fi
if [[ -z "$RUST_VERSION_OVERRIDE" ]]; then
    REQUIRED_COMMANDS+=(rustc)
fi
for command in "${REQUIRED_COMMANDS[@]}"; do
    require_command "$command"
done
if [[ ! -x "$BINARY" ]]; then
    echo "error: release binary not found at $BINARY" >&2
    echo "build it with: cargo build --release --bin streamforge" >&2
    exit 1
fi
if [[ "$EXECUTION_MODE" == "container" ]]; then
    for container in \
        "$KAFKA_CONTAINER" "$INGRESS_CONTAINER" "$OUTPUT_CONTAINER"; do
        if [[ "$("$CONTAINER_RUNTIME" inspect --format '{{.State.Running}}' "$container" 2>/dev/null)" != "true" ]]; then
            echo "error: required benchmark container is not running: $container" >&2
            echo "start with: $CONTAINER_RUNTIME compose -f docker-compose.benchmark.yml up -d" >&2
            exit 1
        fi
    done
    if ! "$CONTAINER_RUNTIME" exec "$INGRESS_CONTAINER" \
        kafka-broker-api-versions --bootstrap-server kafka:29092 \
        >/dev/null 2>&1; then
        echo "error: ingress container cannot reach the internal Kafka listener" >&2
        exit 1
    fi
    if [[ "$("$CONTAINER_RUNTIME" port "$KAFKA_CONTAINER" 9092/tcp)" != "127.0.0.1:9092" ]]; then
        echo "error: Kafka must be published only on 127.0.0.1:9092" >&2
        exit 1
    fi
    KAFKA_BOOTSTRAP="kafka:29092"
else
    : "${KAFKA_BOOTSTRAP:=127.0.0.1:9092}"
    if [[ -z "$KAFKA_DATA_DIR" || ! -d "$KAFKA_DATA_DIR" ]]; then
        echo "error: direct mode requires an existing BENCHMARK_KAFKA_DATA_DIR" >&2
        exit 1
    fi
    if [[ -z "$KAFKA_IMAGE_OVERRIDE" ]]; then
        echo "error: direct mode requires BENCHMARK_KAFKA_IMAGE" >&2
        exit 1
    fi
    if ! kafka-broker-api-versions \
        --bootstrap-server "$KAFKA_BOOTSTRAP" >/dev/null 2>&1; then
        echo "error: direct benchmark runner cannot reach Kafka" >&2
        exit 1
    fi
fi
if curl --silent --fail --max-time 1 "$METRICS_URL" >/dev/null 2>&1; then
    echo "error: metrics port $METRICS_PORT is already in use" >&2
    exit 1
fi

mkdir -p "$RESULTS_DIR"
"$GENERATOR" "$DATASET_RECORDS" "$DATA_FILE" "$BENCHMARK_SEED"
DATASET_SHA=$(python3 "$RESULT_HELPER" sha256 "$DATA_FILE")
if [[ -n "$BENCHMARK_GIT_SHA" || -n "$BENCHMARK_GIT_DIRTY" ]]; then
    if [[ -z "$BENCHMARK_GIT_SHA" ||
          ("$BENCHMARK_GIT_DIRTY" != "true" &&
           "$BENCHMARK_GIT_DIRTY" != "false") ]]; then
        echo "error: BENCHMARK_GIT_SHA and boolean BENCHMARK_GIT_DIRTY must be set together" >&2
        exit 2
    fi
    GIT_SHA=$BENCHMARK_GIT_SHA
    GIT_DIRTY=$BENCHMARK_GIT_DIRTY
else
    GIT_SHA=$(git -C "$PROJECT_ROOT" rev-parse HEAD)
    GIT_DIRTY=false
    [[ -n "$(git -C "$PROJECT_ROOT" status --porcelain)" ]] && GIT_DIRTY=true
fi
OS_DESCRIPTION=$(uname -srm)
if [[ "$(uname -s)" == "Darwin" ]]; then
    CPU_DESCRIPTION=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || uname -m)
elif [[ -r /proc/cpuinfo ]]; then
    CPU_DESCRIPTION=$(awk -F: '/model name/ { sub(/^[ \t]+/, "", $2); print $2; exit }' /proc/cpuinfo)
else
    CPU_DESCRIPTION=$(uname -m)
fi
if [[ -n "$RUST_VERSION_OVERRIDE" ]]; then
    RUST_VERSION=$RUST_VERSION_OVERRIDE
else
    RUST_VERSION=$(rustc --version)
fi
if [[ "$EXECUTION_MODE" == "direct" ]]; then
    KAFKA_IMAGE=$KAFKA_IMAGE_OVERRIDE
    KAFKA_IMAGE_ID=${KAFKA_IMAGE_ID_OVERRIDE:-$KAFKA_IMAGE_OVERRIDE}
    CONTAINER_RUNTIME_VERSION="ecs-direct"
else
    KAFKA_IMAGE=$("$CONTAINER_RUNTIME" inspect --format '{{.Config.Image}}' "$KAFKA_CONTAINER")
    KAFKA_IMAGE_ID=$("$CONTAINER_RUNTIME" inspect --format '{{.Image}}' "$KAFKA_CONTAINER")
    CONTAINER_RUNTIME_VERSION=$("$CONTAINER_RUNTIME" --version)
fi
CONTAINER_VM_CPUS=0
CONTAINER_VM_MEMORY_MIB=0
if [[ "$EXECUTION_MODE" == "container" &&
      "$CONTAINER_RUNTIME" == "podman" ]]; then
    CONTAINER_VM_CPUS=$(
        "$CONTAINER_RUNTIME" machine inspect --format '{{.Resources.CPUs}}'
    )
    CONTAINER_VM_MEMORY_MIB=$(
        "$CONTAINER_RUNTIME" machine inspect --format '{{.Resources.Memory}}'
    )
fi

log "results: $RESULTS_DIR"
log "measurement: ${DURATION_SECONDS}s x ${REPETITIONS} repetition(s)"
log "ingress target: ${INGRESS_TARGET_RATE} msg/s (0 means unbounded)"
log "startup and warm-up are outside the shared measurement barrier"

JOB_MODE_ARGS=()
if [[ "$EXECUTION_MODE" == "direct" ]]; then
    JOB_MODE_ARGS+=(--direct)
fi

repetition=1
while [[ "$repetition" -le "$REPETITIONS" ]]; do
    REPETITION_DIR="$RESULTS_DIR/run-$repetition"
    JOB_DIR="$REPETITION_DIR/jobs"
    mkdir -p "$JOB_DIR"
    TOPIC_SUFFIX="${RUN_ID}-r${repetition}"
    INPUT_TOPIC="streamforge-bench-${TOPIC_SUFFIX}-input"
    OUTPUT_TOPIC="streamforge-bench-${TOPIC_SUFFIX}-output"
    GROUP_ID="streamforge-bench-${TOPIC_SUFFIX}"
    OBSERVER_GROUP="${GROUP_ID}-output-observer"
    CONFIG_PATH="$REPETITION_DIR/config.json"
    STREAMFORGE_LOG="$REPETITION_DIR/streamforge.log"
    RUN_RESULT="$REPETITION_DIR/result.json"
    START_SIGNAL="$JOB_DIR/start.json"
    WARMUP_SIGNAL="$JOB_DIR/warmup.go"
    INGRESS_READY="$JOB_DIR/ingress-ready.json"
    INGRESS_WARMUP="$JOB_DIR/ingress-warmup.json"
    INGRESS_RESULT="$JOB_DIR/ingress-result.json"
    OBSERVER_READY="$JOB_DIR/output-ready.json"
    OBSERVER_RESULT="$JOB_DIR/output-result.json"
    METRICS_READY="$JOB_DIR/metrics-ready.json"
    METRICS_PROGRESS="$JOB_DIR/metrics-progress.json"
    METRICS_RESULT="$JOB_DIR/metrics-result.json"
    METRICS_SAMPLES="$JOB_DIR/metrics-samples.csv"

    kafka_topics --create --topic "$INPUT_TOPIC" \
        --partitions "$PARTITIONS" --replication-factor 1 >/dev/null
    CREATED_TOPICS+=("$INPUT_TOPIC")
    kafka_topics --create --topic "$OUTPUT_TOPIC" \
        --partitions "$PARTITIONS" --replication-factor 1 >/dev/null
    CREATED_TOPICS+=("$OUTPUT_TOPIC")
    python3 "$RESULT_HELPER" write-config \
        "$CONFIG_PATH" "$GROUP_ID" "$INPUT_TOPIC" "$OUTPUT_TOPIC" \
        "$THREADS" "$METRICS_PORT" "$PROCESSING_MODE" "$DELIVERY_MODE" \
        "$MAX_IN_FLIGHT"

    log "run $repetition/$REPETITIONS: starting and warming StreamForge"
    CONFIG_FILE="$CONFIG_PATH" "$BINARY" >"$STREAMFORGE_LOG" 2>&1 &
    CURRENT_STREAMFORGE_PID=$!
    ready=0
    deadline=$((SECONDS + STARTUP_TIMEOUT))
    while [[ "$SECONDS" -le "$deadline" ]]; do
        if ! kill -0 "$CURRENT_STREAMFORGE_PID" 2>/dev/null; then
            echo "error: StreamForge exited before metrics readiness" >&2
            exit 1
        fi
        if curl --silent --fail --max-time 1 "$METRICS_URL" >/dev/null; then
            ready=1
            break
        fi
        sleep 0.1
    done
    [[ "$ready" -eq 1 ]] || {
        echo "error: metrics endpoint did not become ready" >&2
        exit 1
    }

    python3 "$OUTPUT_JOB" \
        --runtime "$CONTAINER_RUNTIME" --container "$OUTPUT_CONTAINER" \
        "${JOB_MODE_ARGS[@]}" \
        --bootstrap "$KAFKA_BOOTSTRAP" --topic "$OUTPUT_TOPIC" \
        --group "$OBSERVER_GROUP" --warmup-messages "$WARMUP_MESSAGES" \
        --ready "$OBSERVER_READY" \
        --start-signal "$START_SIGNAL" --ingress-result "$INGRESS_RESULT" \
        --result "$OBSERVER_RESULT" --log "$JOB_DIR/output-consumer.log" \
        --control-timeout "$CONTROL_TIMEOUT" &
    OBSERVER_PID=$!
    CURRENT_JOB_PIDS+=("$OBSERVER_PID")

    python3 "$METRICS_JOB" \
        --url "$METRICS_URL" --destination "$OUTPUT_TOPIC" \
        --pid "$CURRENT_STREAMFORGE_PID" --warmup-messages "$WARMUP_MESSAGES" \
        --ready "$METRICS_READY" --progress "$METRICS_PROGRESS" \
        --start-signal "$START_SIGNAL" --ingress-result "$INGRESS_RESULT" \
        --result "$METRICS_RESULT" --samples "$METRICS_SAMPLES" \
        --sample-interval "$SAMPLE_INTERVAL_SECONDS" \
        --control-timeout "$CONTROL_TIMEOUT" &
    METRICS_PID=$!
    CURRENT_JOB_PIDS+=("$METRICS_PID")

    python3 "$INGRESS_JOB" \
        --runtime "$CONTAINER_RUNTIME" --container "$INGRESS_CONTAINER" \
        "${JOB_MODE_ARGS[@]}" \
        --bootstrap "$KAFKA_BOOTSTRAP" --topic "$INPUT_TOPIC" \
        --payload "$DATA_FILE" --workers "$INGRESS_WORKERS" \
        --warmup-messages "$WARMUP_MESSAGES" --ready "$INGRESS_READY" \
        --warmup-signal "$WARMUP_SIGNAL" --warmup-result "$INGRESS_WARMUP" \
        --start-signal "$START_SIGNAL" --result "$INGRESS_RESULT" \
        --log "$JOB_DIR/ingress-producer.log" \
        --control-timeout "$CONTROL_TIMEOUT" --flush-timeout "$DRAIN_TIMEOUT" \
        --target-rate "$INGRESS_TARGET_RATE" &
    INGRESS_PID=$!
    CURRENT_JOB_PIDS+=("$INGRESS_PID")

    wait_for_file "$OBSERVER_READY"
    wait_for_file "$METRICS_READY"
    wait_for_file "$INGRESS_READY"
    : >"$WARMUP_SIGNAL"
    wait_for_file "$INGRESS_WARMUP"
    wait_for_file "$METRICS_PROGRESS"
    wait_for_warmup \
        "$INPUT_TOPIC" "$OUTPUT_TOPIC" \
        "$METRICS_PROGRESS"

    log "run $repetition/$REPETITIONS: all jobs ready; releasing ${DURATION_SECONDS}s barrier"
    python3 "$CONTROL_HELPER" write-barrier \
        --path "$START_SIGNAL" --duration-seconds "$DURATION_SECONDS"

    if ! wait "$INGRESS_PID"; then
        echo "error: ingress job failed; see $INGRESS_RESULT" >&2
        exit 1
    fi
    if ! wait "$OBSERVER_PID"; then
        echo "error: output observer failed; see $OBSERVER_RESULT" >&2
        exit 1
    fi
    if ! wait "$METRICS_PID"; then
        echo "error: metrics validator failed; see $METRICS_RESULT" >&2
        exit 1
    fi
    CURRENT_JOB_PIDS=()

    INPUT_END_OFFSET=$(topic_end_offset_sum "$INPUT_TOPIC")
    OUTPUT_END_OFFSET=$(topic_end_offset_sum "$OUTPUT_TOPIC")
    python3 "$RESULT_HELPER" write-run \
        "$RUN_RESULT" "$repetition" "$INPUT_TOPIC" "$OUTPUT_TOPIC" "$GROUP_ID" \
        "$WARMUP_MESSAGES" "$DURATION_SECONDS" \
        "$INPUT_END_OFFSET" "$OUTPUT_END_OFFSET" "$CONFIG_PATH" \
        "$INGRESS_RESULT" "$OBSERVER_RESULT" "$METRICS_RESULT" "$METRICS_SAMPLES"
    RUN_RESULT_FILES+=("$RUN_RESULT")

    kill "$CURRENT_STREAMFORGE_PID" 2>/dev/null || true
    wait "$CURRENT_STREAMFORGE_PID" 2>/dev/null || true
    CURRENT_STREAMFORGE_PID=""
    RUN_RATE=$(json_field "$RUN_RESULT" \
        phases.steady_state.output_delivery_rate_messages_per_second)
    RUN_CLASS=$(json_field "$RUN_RESULT" phases.steady_state.classification)
    log "run $repetition: sustained output=${RUN_RATE} msg/s, classification=${RUN_CLASS}"
    delete_topics_and_wait "$INPUT_TOPIC" "$OUTPUT_TOPIC"
    CREATED_TOPICS=()
    repetition=$((repetition + 1))
done

python3 "$RESULT_HELPER" write-aggregate \
    "$AGGREGATE_RESULT" "$DATASET_RECORDS" "$PARTITIONS" "$THREADS" \
    "$REPETITIONS" "$BENCHMARK_SEED" "$DATA_FILE" "$DATASET_SHA" \
    "$WARMUP_MESSAGES" "$DURATION_SECONDS" "$POLL_INTERVAL_MS" \
    "$INGRESS_TARGET_RATE" \
    "$GIT_SHA" "$OS_DESCRIPTION" "$CPU_DESCRIPTION" "$RUST_VERSION" \
    "$GIT_DIRTY" "$KAFKA_CONTAINER" "$KAFKA_IMAGE" "$KAFKA_IMAGE_ID" \
    "$INGRESS_CONTAINER" "$OUTPUT_CONTAINER" "$CONTAINER_RUNTIME_VERSION" \
    "$CONTAINER_VM_CPUS" "$CONTAINER_VM_MEMORY_MIB" \
    "${RUN_RESULT_FILES[@]}"
log "aggregate result: $AGGREGATE_RESULT"
python3 "$RESULT_HELPER" print-summary "$AGGREGATE_RESULT"
