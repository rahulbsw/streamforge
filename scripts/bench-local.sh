#!/usr/bin/env bash
# bench-local.sh — Run end-to-end StreamForge benchmarks on your local machine
#
# Prerequisites:
#   - Docker (for Kafka)
#   - Rust toolchain (cargo build --release)
#   - Python 3 (for data generation and reporting)
#
# Usage:
#   ./scripts/bench-local.sh                    # default: 100K messages, 8 threads
#   ./scripts/bench-local.sh -m 500000          # 500K messages
#   ./scripts/bench-local.sh -t 4               # 4 threads
#   ./scripts/bench-local.sh -m 100000 -t 8 -p 8  # messages, threads, partitions
#   ./scripts/bench-local.sh --dsl-only         # skip end-to-end, only run cargo bench
#   ./scripts/bench-local.sh --no-kafka         # skip tests that need Kafka

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
MESSAGES=100000
THREADS=8
PARTITIONS=8
DSL_ONLY=false
NO_KAFKA=false
KAFKA_CONTAINER="bench-kafka"
STREAMFORGE_BIN="./target/release/streamforge"
TMP_DIR=$(mktemp -d)
RESULTS_DIR="./benchmarks/results/local-$(date +%Y%m%d-%H%M%S)"

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

log()  { echo -e "${CYAN}▶${RESET} $*"; }
ok()   { echo -e "${GREEN}✓${RESET} $*"; }
warn() { echo -e "${YELLOW}⚠${RESET} $*"; }
err()  { echo -e "${RED}✗${RESET} $*" >&2; }
hdr()  { echo -e "\n${BOLD}── $* ──${RESET}"; }

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case $1 in
    -m|--messages)   MESSAGES="$2";   shift 2;;
    -t|--threads)    THREADS="$2";    shift 2;;
    -p|--partitions) PARTITIONS="$2"; shift 2;;
    --dsl-only)      DSL_ONLY=true;   shift;;
    --no-kafka)      NO_KAFKA=true;   shift;;
    -h|--help)
      echo "Usage: $0 [-m messages] [-t threads] [-p partitions] [--dsl-only] [--no-kafka]"
      exit 0;;
    *) err "Unknown argument: $1"; exit 1;;
  esac
done

mkdir -p "$RESULTS_DIR"

echo -e "${BOLD}"
echo "  ╔═══════════════════════════════════════╗"
echo "  ║    StreamForge Local Benchmark        ║"
echo "  ╚═══════════════════════════════════════╝"
echo -e "${RESET}"
echo "  Messages   : $MESSAGES"
echo "  Threads    : $THREADS"
echo "  Partitions : $PARTITIONS"
echo "  Results    : $RESULTS_DIR"
echo ""

# ── Cleanup on exit ───────────────────────────────────────────────────────────
cleanup() {
  pkill -f "streamforge" 2>/dev/null || true
  docker rm -f "$KAFKA_CONTAINER" 2>/dev/null || true
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

# ══════════════════════════════════════════════════════════════════════════════
# PART 1 — Rhai DSL micro-benchmarks (no Kafka)
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 1: Rhai DSL Micro-benchmarks"

log "Building benchmarks..."
cargo bench --no-run --bench filter_benchmarks --bench transform_benchmarks 2>/dev/null

log "Running filter benchmarks..."
cargo bench --bench filter_benchmarks 2>&1 | tee "$RESULTS_DIR/criterion-filter.txt"

log "Running transform benchmarks..."
cargo bench --bench transform_benchmarks 2>&1 | tee "$RESULTS_DIR/criterion-transform.txt"

ok "Criterion results saved to $RESULTS_DIR"

if $DSL_ONLY; then
  echo ""
  ok "DSL benchmarks complete (--dsl-only mode, skipping end-to-end tests)"
  exit 0
fi

# ══════════════════════════════════════════════════════════════════════════════
# PART 2 — Build release binary
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 2: Build release binary"

log "Building streamforge (release)..."
cargo build --release 2>&1 | tail -3
ok "Binary ready: $STREAMFORGE_BIN"

if $NO_KAFKA; then
  warn "--no-kafka: skipping end-to-end tests"
  exit 0
fi

# ══════════════════════════════════════════════════════════════════════════════
# PART 3 — Start local Kafka (KRaft, no Zookeeper)
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 3: Start local Kafka"

if ! command -v docker &>/dev/null; then
  err "Docker not found. Install Docker or run with --no-kafka"
  exit 1
fi

# Stop any leftover container
docker rm -f "$KAFKA_CONTAINER" 2>/dev/null || true

log "Starting Kafka (KRaft mode)..."
docker run -d \
  --name "$KAFKA_CONTAINER" \
  -p 9092:9092 \
  -e KAFKA_NODE_ID=1 \
  -e KAFKA_PROCESS_ROLES=broker,controller \
  -e KAFKA_LISTENERS="PLAINTEXT://0.0.0.0:9092,CONTROLLER://0.0.0.0:9093" \
  -e KAFKA_ADVERTISED_LISTENERS="PLAINTEXT://localhost:9092" \
  -e KAFKA_CONTROLLER_QUORUM_VOTERS="1@localhost:9093" \
  -e KAFKA_CONTROLLER_LISTENER_NAMES=CONTROLLER \
  -e KAFKA_LISTENER_SECURITY_PROTOCOL_MAP="PLAINTEXT:PLAINTEXT,CONTROLLER:PLAINTEXT" \
  -e KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1 \
  -e KAFKA_AUTO_CREATE_TOPICS_ENABLE=true \
  -e CLUSTER_ID="local-bench-cluster-001" \
  confluentinc/cp-kafka:7.6.0 >/dev/null

log "Waiting for Kafka..."
for i in $(seq 1 30); do
  docker exec "$KAFKA_CONTAINER" kafka-topics \
    --bootstrap-server localhost:9092 --list >/dev/null 2>&1 && \
    { ok "Kafka ready (${i}s)"; break; }
  [ "$i" -eq 30 ] && { err "Kafka failed to start"; exit 1; }
  sleep 2
done

# Create topics
log "Creating topics ($PARTITIONS partitions each)..."
for topic in \
  bench-passthrough-in bench-passthrough-out \
  bench-rhai-in bench-rhai-active bench-rhai-archive \
  bench-multi-in bench-multi-premium bench-multi-standard bench-multi-dlq; do
  docker exec "$KAFKA_CONTAINER" kafka-topics \
    --create --if-not-exists \
    --topic "$topic" \
    --partitions "$PARTITIONS" \
    --replication-factor 1 \
    --bootstrap-server localhost:9092 2>/dev/null
done
ok "Topics created"

# ══════════════════════════════════════════════════════════════════════════════
# PART 4 — Generate test data
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 4: Generate test data ($MESSAGES messages)"

python3 - "$MESSAGES" "$TMP_DIR/test_data.jsonl" << 'PYEOF'
import json, random, time, sys

count = int(sys.argv[1])
output = sys.argv[2]
tiers = ["free", "standard", "premium", "enterprise"]
events = ["page.view", "item.added", "checkout.started", "purchase.completed", "login"]

with open(output, "w") as f:
    for i in range(count):
        f.write(json.dumps({
            "userId":    f"usr-{i:06d}",
            "email":     f"User{i}@Example.COM",
            "tier":      random.choice(tiers),
            "status":    "active" if random.random() > 0.1 else "suspended",
            "eventType": random.choice(events),
            "amount":    round(random.uniform(1, 2000), 2),
            "score":     random.randint(0, 100),
            "timestamp": int(time.time() * 1000) - random.randint(0, 120000),
        }) + "\n")

print(f"Generated {count:,} messages")
PYEOF
ok "Test data ready"

# ── Helper: run one scenario ──────────────────────────────────────────────────
run_scenario() {
  local name="$1" config_file="$2" input_topic="$3" metrics_port="$4"
  local sf_pid

  log "Starting StreamForge for scenario: $name"
  CONFIG_FILE="$config_file" "$STREAMFORGE_BIN" \
    > "$RESULTS_DIR/sf-${name}.log" 2>&1 &
  sf_pid=$!
  sleep 2

  local t_start t_end elapsed throughput consumed p99

  log "Producing $MESSAGES messages..."
  t_start=$(python3 -c "import time; print(int(time.time()*1000))")
  cat "$TMP_DIR/test_data.jsonl" | docker exec -i "$KAFKA_CONTAINER" \
    kafka-console-producer \
    --bootstrap-server localhost:9092 \
    --topic "$input_topic" \
    --batch-size 5000 \
    --request-required-acks 1 2>/dev/null

  log "Waiting for all messages to be consumed..."
  for i in $(seq 1 90); do
    consumed=$(curl -s "http://localhost:${metrics_port}/metrics" \
      | grep "^streamforge_messages_consumed_total " \
      | awk '{printf "%.0f", $2}' 2>/dev/null || echo 0)
    printf "\r  [%3ds] %s / %s consumed" "$((i*2))" "${consumed:-0}" "$MESSAGES"
    [ "${consumed:-0}" -ge "$MESSAGES" ] && break
    sleep 2
  done
  echo ""

  t_end=$(python3 -c "import time; print(int(time.time()*1000))")
  elapsed=$(( (t_end - t_start) ))
  consumed=$(curl -s "http://localhost:${metrics_port}/metrics" \
    | grep "^streamforge_messages_consumed_total " \
    | awk '{printf "%.0f", $2}')
  throughput=$(python3 -c "print(int(${consumed:-0} * 1000 / max(${elapsed},1)))")
  p99=$(curl -s "http://localhost:${metrics_port}/metrics" \
    | grep 'processing_duration_seconds{.*quantile="0.99"' \
    | awk '{printf "%.2f", $2 * 1000000}' | head -1)

  # Save metrics
  curl -s "http://localhost:${metrics_port}/metrics" \
    > "$RESULTS_DIR/metrics-${name}.txt" 2>/dev/null

  kill "$sf_pid" 2>/dev/null; sleep 2

  printf "  %-40s %8s msg/s   p99=%s µs\n" \
    "$name" "$throughput" "${p99:-n/a}"

  # Store for summary
  echo "${name}|${consumed}|$((elapsed/1000))|${throughput}|${p99:-n/a}" \
    >> "$RESULTS_DIR/summary.csv"
}

# ══════════════════════════════════════════════════════════════════════════════
# PART 5 — Scenario A: Passthrough baseline
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 5: Scenario A — Passthrough (baseline)"

cat > "$TMP_DIR/config-a.yaml" << EOF
appid: bench-passthrough
bootstrap: localhost:9092
input: bench-passthrough-in
output: bench-passthrough-out
threads: $THREADS
offset: earliest
observability:
  metrics_enabled: true
  metrics_port: 9091
  lag_monitoring_enabled: false
EOF

run_scenario "passthrough" "$TMP_DIR/config-a.yaml" \
  "bench-passthrough-in" "9091"

# ══════════════════════════════════════════════════════════════════════════════
# PART 6 — Scenario B: Rhai filter + transform
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 6: Scenario B — Rhai filter + transform"

cat > "$TMP_DIR/config-b.yaml" << EOF
appid: bench-rhai
bootstrap: localhost:9092
input: bench-rhai-in
threads: $THREADS
offset: earliest
observability:
  metrics_enabled: true
  metrics_port: 9092
  lag_monitoring_enabled: false

routing:
  destinations:
    - output: bench-rhai-active
      filter:
        - 'msg["status"] == "active"'
        - 'msg["score"] > 50'
        - 'not_null(msg["userId"])'
      transform: |
        #{
          id:        msg["userId"],
          tier:      msg["tier"].to_upper(),
          email:     msg["email"].to_lower(),
          amount:    msg["amount"] * 1.08,
          processed: true,
          ts:        now_ms()
        }
    - output: bench-rhai-archive
      transform: 'msg + #{ archived: true }'
EOF

run_scenario "rhai-filter-transform" "$TMP_DIR/config-b.yaml" \
  "bench-rhai-in" "9092"

# ══════════════════════════════════════════════════════════════════════════════
# PART 7 — Scenario C: Multi-destination + cache
# ══════════════════════════════════════════════════════════════════════════════
hdr "Part 7: Scenario C — Multi-destination + cache enrichment"

cat > "$TMP_DIR/config-c.yaml" << EOF
appid: bench-multi
bootstrap: localhost:9092
input: bench-multi-in
threads: $THREADS
offset: earliest
observability:
  metrics_enabled: true
  metrics_port: 9093
  lag_monitoring_enabled: false

cache:
  backend_type: local
  local:
    max_capacity: 100000
    ttl_seconds: 3600

routing:
  destinations:
    - output: bench-multi-premium
      filter:
        - 'msg["tier"] in ["premium", "enterprise"]'
        - 'msg["status"] == "active"'
      transform: |
        cache_put("seen", msg["userId"], msg["tier"]);
        msg + #{ tier: msg["tier"].to_upper(), amount: msg["amount"] * 1.08 }

    - output: bench-multi-standard
      filter: 'msg["tier"] in ["free", "standard"] && msg["status"] == "active"'
      transform: |
        let prev = cache_lookup("seen", msg["userId"]);
        msg + #{ upgradable: prev == () }

    - output: bench-multi-dlq
      filter: 'msg["status"] != "active"'
EOF

run_scenario "multi-dest-cache" "$TMP_DIR/config-c.yaml" \
  "bench-multi-in" "9093"

# ══════════════════════════════════════════════════════════════════════════════
# PART 8 — Print summary
# ══════════════════════════════════════════════════════════════════════════════
hdr "Benchmark Results"

python3 - "$RESULTS_DIR/summary.csv" "$MESSAGES" "$THREADS" << 'PYEOF'
import sys, csv

csv_file = sys.argv[1]
messages = int(sys.argv[2])
threads  = int(sys.argv[3])

rows = []
try:
    for line in open(csv_file):
        name, consumed, duration, throughput, p99 = line.strip().split("|")
        rows.append({
            "name": name, "consumed": int(consumed),
            "duration": int(duration), "throughput": int(throughput), "p99": p99
        })
except Exception as e:
    print(f"Could not parse results: {e}")
    sys.exit(0)

baseline = next((r["throughput"] for r in rows if "passthrough" in r["name"]), None)

print(f"\n{'─'*70}")
print(f"  StreamForge Benchmark Results")
print(f"  {messages:,} messages · {threads} threads")
print(f"{'─'*70}")
print(f"  {'Scenario':<40} {'Throughput':>12}  {'vs baseline':>12}  {'p99':>10}")
print(f"{'─'*70}")

for r in rows:
    vs = ""
    if baseline and r["throughput"] > 0:
        pct = (r["throughput"] / baseline) * 100
        vs = f"{pct:.0f}%"
    p99_str = f"{r['p99']} µs" if r['p99'] != 'n/a' else "—"
    print(f"  {r['name']:<40} {r['throughput']:>10,}/s  {vs:>12}  {p99_str:>10}")

print(f"{'─'*70}")

if baseline:
    rhai_row = next((r for r in rows if "rhai" in r["name"]), None)
    if rhai_row:
        overhead_pct = 100 - (rhai_row["throughput"] / baseline * 100)
        print(f"\n  Rhai DSL overhead vs passthrough: {overhead_pct:.1f}%")
        print(f"  (filter + transform evaluation at {messages:,} msgs)")

print()
PYEOF

echo ""
ok "Results saved to: $RESULTS_DIR"
echo ""
echo "  Files:"
echo "    $RESULTS_DIR/criterion-filter.txt    — DSL filter micro-benchmarks"
echo "    $RESULTS_DIR/criterion-transform.txt — DSL transform micro-benchmarks"
echo "    $RESULTS_DIR/metrics-*.txt            — Raw Prometheus metrics per scenario"
echo "    $RESULTS_DIR/sf-*.log                 — StreamForge logs per scenario"
echo "    $RESULTS_DIR/summary.csv              — Throughput summary"
echo ""
