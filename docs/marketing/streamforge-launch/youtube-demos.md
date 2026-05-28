# StreamForge YouTube Demo Series

Use this file as the working script source for the campaign. Keep the demos practical, terminal-visible, and focused on the problem StreamForge solves: selective replication and data shaping for Kafka-compatible brokers.

Repo link to use in descriptions: `https://github.com/rahulbsw/streamforge`

## Demo 1: 5-Minute Local Redpanda Quickstart

**Primary Title:** StreamForge in 5 Minutes: Selective Kafka Replication with Redpanda

**Alternate Titles:**

- Filter and Replicate Kafka Events Locally with StreamForge and Redpanda
- Kafka Selective Replication Without Kafka Connect: StreamForge Quickstart

**Target Audience:** developers, data engineers, and first-time evaluators.

**Hook:** "Full topic mirroring is often too much. In this demo, one raw orders topic becomes two focused downstream topics: analytics-safe output and PII-safe output."

**Recording Setup:**

- Terminal at repo root.
- Docker running.
- Use `examples/redpanda/docker-compose.yml`.
- Keep a second terminal ready for topic creation, produce, and consume commands.

**Demo Flow:**

1. Show the README one-line positioning.
2. Start Redpanda.
3. Validate `examples/redpanda/selective-replication.yaml`.
4. Run StreamForge.
5. Create `raw-orders`, `analytics-orders`, and `pii-safe-orders`.
6. Produce one raw order with email and customer ID.
7. Consume `analytics-orders`.
8. Consume `pii-safe-orders` and point out that raw email is not in the value.

**Script Beats:**

- "This is StreamForge: selective replication for Kafka-compatible brokers."
- "The source event has more data than every downstream consumer should receive."
- "I am using Redpanda locally because it gives us a Kafka-compatible broker without a large local setup."
- "Before running anything, I validate the config. That is important because the config is the contract."
- "Now StreamForge reads one source topic and writes multiple downstream shapes."
- "The analytics topic keeps fields analytics needs."
- "The PII-safe topic removes raw email from the payload and uses a hashed identifier as the key."
- "That is the core use case: reduce blast radius before data crosses a trust boundary."

**Core Commands:**

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml cargo run --release --bin streamforge
```

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic create raw-orders analytics-orders pii-safe-orders

printf '%s\n' \
  '{"order_id":"ord-1001","customer":{"id":"cust-42","email":"alice@example.com"},"amount":125,"region":"us","created_at":"2026-05-12T15:04:05Z"}' \
  | docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
      rpk topic produce raw-orders
```

**Thumbnail Concept:** Terminal on the left with `raw-orders`, right side split into `analytics-orders` and `pii-safe-orders`. Large text: "One Kafka Topic -> Safe Outputs".

**Chapters:**

- 00:00 Problem: full mirroring sends too much data
- 00:35 Start local Redpanda
- 01:10 Validate the StreamForge config
- 01:45 Run StreamForge
- 02:20 Produce one raw event
- 03:10 Verify analytics output
- 03:55 Verify PII-safe output
- 04:35 When to use this pattern

**Success Criteria:**

- Viewer sees one input event become two downstream outputs.
- Viewer can reproduce the demo from the README and `docs/QUICKSTART.md`.
- The positioning stays focused on selective replication, not universal mirroring.

## Demo 2: Kubernetes UI and Operator Demo

**Primary Title:** StreamForge Kubernetes Demo: Helm, Operator, UI, and Kafka Pipeline

**Alternate Titles:**

- Build a Kafka Pipeline in Kubernetes with StreamForge UI
- StreamForge Operator Demo: UI to YAML to Kubernetes CRD

**Target Audience:** platform engineers, data platform teams, and Kubernetes operators.

**Hook:** "A pipeline should be easy to create, but still visible as Kubernetes YAML. This demo goes from UI form to generated YAML to CRD deployment."

**Recording Setup:**

- Minikube or a small Kubernetes cluster.
- Helm available.
- Browser pointed at StreamForge UI.
- Terminal ready for `kubectl`, `helm`, produce, and consume commands.
- Use `docs/UI_MINIKUBE_DEMO.md` as the reference path.

**Demo Flow:**

1. Show cluster and namespace.
2. Install the StreamForge operator and UI with Helm.
3. Open the UI and sign in with demo credentials.
4. Create a pipeline in form mode.
5. Preview generated YAML.
6. Deploy the CRD.
7. Produce an input event.
8. Consume pipeline output.
9. Show the pipeline exists as Kubernetes state.

**Script Beats:**

- "Platform teams usually want both: a usable interface and declarative state."
- "The UI helps create the pipeline, but the pipeline still lands as Kubernetes configuration."
- "Before deploying, I can review generated YAML instead of trusting a black box."
- "Now I deploy the CRD and verify behavior from Kafka, not just from the UI."
- "This is the platform story: users get a guided workflow, operators get Kubernetes-native control."

**Core Commands:**

```bash
kubectl get nodes
helm install streamforge ./helm/streamforge-operator --namespace streamforge --create-namespace
kubectl get pods -n streamforge
```

Use the exact UI flow from `docs/UI_MINIKUBE_DEMO.md` for the browser recording.

**Thumbnail Concept:** Browser UI screenshot with a YAML preview overlay and Kubernetes icon-style labels: "UI -> YAML -> CRD".

**Chapters:**

- 00:00 Platform problem: pipelines need UX and control
- 00:45 Helm install
- 01:40 Open the StreamForge UI
- 02:20 Create a pipeline
- 03:20 Review generated YAML
- 04:05 Deploy the CRD
- 05:00 Produce and consume test events
- 06:10 Kubernetes-native state

**Success Criteria:**

- Viewer understands the UI is not replacing Kubernetes control.
- Viewer sees a pipeline move from form input to YAML to deployed CRD.
- Viewer sees Kafka output from the deployed pipeline, not only a successful UI action.
- With the current chart default pipeline image, do not claim transform verification unless the output has been re-tested with an updated image.

## Demo 3: PII-Safe Data Engineering Pipeline

**Primary Title:** PII-Safe Kafka Pipelines: Filter, Transform, and Redact with StreamForge

**Alternate Titles:**

- Stop Sending Raw PII Downstream: StreamForge Kafka Demo
- Build Analytics-Safe Kafka Topics with StreamForge

**Target Audience:** data engineers, analytics engineers, privacy-aware platform teams, and compliance-adjacent teams.

**Hook:** "The source topic has customer data. The downstream analytics topic should not. This demo shows how to create a safer event contract before data leaves the operational boundary."

**Recording Setup:**

- Local Redpanda or Kubernetes environment.
- Use `examples/production/pii-redaction.yaml` as the reference config.
- Prepare an input event with email, customer ID, region, amount, and consent fields.

**Demo Flow:**

1. Show the raw event and identify sensitive fields.
2. Open the PII redaction config.
3. Show filters and field projection.
4. Run config validation.
5. Start StreamForge.
6. Produce raw event.
7. Consume analytics-safe output.
8. Point out removed or hashed sensitive fields.

**Script Beats:**

- "The problem is not Kafka. The problem is that raw operational topics often have a broader contract than downstream systems need."
- "This config is explicit about what leaves the source boundary."
- "Projection is safer than relying on every consumer to ignore fields."
- "Hashing gives downstream systems stable joins or grouping when raw identifiers are not appropriate."
- "StreamForge fits before analytics, partner, staging, and lower-trust environments."

**Core Commands:**

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/pii-redaction.yaml
CONFIG_FILE=examples/production/pii-redaction.yaml cargo run --release --bin streamforge
```

**Thumbnail Concept:** Raw event card with `email` and `customer_id` marked, arrow into StreamForge, output card labeled "analytics-safe". Large text: "PII Out".

**Chapters:**

- 00:00 Why raw topic mirroring is risky
- 00:50 Inspect the raw event
- 01:35 Review the redaction config
- 02:35 Validate and run StreamForge
- 03:25 Produce sensitive input
- 04:05 Verify safe output
- 05:00 Where this fits in data platforms

**Success Criteria:**

- Viewer sees a concrete before/after payload.
- Viewer understands StreamForge as a contract enforcement point before downstream analytics.
- The demo avoids claiming complete compliance by itself.

## Demo 4: CDC to Data Lake Pipeline

**Primary Title:** Kafka CDC to Data Lake: Shape Debezium Events with StreamForge

**Alternate Titles:**

- Lightweight CDC Event Shaping Before S3, Snowflake, Spark, or Flink
- StreamForge Demo: Route Database Change Events from Kafka

**Target Audience:** data engineers, lakehouse pipeline owners, analytics platform teams.

**Hook:** "CDC topics are rich, but lake and warehouse consumers usually need a cleaner shape. StreamForge can sit between Debezium-style topics and downstream lake consumers."

**Recording Setup:**

- Local broker environment.
- Use `examples/production/cdc-to-datalake.yaml` as the reference config.
- Prepare sample Debezium-style create, update, delete, and schema-change events.

**Demo Flow:**

1. Show a Debezium-style CDC envelope.
2. Open the CDC-to-lake config.
3. Explain create/update/delete routing.
4. Validate the config.
5. Run StreamForge.
6. Produce sample CDC events.
7. Consume shaped destination topics.
8. Explain how an S3 sink, Snowflake loader, Spark job, or Flink job would consume the shaped topics.

**Script Beats:**

- "CDC is useful, but raw envelopes are not always the contract you want downstream."
- "This config extracts the part of the event each consumer cares about."
- "Create and update events use the `after` state; delete events use the `before` state."
- "StreamForge is not replacing your lake sink. It is making the Kafka side cleaner before the sink reads it."

**Core Commands:**

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/cdc-to-datalake.yaml
CONFIG_FILE=examples/production/cdc-to-datalake.yaml cargo run --release --bin streamforge
```

**Thumbnail Concept:** Debezium envelope on the left, StreamForge in the middle, clean lake topics on the right. Large text: "CDC -> Clean Topics".

**Chapters:**

- 00:00 CDC envelope problem
- 00:45 Inspect sample event
- 01:30 Review routing config
- 02:25 Validate and run
- 03:10 Produce CDC events
- 04:00 Verify shaped outputs
- 05:10 Where the data lake sink fits

**Success Criteria:**

- Viewer understands StreamForge as a shaping layer, not a data lake sink.
- Viewer sees different CDC operations routed or extracted differently.
- Viewer can connect the pattern to existing warehouse and lake consumers.

## Demo 5: AI-Ready Event Stream

**Primary Title:** Build PII-Safe Real-Time Streams for AI Systems with Kafka and StreamForge

**Alternate Titles:**

- AI Infrastructure Needs Safer Kafka Topics: StreamForge Demo
- From Raw Events to AI-Ready Kafka Streams with StreamForge

**Target Audience:** AI infrastructure teams, MLOps teams, platform engineers, and data engineers.

**Hook:** "AI systems do not need raw operational topics. They need approved, stable, real-time data contracts."

**Recording Setup:**

- Local Redpanda or Kubernetes environment.
- Start from the selective replication or PII-safe config and adapt output topic naming for the recording.
- Use `ai-features-orders` or `model-monitoring-events` as the destination topic.

**Demo Flow:**

1. Show a raw order or user activity event.
2. Explain why raw PII should not feed every AI workflow.
3. Filter for approved event types and regions.
4. Project stable business fields.
5. Hash or remove customer identifiers.
6. Publish to `ai-features-orders` or `model-monitoring-events`.
7. Consume output and explain where a feature pipeline, model monitor, RAG/event-context service, or experimentation system would subscribe.

**Script Beats:**

- "The AI angle here is simple: make the data contract safe before the AI system sees it."
- "This is not an LLM demo. It is infrastructure for real-time AI and ML systems."
- "The downstream topic has the business facts we want: order ID, amount, region, timestamp, maybe a stable hashed identifier."
- "It does not carry raw customer email."
- "That makes the Kafka topic easier to govern and easier for AI teams to consume."

**Core Commands:**

```bash
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml cargo run --release --bin streamforge
```

During recording, describe the destination as an AI-facing contract. If using a dedicated config, name the output `ai-features-orders`.

**Thumbnail Concept:** Raw Kafka topic feeding StreamForge, output labeled `ai-features-orders`, red PII lock icon on removed fields. Large text: "AI-Ready Kafka".

**Chapters:**

- 00:00 AI systems need safe real-time contracts
- 00:45 Inspect raw business event
- 01:30 Filter approved events
- 02:15 Project AI-facing fields
- 03:00 Remove or hash PII
- 03:45 Consume the AI-ready topic
- 04:40 How this fits MLOps and feature pipelines

**Success Criteria:**

- Viewer sees AI positioning without exaggerated claims.
- Viewer understands this as safe event preparation for downstream AI systems.
- Viewer sees a clear topic contract that excludes raw PII.

## Demo 6: AWS Production Deployment

**Primary Title:** Deploy StreamForge on AWS: EKS, MSK, Helm, and Kafka Pipeline Verification

**Alternate Titles:**

- Production-Style Kafka Selective Replication on AWS with StreamForge
- StreamForge AWS Demo: EKS + MSK + Observability

**Target Audience:** cloud platform teams, production data infrastructure teams, and evaluators who need cloud credibility.

**Hook:** "Local demos are useful, but production teams need to see the cloud pattern: EKS, MSK, secure configuration, Helm deployment, verification, and cleanup."

**Recording Setup:**

- AWS account with budget alarm enabled.
- AWS CLI, `kubectl`, `eksctl`, and Helm installed.
- Prefer a temporary EKS cluster and MSK Serverless cluster for a bounded demo.
- Use `docs/marketing/streamforge-launch/aws-demo-runbook.md` as the recording source.

**Demo Flow:**

1. Show architecture: EKS runs StreamForge, MSK provides Kafka-compatible topics.
2. Show cost-control and cleanup plan before creating resources.
3. Create or show EKS cluster.
4. Create or show MSK cluster and bootstrap brokers.
5. Build or reference StreamForge image.
6. Install operator/UI with Helm.
7. Configure StreamForge with MSK bootstrap and auth settings.
8. Produce input event.
9. Verify transformed output.
10. Show metrics and cleanup commands.

**Script Beats:**

- "This demo is intentionally production-style, but temporary."
- "The key production question is not whether a pod starts. It is whether a Kafka input becomes the expected downstream contract."
- "I am showing cleanup up front because cloud demos should be reproducible without leaving expensive resources behind."
- "MSK is the Kafka-compatible service. EKS is where StreamForge runs."
- "The verification step is still Kafka-level: produce input, consume shaped output."

**Core Commands:**

```bash
aws sts get-caller-identity
eksctl create cluster --name streamforge-demo --region us-west-2 --nodes 2 --node-type t3.large
kubectl get nodes
helm install streamforge ./helm/streamforge-operator --namespace streamforge --create-namespace
```

Use the AWS runbook for exact environment variables, topic setup, verification, and cleanup.

**Thumbnail Concept:** AWS/EKS/MSK architecture line with StreamForge in the middle. Large text: "Kafka Pipelines on AWS".

**Chapters:**

- 00:00 Why production-style demo matters
- 00:50 Architecture and cost controls
- 01:40 EKS cluster check
- 02:35 MSK bootstrap and topics
- 03:40 Helm deployment
- 04:50 Pipeline config
- 06:10 Produce and consume verification
- 07:20 Metrics and cleanup

**Success Criteria:**

- Viewer sees a complete cloud path without pretending it is a full production hardening guide.
- Viewer understands how EKS, MSK, Helm, and StreamForge fit together.
- Cleanup is visible and repeatable.

## Demo 7: Observability and Scaling

**Primary Title:** Operating StreamForge: Kafka Lag, Prometheus Metrics, Scaling, Retry, and DLQ

**Alternate Titles:**

- StreamForge Observability Demo: Metrics, Lag, and Scaling
- Production Signals for Selective Kafka Replication

**Target Audience:** SREs, platform engineers, Kafka operators, and production owners.

**Hook:** "Selective replication still needs production signals. This demo shows the metrics and lag views you need before trusting a pipeline."

**Recording Setup:**

- Local or Kubernetes environment with metrics enabled.
- Use `docs/OBSERVABILITY_QUICKSTART.md`.
- Prometheus or direct `/metrics` curl ready.
- Traffic generator ready with repeated order events.

**Demo Flow:**

1. Enable metrics in config.
2. Start StreamForge.
3. Show `/health` and `/metrics`.
4. Generate traffic.
5. Show consumed, produced, filtered, latency, and lag metrics.
6. Discuss horizontal scaling and partition count.
7. Show retry and DLQ behavior if using a config that exercises failure paths.

**Script Beats:**

- "A Kafka pipeline is not done when it processes one message."
- "You need to know whether it is falling behind, filtering correctly, and producing to the expected destinations."
- "The first check is health. The second is metrics."
- "Consumer lag tells you whether your pipeline is keeping up."
- "Throughput and latency tell you how the pipeline behaves under load."
- "Retry and DLQ behavior are the safety rails for bad records or downstream failures."

**Core Commands:**

```bash
curl http://localhost:9090/health
curl http://localhost:9090/metrics
```

Prometheus queries to show:

```promql
rate(streamforge_messages_consumed_total[5m])
sum(rate(streamforge_messages_produced_total[5m])) by (destination)
sum(streamforge_consumer_lag)
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

**Thumbnail Concept:** Metrics dashboard with labels for lag, throughput, errors, and p99 latency. Large text: "Operate It".

**Chapters:**

- 00:00 Why observability matters
- 00:40 Enable metrics
- 01:25 Health and metrics endpoints
- 02:10 Generate traffic
- 03:00 Throughput and filter metrics
- 04:00 Consumer lag
- 05:00 Scaling considerations
- 06:00 Retry and DLQ notes

**Success Criteria:**

- Viewer sees operational signals, not only feature behavior.
- Viewer understands which metrics answer which production questions.
- Viewer sees scaling framed around Kafka partitions and consumer groups.
