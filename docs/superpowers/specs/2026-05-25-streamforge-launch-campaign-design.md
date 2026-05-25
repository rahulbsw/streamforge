# StreamForge Launch Campaign Design

Date: 2026-05-25

## Goal

Create a repeatable launch campaign that makes StreamForge easy to understand, easy to try, and credible for production-facing teams.

The campaign should promote StreamForge as selective replication for Kafka-compatible brokers, with Redpanda as a compatibility target. The core message is that StreamForge moves only the records and fields downstream systems need, with filtering, transformation, PII-safe routing, Kubernetes deployment, and observability.

## Audience

Primary audiences:

- Platform engineers who operate Kafka, Redpanda, Kubernetes, Helm, and internal data platforms.
- Data engineers who build analytics, lake, CDC, compliance, and downstream contract pipelines.
- Streaming infrastructure teams evaluating lighter alternatives to heavy Kafka Connect-style deployments for selective replication use cases.

Secondary audience:

- AI infrastructure and MLOps teams that need safe real-time event streams for feature pipelines, model monitoring, RAG/event context, experimentation, or analytics. The AI message should stay practical and infrastructure-focused: approved fields in, raw PII out.

## Positioning

Good claim:

- StreamForge is a focused selective replication and data-shaping layer for Kafka-compatible brokers.
- StreamForge is useful when downstream systems need filtered, transformed, redacted, or routed event streams.
- StreamForge can be lighter to deploy than Kafka Connect for targeted replication and shaping workflows.

Avoid:

- Positioning StreamForge as a universal MirrorMaker 2 replacement.
- Claiming it handles active-active replication, offset sync, topic/ACL mirroring, joins, SQL, or broad stateful stream processing.
- Overusing AI language where the demo is really about safe real-time data movement.

## Campaign Shape

Use multiple focused YouTube demos instead of one long overview. Each demo should have a matching X post, optional X thread, LinkedIn post, and short clip.

Recommended order:

1. Local Redpanda quickstart.
2. Kubernetes UI and operator demo.
3. PII-safe data engineering pipeline.
4. CDC to data lake pipeline.
5. AI-ready event stream.
6. AWS production deployment.
7. Observability and scaling.

This order starts with demos that viewers can reproduce quickly, then moves toward production credibility.

## YouTube Demo Series

### Demo 1: 5-Minute Local Redpanda Quickstart

Audience: developers, data engineers, and first-time evaluators.

Story:

- Start a local Redpanda broker.
- Validate `examples/redpanda/selective-replication.yaml`.
- Run StreamForge locally.
- Produce one raw order event.
- Consume shaped output from `analytics-orders`.
- Consume PII-safe output from `pii-safe-orders`.

Main point: StreamForge can selectively replicate one source stream into multiple downstream contracts without a full Kafka Connect deployment.

### Demo 2: Kubernetes UI and Operator Demo

Audience: platform engineers and data platform teams.

Story:

- Install the operator and UI with Helm.
- Sign in to the UI.
- Create a pipeline in form mode.
- Preview generated YAML.
- Deploy the CRD.
- Produce an event and verify transformed output.

Main point: platform teams can manage pipelines through Kubernetes-native resources while still giving users a browser workflow.

### Demo 3: PII-Safe Data Engineering Pipeline

Audience: data engineers, compliance-aware platform teams, and analytics teams.

Story:

- Start from an operational order or user event topic.
- Project only approved fields into an analytics topic.
- Hash or remove raw customer identifiers.
- Route a lower-trust output topic separately from a richer internal topic.
- Validate the output contract by consuming the destination topics.

Main point: StreamForge helps data teams enforce downstream data minimization before events cross trust boundaries.

### Demo 4: CDC to Data Lake Pipeline

Audience: data engineers and lake/warehouse pipeline owners.

Story:

- Use a Debezium-style CDC event.
- Route create/update/delete/schema-change events into focused downstream topics.
- Extract the `after` or `before` payload where appropriate.
- Show how the output can feed S3, Snowflake, Spark, Flink, or a custom lake consumer.

Main point: StreamForge can act as a lightweight shaping step between CDC topics and lake/warehouse consumers.

### Demo 5: AI-Ready Event Stream

Audience: AI infrastructure, MLOps, data engineering, and platform teams.

Story:

- Produce rich raw business events.
- Filter only approved event types and regions.
- Project fields into an `ai-features-orders` or `model-monitoring-events` topic.
- Remove or hash PII before the AI-facing stream.
- Explain where the resulting topic could feed feature pipelines, model monitoring, RAG/event context, or experimentation systems.

Main point: AI teams need safe real-time data contracts, not raw operational topics. StreamForge can create those contracts close to Kafka.

### Demo 6: AWS Production Deployment

Audience: cloud platform teams, production data infrastructure teams, and evaluators who need credibility beyond a laptop demo.

Story:

- Use AWS account infrastructure for a production-style environment.
- Deploy Kafka-compatible infrastructure such as Amazon MSK, or use a clearly named Redpanda/Kafka-compatible setup where appropriate.
- Deploy StreamForge on EKS with Helm/operator.
- Use secure configuration patterns.
- Produce input events and verify transformed outputs.
- Show metrics, lag, and resource sizing.

Main point: StreamForge is not just a local demo. It can be deployed in cloud-native production patterns.

### Demo 7: Observability and Scaling

Audience: SRE, platform engineering, and production owners.

Story:

- Enable Prometheus metrics.
- Show consumed, produced, filtered, error, latency, and lag metrics.
- Generate traffic.
- Demonstrate consumer lag behavior and horizontal scaling.
- Discuss retry and DLQ behavior where the current implementation supports it.

Main point: selective replication needs operational visibility. StreamForge exposes the signals platform teams expect.

## Social Content Model

Each demo should produce:

- One YouTube title and thumbnail concept.
- One short X launch post.
- One X technical thread with three to six posts.
- One LinkedIn post aimed at practitioners.
- One short clip for reuse on X, LinkedIn, and YouTube Shorts.

### X Style

Keep posts technical and concise. Use one strong hook, one practical use case, and one link. Prefer three to five targeted hashtags.

Core hashtags:

- `#Kafka`
- `#Redpanda`
- `#DataEngineering`
- `#Kubernetes`
- `#StreamingData`

Platform hashtags:

- `#PlatformEngineering`
- `#DevOps`
- `#SRE`
- `#CloudNative`
- `#Helm`

AI hashtags:

- `#AIInfrastructure`
- `#MLOps`
- `#RealtimeAI`
- `#FeatureEngineering`

AWS hashtags:

- `#AWS`
- `#MSK`
- `#EKS`
- `#CloudEngineering`

### LinkedIn Style

Use LinkedIn for the deeper practitioner story:

- Start with the production problem.
- Explain why full mirroring is often too broad.
- Show the StreamForge use case.
- Name the demo output clearly.
- Close with a practical invitation to try the demo or review the GitHub repo.

Use three to six hashtags. Favor audience-specific tags over generic reach tags.

## Publishing Sequence

Recommended cadence:

- Week 1: Local Redpanda quickstart and Kubernetes UI demo.
- Week 2: PII-safe pipeline and CDC to data lake.
- Week 3: AI-ready event stream.
- Week 4: AWS production deployment and observability/scaling.

Each YouTube video should be supported by posts on the day of publishing and one follow-up post within 48 hours that highlights a specific command, YAML snippet, metric, or output topic.

## Success Metrics

Track:

- GitHub stars and forks.
- README demo clicks.
- YouTube views, average view duration, and click-through rate.
- X impressions, reposts, profile clicks, and link clicks.
- LinkedIn impressions, saves, comments from practitioners, and profile/repo clicks.
- Issues or discussions opened by users trying the demos.

The strongest signal is not raw impressions. It is a practitioner reproducing a demo, opening an issue, asking for a feature, or starring the repo after watching.

## Next Deliverables

After this design is approved, create:

1. Detailed scripts for each YouTube demo.
2. Exact X launch posts and X thread drafts for each demo.
3. LinkedIn post drafts for each demo.
4. Thumbnail/title concepts for each video.
5. AWS demo runbook with setup, commands, cleanup, and cost controls.
