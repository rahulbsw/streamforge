# Redpanda Demo

Use this directory when you want a local StreamForge demo without Kubernetes.

## Files

- `docker-compose.yml` - single-node Redpanda
- `selective-replication.yaml` - analytics payload + PII-safe summary pipeline

## Run

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml \
  cargo run --release --bin streamforge
```

The `pii-safe-orders` destination keeps raw email out of the value payload and uses a hashed email as the record key.
