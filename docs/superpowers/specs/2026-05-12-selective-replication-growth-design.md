# StreamForge Selective Replication Growth Design

Date: 2026-05-12
Status: Approved in brainstorming
Primary owner: StreamForge project

## Summary

StreamForge should position itself as a selective replication engine for Kafka and Redpanda. The public story is not "a better MirrorMaker 2 in every way" and not "a general-purpose streaming engine." The winning wedge is selective cross-topic and cross-cluster movement of only the records and fields downstream systems actually need, with filtering, transformation, redaction, routing, and a small operational footprint.

The primary initial audience is data engineering teams. The primary hero use case is filtered replication into analytics and lake pipelines. Secondary use cases remain PII-safe replication, topic fan-out for downstream applications, and cross-cluster migration or disaster recovery.

## Goals

- Establish a public product identity that is clear in under 30 seconds.
- Make the repository, website, and docs look credible, focused, and easy to evaluate.
- Differentiate from MirrorMaker 2 on selective replication and data shaping.
- Stay complementary to Arroyo rather than competing as a full stateful stream processor.
- Build a roadmap that supports adoption toward 500 GitHub stars.
- Add lightweight aggregation in a way that strengthens the selective replication story.

## Non-Goals

- Claim feature superiority over MirrorMaker 2 in every replication scenario.
- Reposition StreamForge as a full SQL stream processing system.
- Compete directly with Arroyo, Flink, or Kafka Streams on joins, generalized stateful DAGs, or rich query execution.
- Introduce broad product scope that weakens the selective replication story.

## Product Identity

### One-line positioning

StreamForge is a selective replication engine for Kafka and Redpanda that filters, transforms, redacts, and routes data between topics and clusters with a small operational footprint.

### Product promise

Move only the data you want, already shaped for the downstream consumer, without standing up Kafka Connect or a full stream processing platform.

### Primary audience

Data engineering teams that need to move filtered or reduced data from operational Kafka topics into analytics, lakes, downstream topics, or lower-trust environments.

### Primary hero use case

Filtered replication to analytics and lake pipelines.

### Secondary use cases

- PII-safe replication across trust boundaries
- Topic fan-out and event shaping for downstream consumers
- Cross-cluster migration and disaster recovery for selected data

## Market Positioning

### Position against MirrorMaker 2

The project should say:

- StreamForge is more capable than MirrorMaker 2 for selective replication, shaping, redaction, and routing.
- StreamForge is simpler to deploy when teams do not want Kafka Connect.
- StreamForge is smaller and faster to start when teams want a focused data movement tool.

The project should not say:

- StreamForge is better than MirrorMaker 2 for every replication use case.

MirrorMaker 2 remains stronger for active-active replication, consumer offset synchronization, and full cluster mirroring workflows. StreamForge wins when the requirement is to move only a chosen subset of data and reshape it on the way out.

### Position against Arroyo

The complement story should be explicit:

- StreamForge prepares, filters, redacts, and routes streams.
- Arroyo is the better fit for richer stateful analytics, SQL, joins, windows, and exactly-once processing across broader streaming workloads.

This boundary prevents category confusion and keeps StreamForge focused.

## Compatibility Strategy

### Kafka

Kafka remains the primary compatibility baseline and deployment target.

### Redpanda

Redpanda should be treated as an explicit supported target rather than an implied "Kafka-compatible" afterthought.

Required outcomes:

- Add Redpanda to the README hero badges and compatibility section.
- Add tested local examples using Redpanda.
- Add CI or release validation coverage against Redpanda where practical.
- Document any unsupported or behavior-sensitive Kafka features if they appear.

## Capability Strategy

### Core capability lane

The core product lane is selective replication:

- cluster-to-cluster and topic-to-topic movement
- content-based filtering
- transformation and field shaping
- redaction and hashing for sensitive data
- multi-destination routing
- repartitioning and envelope manipulation
- operator, Helm, and UI support for production deployment

### Production usability lane

Operational simplicity should be a competitive advantage:

- validation CLI
- dry-run and preflight checks
- config preview in UI and CRD workflows
- pipeline diffing for updates
- strong observability and delivery guarantee documentation

### Analytics lane

Live aggregation should be intentionally narrow and derived-stream oriented.

#### Phase 1

- keyed tumbling windows
- `sum`
- `count`
- `avg`

#### Phase 2

- sketch-backed approximate distinct counts
- quantile and percentile summaries
- frequent item or heavy-hitter summaries

#### Product boundary

Aggregation outputs should be emitted back to Kafka topics. The feature should not expand into general SQL, joins, arbitrary query planning, or a full stateful stream processing model.

## DataSketches Strategy

The Rust `datasketches` crate should be adopted as a narrow analytics extension, not as a reason to reposition the product.

Recommended initial functions:

- `approx_distinct`
- `quantiles`
- `top_k` or frequent-items style summaries if supported cleanly

Recommended execution model:

- keyed in-memory or pluggable state per aggregation stream
- simple window boundaries first
- CRD and DSL configuration that maps directly to Kafka topic outputs

Recommended messaging:

- "lightweight derived metrics"
- "approximate summaries on the replication path"

Messaging to avoid:

- "full stream processing"
- "distributed analytics engine"

## Public-Facing Experience

### Visual direction

The public face should use a product-led information architecture with a darker, operator-grade visual theme. The project should feel like serious data infrastructure while still leading with data engineer outcomes.

### Homepage and README narrative

The first screen should answer:

- What is StreamForge?
- Why would a data engineer use it?
- Why not just MirrorMaker 2?
- Why not a full stream processor?

### Recommended README structure

1. One-line positioning
2. Primary use case and two secondary use cases
3. Five-minute demo
4. "When to use StreamForge vs MirrorMaker 2 vs Arroyo"
5. Feature table focused on selective replication
6. Operator, Helm, and UI trust signals
7. Links to docs, examples, and compatibility guides

### Recommended docs home structure

- Get Started
- Build Pipelines
- Run in Production
- Compatibility
- Reference

### Documentation cleanup goals

- Reduce duplication across overview and feature files
- Move long benchmark detail out of first-touch pages
- Keep the docs home action-oriented rather than index-heavy
- Improve theme polish and visual hierarchy

## Growth Strategy

### Phase 1: sharpen first impression

- Rewrite the README around the analytics replication story.
- Build a reliable five-minute demo.
- Add a clear decision guide versus MirrorMaker 2 and Arroyo.
- Add explicit Kafka and Redpanda compatibility content.
- Make the docs landing page cleaner and more attractive.

### Phase 2: add proof

- Publish benchmark methodology, not only benchmark claims.
- Add screenshots or short demos of the UI, operator flow, and deployment path.
- Publish three strong case-study-style examples:
  - filtered replication to analytics
  - PII-safe replication
  - topic fan-out and shaping
- Add design partner or early user proof when available.

### Phase 3: expand carefully

- Add schema-aware support where it strengthens the replication story.
- Add lightweight aggregation with strict scope.
- Preserve the boundary against generalized stream processing.

## Success Criteria

### Product clarity

- A new visitor should understand the selective replication value proposition from the README hero section alone.
- The decision guide should make category boundaries obvious.

### Adoption

- Increase stars through stronger first-touch conversion.
- Increase issue and discussion quality because the product story is more focused.
- Increase example-driven adoption in analytics and platform-adjacent workloads.

### Product execution

- Redpanda examples and validation exist.
- Lightweight aggregation arrives without diluting positioning.
- Docs become simpler to navigate and more trustworthy.

## Risks

### Scope expansion risk

If aggregation grows into generalized stateful processing, StreamForge will drift into direct competition with larger systems before it has enough adoption and proof.

### Positioning risk

If the project keeps claiming to outperform or out-feature MirrorMaker 2 in every scenario, technically informed users may stop trusting the broader story.

### Documentation risk

If the repo keeps adding docs without reducing duplication and improving narrative order, project quality will look lower than the actual implementation quality.

## Recommended Next Planning Step

The next plan should turn this design into a concrete execution sequence:

1. README rewrite
2. docs homepage restructuring
3. compatibility documentation for Kafka and Redpanda
4. comparison page versus MirrorMaker 2 and Arroyo
5. five-minute demo flow
6. lightweight aggregation design and implementation plan
