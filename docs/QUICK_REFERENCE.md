# StreamForge - Quick Reference Card

## Installation

```bash
# Build from source
cargo build --release

# Or use Docker
docker pull streamforge:latest
```

## Running

```bash
# Direct
CONFIG_FILE=config.json ./target/release/streamforge

# Docker
docker run -v $(pwd)/config.json:/app/config/config.json:ro streamforge

# With logging
RUST_LOG=debug CONFIG_FILE=config.json ./streamforge
```

## Basic Configuration

**YAML (Recommended):**
```yaml
appid: mirrormaker
bootstrap: source-kafka:9092
target_broker: target-kafka:9092
input: source-topic
output: destination-topic
threads: 4
```

**JSON (Also Supported):**
```json
{
  "appid": "mirrormaker",
  "bootstrap": "source-kafka:9092",
  "target_broker": "target-kafka:9092",
  "input": "source-topic",
  "output": "destination-topic",
  "threads": 4
}
```

**Auto-detected** by file extension (`.yaml`, `.yml`, `.json`)

## Rhai DSL Syntax Reference

StreamForge uses **Rhai** - a JavaScript-like scripting language - for filters and transforms.

### Filters (return true/false)

| Syntax | Example | Description |
|--------|---------|-------------|
| Simple | `msg["field"] op value` | `msg["id"] > 1000` |
| Boolean AND | `cond1 && cond2` | `msg["id"] > 0 && msg["status"] == "active"` |
| Boolean OR | `cond1 \|\| cond2` | `msg["priority"] == "high" \|\| msg["urgent"]` |
| Boolean NOT | `!condition` | `!msg["test"]` |
| Regex | `field.matches("pattern")` | `msg["email"].matches("^[\\w]+@.*")` |
| Array All | `array.all(\|x\| condition)` | `msg["users"].all(\|u\| u["active"])` |
| Array Any | `array.any(\|x\| condition)` | `msg["users"].any(\|u\| u["admin"])` |

### Operators

**Comparison**: `==` `!=` `>` `>=` `<` `<=`  
**Logical**: `&&` (AND)  `||` (OR)  `!` (NOT)  
**Arithmetic**: `+` `-` `*` `/` `%`

### Transforms (return new value)

| Syntax | Example | Description |
|--------|---------|-------------|
| Extract | `msg["path"]` | `msg["user"]["id"]` |
| Object | `#{ f1: v1, f2: v2 }` | `#{ id: msg["id"], name: msg["name"] }` |
| Array Map | `array.map(\|x\| expr)` | `msg["users"].map(\|u\| u["id"])` |
| Arithmetic | `expr op expr` | `msg["price"] + msg["tax"]` |
| String Ops | `string.method()` | `msg["email"].to_lower()` |

### String Methods

`to_upper()` `to_lower()` `trim()` `split()` `replace()` `starts_with()` `ends_with()` `contains()` `matches()`

## Configuration Patterns

### Simple Mirror

```json
{
  "bootstrap": "kafka-a:9092",
  "target_broker": "kafka-b:9092",
  "input": "events",
  "output": "events-mirror"
}
```

### Content Routing

```yaml
input: events
routing:
  destinations:
    - output: user-events
      filter: 'msg["type"].starts_with("user")'
    
    - output: order-events
      filter: 'msg["type"].starts_with("order")'
```

### Data Validation

```yaml
routing:
  destinations:
    - output: valid
      filter: 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
      transform: 'msg["user"]'
    
    - output: invalid
      filter: '!msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
```

### Calculate Metrics

```yaml
routing:
  destinations:
    - output: with-tax
      filter: 'msg["price"] > 0'
      transform: 'msg["price"] * 1.08'
```

## Performance Tuning

### High Throughput

```json
{
  "threads": 8,
  "consumer_properties": {
    "fetch.min.bytes": "1048576",
    "max.poll.records": "1000"
  },
  "producer_properties": {
    "batch.size": "131072",
    "linger.ms": "10",
    "compression.type": "snappy"
  }
}
```

### Low Latency

```json
{
  "threads": 4,
  "consumer_properties": {
    "fetch.min.bytes": "1",
    "fetch.wait.max.ms": "0"
  },
  "producer_properties": {
    "batch.size": "16384",
    "linger.ms": "0"
  }
}
```

## Monitoring

### View Logs

```bash
# Real-time
docker logs -f mirrormaker

# With timestamps
docker logs -f --timestamps mirrormaker

# Last 100 lines
docker logs --tail 100 mirrormaker
```

### Metrics

Application reports every 10 seconds:
```
Stats: processed=10000 (1000.0/s), filtered=100 (10.0/s),
       completed=9900 (990.0/s), errors=0 (0.0/s)
```

### Check Consumer Lag

```bash
kafka-consumer-groups.sh \
  --bootstrap-server kafka:9092 \
  --group streamforge \
  --describe
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs mirrormaker

# Run interactively
docker run --rm -it -v $(pwd)/config.json:/app/config/config.json:ro streamforge
```

### No Messages Flowing

```bash
# Enable debug logging
RUST_LOG=debug docker run ...

# Check filter logic
# Add catchall destination
{
  "destinations": [{"output": "debug-topic"}]
}
```

### High CPU Usage

```bash
# Check thread count
# Reduce threads or simplify filters

# Profile
perf record -p $(pgrep streamforge)
```

### High Memory Usage

```bash
# Reduce batch sizes
"consumer_properties": {
  "max.poll.records": "100"
}
```

## Benchmarking

```bash
# Run benchmarks
cargo bench

# Or use script
./run-benchmarks.sh

# View results
open target/criterion/report/index.html
```

## Testing

```bash
# All tests
cargo test

# Specific test
cargo test test_array_filter

# With output
cargo test -- --nocapture
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Throughput | 25K+ msg/s |
| Latency p99 | <15ms |
| Memory | <200MB |
| CPU | <400% (4 cores) |

## Common Patterns

### Filter by Field Value

```yaml
filter: 'msg["status"] == "active"'
```

### Filter with Boolean Logic

```yaml
filter: 'msg["status"] == "active" && msg["tier"] == "premium"'
```

### Extract Nested Field

```yaml
transform: 'msg["user"]["profile"]["email"]'
```

### Build New Object

```yaml
transform: |
  #{
    id: msg["user"]["id"],
    email: msg["user"]["email"]
  }
```

### Calculate Total

```yaml
transform: 'msg["price"] + msg["tax"]'
```

### Extract Array IDs

```yaml
transform: 'msg["users"].map(|u| u["id"])'
```

## Compression Options

| Type | Speed | Ratio | Use Case |
|------|-------|-------|----------|
| none | Fastest | 1.0x | CPU limited |
| snappy | Fast | 2.5x | Balanced |
| gzip | Slow | 4.0x | Bandwidth limited |
| zstd | Medium | 4.5x | Best overall |

## Resource Limits

### Docker

```bash
docker run -d \
  --cpus="4" \
  --memory="512m" \
  mirrormaker
```

### Kubernetes

```yaml
resources:
  limits:
    cpu: "4000m"
    memory: "512Mi"
  requests:
    cpu: "1000m"
    memory: "256Mi"
```

## Documentation Links

- [QUICKSTART.md](QUICKSTART.md) - Get started
- [USAGE.md](USAGE.md) - Use cases
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - DSL reference
- [PERFORMANCE.md](PERFORMANCE.md) - Tuning guide
- [DOCKER.md](DOCKER.md) - Deployment
- [CONTRIBUTING.md](CONTRIBUTING.md) - Development

## Performance Cheat Sheet

### Filter Performance

- Simple: ~100ns
- AND/OR: ~300ns
- Regex: ~500ns
- Array: ~5µs

### Transform Performance

- Extract: ~50ns
- Construct: ~500ns
- Array Map: ~5µs
- Arithmetic: ~50ns

### Optimization Tips

1. ✅ Use simple filters when possible
2. ✅ Put fast filters first in AND
3. ✅ Avoid complex regex
4. ✅ Limit array operations on large arrays
5. ✅ Use snappy compression for balance
6. ✅ Match threads to CPU cores
7. ✅ Tune batch size for latency/throughput
8. ✅ Monitor consumer lag

---

**Quick Help**: `RUST_LOG=debug ./streamforge --help`
