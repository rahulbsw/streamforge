---
title: Usage Guide
nav_order: 3
---

# Usage Guide

StreamForge is strongest when you want to move a selected subset of Kafka data into downstream systems that need a different shape, a lower-trust payload, or a different topic layout.

This guide focuses on the high-value usage patterns that match the current product position: selective replication, shaping, redaction, and routing. It does not treat StreamForge as a full active-active mirroring tool or a general stream processing engine.

## Start with a Validated Example

If you want a working starting point instead of building from scratch:

- [QUICKSTART.md](QUICKSTART.md) for the five-minute local demo
- [EXAMPLES.md](EXAMPLES.md) for validated example packs
- [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) for the configuration structure
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) for filter and transform expressions

## Core Usage Patterns

### Filtered Replication to Analytics or Lake

Use StreamForge when an operational topic contains more fields than downstream analytics systems should receive.

Typical flow:
- consume an application topic
- filter to the event types you actually want downstream
- project only the analytics-safe fields
- publish to a dedicated analytics topic or cluster

Good fit:
- CDC or event streams feeding a warehouse or lake
- domain event cleanup before analytics ingestion
- splitting operational and analytical contracts

See:
- [examples/production/cdc-to-datalake.yaml](../examples/production/cdc-to-datalake.yaml)
- [QUICKSTART.md](QUICKSTART.md)

### PII-Safe Replication Across Trust Boundaries

Use StreamForge when the destination system should receive business events but not raw identifiers or sensitive fields.

Typical flow:
- keep only approved fields
- hash, mask, or drop sensitive values
- publish to a downstream topic with a tighter contract

Good fit:
- staging or partner environments
- lower-trust analytics consumers
- internal topics that should not expose raw customer data

See:
- [examples/production/pii-redaction.yaml](../examples/production/pii-redaction.yaml)
- [SECURITY_CONFIGURATION.md](SECURITY_CONFIGURATION.md)

### Topic Fan-Out with Consumer-Specific Shapes

Use one input topic with multiple outputs when different consumers need different subsets or payload shapes.

Typical flow:
- read one source topic
- apply per-destination filters
- publish consumer-specific payloads to separate topics

Good fit:
- one source topic feeding operations, analytics, and audits
- service-specific integration topics
- separating high-volume raw streams from narrower downstream contracts

### Derived Metrics with Aggregations

Use an `aggregation:` block inside a routing destination when the downstream system needs a compact metrics topic instead of every raw event.

Typical flow:
- filter to the events that should count toward the rollup
- reshape the value so aggregation reads a stable payload
- emit tumbling-window metrics to a dedicated Kafka topic

Good fit:
- per-region or per-tenant rollups for analytics
- low-overhead operational metrics streams derived from business events
- approximate distinct counts or quantile summaries without standing up a separate streaming stack

Boundaries:
- aggregations run after the destination filter and value transform
- aggregated outputs go to the destination `output` topic
- windowing is processing-time and in-memory in v1
- `emit_interval_seconds` controls how often StreamForge checks for completed windows
- `commit_strategy.manual_commit: true` is not supported for aggregation destinations
- this mode is for lightweight rollups, not joins, SQL, sliding windows, session windows, or durable state

See:
- [AGGREGATIONS.md](AGGREGATIONS.md)
- [../examples/aggregation/orders-windowed-metrics.yaml](../examples/aggregation/orders-windowed-metrics.yaml)
- [../examples/aggregation/orders-quantiles.yaml](../examples/aggregation/orders-quantiles.yaml)

### Cross-Cluster Replication with Shaping

Use StreamForge when you need to move data between clusters but do not want to mirror whole topics unchanged.

Good fit:
- regional or environment replication with filtering
- topic migrations where downstream contracts are changing
- Redpanda or Kafka targets that only need a selected portion of the source stream

See:
- [COMPATIBILITY.md](COMPATIBILITY.md)
- [examples/redpanda/README.md](../examples/redpanda/README.md)

## Build a Pipeline

### 1. Choose the Source and Destinations

Define the input topic and decide whether you are publishing to:
- one destination topic
- multiple destination topics in the same cluster
- a different Kafka-compatible target cluster

### 2. Add Selection Logic

Use filters when only part of the source stream should move downstream.

Common selectors:
- event type
- region or tenant
- presence or value of a field
- metadata in key or headers

### 3. Shape the Payload

Use transforms to:
- keep only downstream-safe fields
- rename or restructure fields
- construct smaller consumer-specific payloads
- hash or drop sensitive data

### 4. Validate Before Running

Prefer validated YAML configs over ad hoc inline examples.

```bash
cargo run --quiet --bin streamforge-validate -- path/to/config.yaml
```

### 5. Deploy in the Right Mode

Use the standalone binary when you want the lightest operational path. Use the operator and Helm chart when you want pipelines managed as Kubernetes resources.

See:
- [DEPLOYMENT.md](DEPLOYMENT.md)
- [KUBERNETES.md](KUBERNETES.md)

## When StreamForge Is the Wrong Tool

Do not use StreamForge as your primary answer for:
- MirrorMaker 2 active-active replication
- consumer offset synchronization across clusters
- general SQL stream processing
- joins, session/sliding windows, and broad stateful event computation

That boundary is intentional. StreamForge is the selective replication and shaping layer. Heavier stateful analytics belongs in tools built for that purpose.

## Recommended Reading Order

1. [QUICKSTART.md](QUICKSTART.md)
2. [EXAMPLES.md](EXAMPLES.md)
3. [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md)
4. [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md)
5. [COMPATIBILITY.md](COMPATIBILITY.md)
