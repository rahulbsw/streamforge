# Redpanda Demo

Use this directory when you want the fastest local StreamForge demo without Kubernetes.

## Files

- `docker-compose.yml` - single-node Redpanda
- `selective-replication.yaml` - analytics + PII-safe demo pipeline

## Run

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml ./target/release/streamforge
```
