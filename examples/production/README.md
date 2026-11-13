# Production Configuration Examples

These are starting templates for common StreamForge deployments, not universal
capacity, latency, compliance, or resource guarantees. Validate the selected
file, review its comments and trade-offs, test it against representative data,
and replace every placeholder endpoint, topic, certificate path, credential
reference, image tag, and resource value before deployment.

| Configuration | Pattern | Primary trade-off |
| --- | --- | --- |
| [user-filtering.yaml](user-filtering.yaml) | Route users by status, tier, event, and region | Destination-specific filter and transform work |
| [cross-region-replication.yaml](cross-region-replication.yaml) | Disaster recovery or regional replication | WAN efficiency, ordering, and recovery point |
| [cdc-to-datalake.yaml](cdc-to-datalake.yaml) | Route Debezium-style create, update, delete, and schema events | Freshness versus batching |
| [multi-tenant-filtering.yaml](multi-tenant-filtering.yaml) | Route active tenants by tier and isolate selected customers | Isolation versus destination and pipeline count |
| [pii-redaction.yaml](pii-redaction.yaml) | Minimize or pseudonymize fields for destination-specific access | Stronger transforms and governance cost more CPU and operations |

## Validate

Build and run the repository validator against every template:

```bash
cargo build --release --locked --bin streamforge-validate

for config in examples/production/*.yaml
do
  target/release/streamforge-validate "$config" --fail-on-warnings
done
```

Validation checks the StreamForge configuration contract; it does not prove
broker reachability, topic permissions, certificate validity, destination
compatibility, throughput, recovery behavior, or regulatory compliance.

## Review before deployment

- Confirm source and destination brokers, topics, partition counts, keys, and
  ordering requirements.
- Choose batching, compression, fetch, thread, and commit settings from measured
  workload results. Less frequent commits can increase replay after failure.
- Confirm retry and DLQ retention, access control, replay, and alerting.
- Inject credentials through the approved secret store; never commit them.
- Use TLS/SASL and least-privilege Kafka ACLs. Treat hashing as pseudonymization,
  not encryption or automatic anonymization.
- Verify every filter and transform against missing, null, malformed, delete,
  and schema-change records expected from the producer.
- Load-test record-count equality, duplicate behavior, throughput, p95/p99
  latency, lag, CPU, and memory on the intended topology.

`cross-region-replication.yaml` contains a documented destination-security
integration constraint; resolve it for the deployment rather than assuming the
source `kafka` security block configures the destination producer.

## Deploy and operate

Use the maintained guides instead of copying deployment, monitoring, or secret
snippets from an example:

- [Deployment](../../docs/DEPLOYMENT.md)
- [Kubernetes](../../docs/KUBERNETES.md)
- [Podman](../../docs/DOCKER.md)
- [Configuration](../../docs/YAML_CONFIGURATION.md)
- [DSL reference](../../docs/ADVANCED_DSL_GUIDE.md)
- [Delivery guarantees](../../docs/DELIVERY_GUARANTEES.md)
- [Performance](../../docs/PERFORMANCE.md)
- [Operations](../../docs/OPERATIONS.md)
- [Troubleshooting](../../docs/TROUBLESHOOTING.md)
- [Security configuration](../../docs/SECURITY_CONFIGURATION.md)

Questions and reproducible defects belong in
[GitHub Issues](https://github.com/rahulbsw/streamforge/issues); general help
belongs in [GitHub Discussions](https://github.com/rahulbsw/streamforge/discussions).
