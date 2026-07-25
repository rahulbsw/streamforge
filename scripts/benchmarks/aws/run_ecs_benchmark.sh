#!/usr/bin/env bash
set -Eeuo pipefail

PROJECT_ROOT=/opt/streamforge
RUNNER="$PROJECT_ROOT/scripts/benchmarks/run_throughput_test.sh"
RESULTS_ROOT=${BENCHMARK_RESULTS_ROOT:-/results}
DATASET_RECORDS=${BENCHMARK_DATASET_RECORDS:-10000}
PARTITIONS=${BENCHMARK_PARTITIONS:-8}
THREADS=${BENCHMARK_THREADS:-8}
REPETITIONS=${BENCHMARK_REPETITIONS:-3}

: "${BENCHMARK_RUN_ID:?BENCHMARK_RUN_ID is required}"
: "${BENCHMARK_ARTIFACT_BUCKET:?BENCHMARK_ARTIFACT_BUCKET is required}"
: "${BENCHMARK_GIT_SHA:?BENCHMARK_GIT_SHA is required}"
: "${AWS_REGION:?AWS_REGION is required}"

export BENCHMARK_GIT_DIRTY=false
export BENCHMARK_RESULTS_ROOT="$RESULTS_ROOT"
export BENCHMARK_DURATION_SECONDS=${BENCHMARK_DURATION_SECONDS:-120}
export BENCHMARK_WARMUP_MESSAGES=${BENCHMARK_WARMUP_MESSAGES:-1000000}
export BENCHMARK_INGRESS_WORKERS=${BENCHMARK_INGRESS_WORKERS:-4}
export BENCHMARK_INGRESS_TARGET_RATE=${BENCHMARK_INGRESS_TARGET_RATE:-0}
export BENCHMARK_PROCESSING_MODE=${BENCHMARK_PROCESSING_MODE:-partition_ordered}
export BENCHMARK_DELIVERY_MODE=${BENCHMARK_DELIVERY_MODE:-queued}

upload_results() {
    local benchmark_status=$1
    local upload_status=0
    local destination="s3://${BENCHMARK_ARTIFACT_BUCKET}/${BENCHMARK_RUN_ID}/"
    trap - EXIT
    printf '%s\n' "$benchmark_status" >"$RESULTS_ROOT/exit-status.txt"
    aws s3 cp "$RESULTS_ROOT/" "$destination" \
        --recursive \
        --only-show-errors \
        --sse AES256 \
        --region "$AWS_REGION" || upload_status=$?
    if [[ "$benchmark_status" -ne 0 ]]; then
        exit "$benchmark_status"
    fi
    exit "$upload_status"
}

mkdir -p "$RESULTS_ROOT"
trap 'upload_results "$?"' EXIT

"$RUNNER" \
    "$DATASET_RECORDS" \
    "$PARTITIONS" \
    "$THREADS" \
    "$REPETITIONS"
