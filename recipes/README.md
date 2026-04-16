# StreamForge Recipes

Copy-paste configs for common Kafka pipeline patterns. Each recipe is a complete, working StreamForge config.

| Recipe | Description |
|---|---|
| [pii-masking.yaml](pii-masking.yaml) | GDPR-compliant PII redaction and hashing |
| [event-router.yaml](event-router.yaml) | Route events to per-type topics |
| [cache-enrichment.yaml](cache-enrichment.yaml) | Enrich messages from a lookup cache |
| [dlq-retry.yaml](dlq-retry.yaml) | Dead-letter queue with exponential backoff |
| [cross-cluster-mirror.yaml](cross-cluster-mirror.yaml) | Selective cross-cluster replication |
| [multi-cloud-fanout.yaml](multi-cloud-fanout.yaml) | Write to multiple clusters simultaneously |
| [schema-slim.yaml](schema-slim.yaml) | Strip unnecessary fields before forwarding |
| [envelope-ops.yaml](envelope-ops.yaml) | Key extraction, header injection, timestamp control |
| [topic-namespace-mirror.yaml](topic-namespace-mirror.yaml) | Mirror a whole topic namespace with prefix |
