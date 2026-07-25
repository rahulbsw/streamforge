---
title: Deployment
nav_order: 6
has_children: true
---

# Deploy StreamForge

StreamForge runs as a native binary, a container, or a Kubernetes workload. The
repository also includes a Kubernetes operator and a local Helm chart. Choose
the smallest deployment model that fits your operating environment.

## Choose a deployment model

| Model | Best for | Start here |
|---|---|---|
| Native binary | Development, diagnostics, and controlled hosts | This page |
| Container | A single managed pipeline or container platform | [Podman](DOCKER.md) |
| Kubernetes Deployment | Teams that manage application manifests directly | [Kubernetes](KUBERNETES.md) |
| Kubernetes operator | Multiple declarative `StreamforgePipeline` resources | [Kubernetes](KUBERNETES.md) |

The Helm chart in this repository is installed from a local checkout. The
documentation does not assume that a public chart repository or supported
prebuilt image is available.

## Production prerequisites

- Source and destination Kafka endpoints reachable from the StreamForge runtime
- Topics, partitions, replication, retention, and ACLs created by the Kafka
  administrator
- A configuration file validated against the same StreamForge revision that
  will be deployed
- Credentials supplied by the platform secret store, never committed with the
  configuration
- CPU, memory, and replica counts established with a representative load test
- Prometheus access to the metrics endpoint through a private network path

Kafka and Kubernetes version compatibility is documented in
[Compatibility](COMPATIBILITY.md).

## Build and validate

Build release binaries:

```bash
cargo build --release --locked \
  --bin streamforge \
  --bin streamforge-validate
```

Validate the configuration before starting a rollout:

```bash
target/release/streamforge-validate config.yaml --fail-on-warnings
```

Start StreamForge by passing the configuration path through `CONFIG_FILE`:

```bash
CONFIG_FILE=config.yaml RUST_LOG=info target/release/streamforge
```

StreamForge also accepts JSON configuration. The filename extension determines
whether the runtime loads YAML or JSON.

## Minimal pipeline

```yaml
appid: orders-replica
bootstrap: source-kafka.internal:9092
target_broker: destination-kafka.internal:9092
input: orders
output: orders-replica
offset: earliest
threads: 4

commit_strategy:
  manual_commit: true
  commit_mode: sync

observability:
  metrics_enabled: true
  metrics_port: 9090
  lag_monitoring_enabled: true
```

Use [Security](SECURITY_CONFIGURATION.md) to add TLS or SASL. Review
[Delivery guarantees](DELIVERY_GUARANTEES.md) before selecting a commit or
producer delivery mode.

## Network exposure

The metrics server listens on all interfaces when enabled and serves
`/metrics` and `/health` on the configured port. It does not provide
authentication or TLS.

Do not expose that port directly to the public internet. Restrict access with a
host firewall, container network, Kubernetes `ClusterIP` service and
`NetworkPolicy`, or a private load balancer. If remote access is required,
terminate authentication and TLS in a trusted private proxy.

Kafka listeners should likewise remain private wherever possible. Limit egress
to the required broker addresses and DNS, and grant only the topic and consumer
group permissions used by the pipeline.

## Rollout sequence

1. Validate the configuration and confirm the source and destination topics.
2. Deploy one pipeline instance with production delivery settings.
3. Verify `/health`, broker connectivity, destination acknowledgements, error
   counters, and consumer lag.
4. Produce a controlled test record and verify its key, value, headers,
   timestamp, and destination.
5. Increase replicas only up to the useful source-partition parallelism.
6. Observe at least one consumer-group rebalance and a clean shutdown before
   declaring the rollout complete.
7. Record the deployed image digest, configuration revision, and rollback
   procedure.

## Updates and rollback

Treat configuration and image changes independently:

- validate a new configuration before rollout;
- use immutable image tags or digests;
- retain the previous configuration and image reference;
- roll instances gradually to limit consumer-group churn;
- watch destination errors and lag throughout the change;
- roll back both image and configuration when their compatibility is uncertain.

Changing `appid` creates a different Kafka consumer group. Changing `offset`
only affects partitions without a committed offset for that group. Resetting
offsets can intentionally replay or skip records and should be performed only
with the pipeline stopped and an approved recovery plan.

## Readiness checklist

- Configuration validation passes without unreviewed warnings.
- Kafka TLS/SASL material is mounted read-only from a secret store.
- Metrics and health endpoints are private.
- Delivery mode and duplicate-handling expectations are documented.
- Source and destination topic capacity and partitioning are verified.
- Resource requests and limits come from measurements of the target workload.
- Consumer lag, delivery failures, processing errors, and restarts are alerted.
- Rollback and offset-recovery procedures have been tested.

Continue with [Operations](OPERATIONS.md) for day-two procedures and
[Troubleshooting](TROUBLESHOOTING.md) for incident diagnosis.
