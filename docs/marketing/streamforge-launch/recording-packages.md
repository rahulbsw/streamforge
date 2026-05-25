# StreamForge Recording Packages

Use these packages to record the seven campaign videos with human narration. Each package includes a recording objective, screen plan, terminal commands, narration script, expected proof points, YouTube metadata, and publishing notes.

Human audio is recommended for every video. Read the narration naturally instead of word-for-word if the terminal takes longer than expected. Keep the recording practical: show commands, configs, output topics, and verification.

Repo link to use before a YouTube URL exists: `https://github.com/rahulbsw/streamforge`

## Shared Recording Setup

Prepare once before recording:

```bash
cd /Users/rajain5/dev/tools/cisco-git/wap-mirrormaker-rust
git status --short
docker version
cargo --version
```

Terminal setup:

- Use a large readable font.
- Use two terminal panes for local demos: left for StreamForge, right for Kafka/Redpanda commands.
- Clear terminal scrollback before each take.
- Keep secrets and AWS account identifiers out of frame.
- Start every demo with the repo root visible so viewers trust the commands are reproducible.
- Record full-screen app windows only.
- Do not record the desktop, laptop wallpaper, personal dock, notifications, or unrelated windows.
- Use one full-screen terminal scene and one full-screen browser scene instead of dragging windows around on the desktop.
- Disable notifications and hide the menu bar/dock if your recorder does not crop them.
- When switching apps, pause recording or use a clean scene transition so personal windows are never visible.

Recommended capture setup:

- Use OBS, Screen Studio, or another recorder that supports window capture.
- Capture the terminal window directly for command-heavy sections.
- Capture the browser window directly for UI sections.
- Use full-screen terminal tabs instead of desktop-level screen capture.
- Keep a separate private desktop space for notes, AWS console, chat, and credentials.

Human narration tone:

- Plain technical explanation.
- Avoid hype-only language.
- Say "selective replication" and "data shaping" consistently.
- Say "Kafka-compatible broker" when using Redpanda.
- Say "production-style" for AWS unless every production hardening step is actually included.

## Package 1: Local Redpanda Quickstart

**Objective:** Show the shortest reproducible StreamForge path: one source topic replicated into analytics-safe and PII-safe destination topics.

**Target Viewer:** Data engineers, platform engineers, and developers evaluating the project for the first time.

**Screen Plan:**

1. Show `README.md` tagline.
2. Show `examples/redpanda/selective-replication.yaml`.
3. Terminal pane 1: start Redpanda and run StreamForge.
4. Terminal pane 2: create topics, produce one event, consume both outputs.
5. Optional browser shot: GitHub README demo section after recording.

**Terminal Commands:**

Pre-build before recording so compile output does not dominate the video:

```bash
cargo build --release --bin streamforge
```

Main recording commands:

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
docker compose -f examples/redpanda/docker-compose.yml ps
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml ./target/release/streamforge
```

In a second terminal:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic delete raw-orders analytics-orders pii-safe-orders || true

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic create raw-orders analytics-orders pii-safe-orders

printf '%s\n' \
  '{"order_id":"ord-1001","customer":{"id":"cust-42","email":"alice@example.com"},"amount":125,"region":"us","created_at":"2026-05-12T15:04:05Z"}' \
  | docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
      rpk topic produce raw-orders

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume analytics-orders -n 1 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume pii-safe-orders -n 1 --offset start

curl http://localhost:8080/health
curl http://localhost:8080/metrics | rg "streamforge_messages_(consumed|produced)|streamforge_consumer_lag"
```

Cleanup:

```bash
docker compose -f examples/redpanda/docker-compose.yml down
```

**Expected Proof Points:**

- Config validation succeeds.
- StreamForge starts with `examples/redpanda/selective-replication.yaml`.
- `analytics-orders` contains `order_id`, `customer_id`, `amount`, `region`, and `created_at`.
- `pii-safe-orders` contains `order_id`, `amount`, `region`, and `created_at`.
- Raw email is absent from the `pii-safe-orders` value.
- Metrics show one consumed message, one produced message for `analytics-orders`, one produced message for `pii-safe-orders`, and zero lag.

**Human Audio Script:**

"This is StreamForge, a selective replication and data-shaping layer for Kafka-compatible brokers. I am using Redpanda locally so the demo is easy to reproduce."

"The source topic is `raw-orders`. Instead of mirroring the whole topic unchanged, this config writes two downstream contracts: one for analytics and one that keeps raw email out of the payload."

"I validate the config first because the YAML is the contract. If the config is wrong, I want to know before the pipeline starts."

"Now StreamForge is running. I will produce one raw order event with a customer ID and email address."

"The analytics topic keeps the fields analytics needs. The PII-safe topic keeps the business facts but does not expose the raw email in the value."

"That is the core use case: move only the records and fields downstream systems actually need."

**YouTube Metadata:**

- Title: `StreamForge in 5 Minutes: Selective Kafka Replication with Redpanda`
- Description: `Run StreamForge locally with Redpanda and replicate one raw orders topic into analytics-safe and PII-safe downstream topics.`
- Thumbnail text: `One Kafka Topic -> Safe Outputs`
- Pinned comment: `Commands are in docs/QUICKSTART.md and examples/redpanda/selective-replication.yaml. Try the demo and open an issue if any command does not work in your environment.`

**Publish Copy:** Use Demo 1 from `social-posts.md`.

**Dry-Run Result: 2026-05-25**

- Config validation passed.
- Redpanda started from `examples/redpanda/docker-compose.yml`.
- The three demo topics were reset and recreated before producing the sample event.
- StreamForge started cleanly from `./target/release/streamforge`.
- Produced sample event at `raw-orders` offset `0`.
- `analytics-orders` output value:

```json
{"amount":125,"created_at":"2026-05-12T15:04:05Z","customer_id":"cust-42","order_id":"ord-1001","region":"us"}
```

- `pii-safe-orders` output value:

```json
{"amount":125,"created_at":"2026-05-12T15:04:05Z","order_id":"ord-1001","region":"us"}
```

- `pii-safe-orders` output key was SHA-256 hash `ff8d9819fc0e12bf0d24892e45987e249a28dce836a85cad60e28eaaa8c6d976`.
- Health endpoint returned `OK`.
- Metrics showed `streamforge_messages_consumed_total 1`, one produced message for each destination, and `streamforge_consumer_lag` at `0`.
- Cleaned up with `docker compose -f examples/redpanda/docker-compose.yml down`.

## Package 2: Kubernetes UI and Operator Demo

**Objective:** Show StreamForge as a Kubernetes-native pipeline workflow with a UI front door and YAML/CRD control plane.

**Target Viewer:** Platform engineers, Kubernetes operators, and data platform teams.

**Screen Plan:**

1. Terminal: show cluster health.
2. Terminal: install Helm chart.
3. Browser: open UI and create a pipeline.
4. Browser: show generated YAML preview.
5. Terminal: show CRD and pods.
6. Terminal: produce and consume verification event.

**Terminal Commands:**

```bash
kubectl get nodes
helm install streamforge ./helm/streamforge-operator --namespace streamforge --create-namespace
kubectl get pods -n streamforge
kubectl get svc -n streamforge
```

If using local port-forward:

```bash
kubectl port-forward svc/streamforge-ui 3000:3000 -n streamforge
```

Verification commands depend on the demo cluster broker. For Minikube, follow `docs/UI_MINIKUBE_DEMO.md` and `docs/KUBERNETES.md`.

Cleanup:

```bash
helm uninstall streamforge -n streamforge
kubectl delete namespace streamforge
```

**Expected Proof Points:**

- Helm install succeeds.
- UI is reachable.
- Pipeline can be created in form mode.
- Generated YAML is visible before deploy.
- Pipeline becomes Kubernetes state.
- Kafka output verifies the transform.

**Human Audio Script:**

"This demo is for platform teams. A pipeline workflow should be usable, but it should also be reviewable and operable."

"I am installing StreamForge with Helm. The operator gives us Kubernetes-native pipeline management, and the UI gives users a guided way to create pipelines."

"The important part is the YAML preview. The UI is not a black box; the pipeline becomes declarative state."

"After deploying the CRD, I verify behavior from Kafka. A successful UI save is not enough. The proof is input event in, transformed output out."

"This is the platform story: guided creation for users, Kubernetes control for operators."

**YouTube Metadata:**

- Title: `StreamForge Kubernetes Demo: Helm, Operator, UI, and Kafka Pipeline`
- Description: `Install StreamForge with Helm, create a pipeline in the UI, preview generated YAML, deploy the CRD, and verify Kafka output.`
- Thumbnail text: `UI -> YAML -> CRD`
- Pinned comment: `The UI demo path is documented in docs/UI_MINIKUBE_DEMO.md. The Helm chart lives under helm/streamforge-operator.`

**Publish Copy:** Use Demo 2 from `social-posts.md`.

## Package 3: PII-Safe Data Engineering Pipeline

**Objective:** Show how StreamForge creates a safer downstream analytics contract by filtering, projecting, and hashing or removing sensitive fields.

**Target Viewer:** Data engineers, analytics engineers, privacy-aware platform teams, and compliance-adjacent teams.

**Screen Plan:**

1. Show a raw event with customer identifiers.
2. Show `examples/production/pii-redaction.yaml`.
3. Validate the config.
4. Run StreamForge.
5. Produce the raw event.
6. Consume the safer downstream topic.
7. Point out absent raw PII.

**Terminal Commands:**

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/pii-redaction.yaml
CONFIG_FILE=examples/production/pii-redaction.yaml cargo run --release --bin streamforge
```

Use broker-specific topic creation and produce/consume commands for the selected environment. For local recording, adapt the Redpanda commands from Package 1.

**Expected Proof Points:**

- PII redaction config validates.
- Raw event contains sensitive fields.
- Destination output contains only approved fields.
- Raw email or direct customer identifier is removed or hashed according to the config.

**Human Audio Script:**

"Operational topics often contain more than analytics systems should receive."

"This config is explicit about what leaves the source boundary. It filters the event, projects approved fields, and avoids sending raw identifiers where they are not needed."

"Projection is safer than relying on every downstream consumer to ignore fields."

"StreamForge does not replace governance, access control, or audit logs. It gives data teams a concrete enforcement point in the Kafka path."

**YouTube Metadata:**

- Title: `PII-Safe Kafka Pipelines: Filter, Transform, and Redact with StreamForge`
- Description: `Use StreamForge to create analytics-safe Kafka topics by filtering events, projecting approved fields, and keeping raw PII out of lower-trust outputs.`
- Thumbnail text: `PII Out`
- Pinned comment: `The production example is examples/production/pii-redaction.yaml. Review your own governance requirements before using any data minimization pattern in production.`

**Publish Copy:** Use Demo 3 from `social-posts.md`.

## Package 4: CDC to Data Lake Pipeline

**Objective:** Show StreamForge as a lightweight shaping layer between CDC topics and lake or warehouse consumers.

**Target Viewer:** Data engineers, lakehouse pipeline owners, analytics platform teams.

**Screen Plan:**

1. Show a Debezium-style CDC envelope.
2. Show `examples/production/cdc-to-datalake.yaml`.
3. Explain create/update/delete extraction.
4. Validate config.
5. Run StreamForge.
6. Produce sample CDC records.
7. Consume shaped outputs.

**Terminal Commands:**

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/cdc-to-datalake.yaml
CONFIG_FILE=examples/production/cdc-to-datalake.yaml cargo run --release --bin streamforge
```

Use local Redpanda commands for topic creation and event production if recording locally.

**Expected Proof Points:**

- Config validates.
- CDC envelope is transformed into a cleaner downstream shape.
- Create/update records use the after-state payload.
- Delete records use the before-state payload when demonstrated.
- Viewer understands the lake sink remains separate from StreamForge.

**Human Audio Script:**

"CDC topics are powerful, but raw envelopes are not always the right downstream contract."

"This demo treats StreamForge as the Kafka-side shaping layer. It extracts the payload shape that lake, warehouse, Spark, Flink, or custom consumers can read more easily."

"Create and update records usually care about the `after` state. Delete records often need the `before` state. Schema changes can be routed separately."

"StreamForge is not the data lake sink. It makes the Kafka contract cleaner before the sink reads it."

**YouTube Metadata:**

- Title: `Kafka CDC to Data Lake: Shape Debezium Events with StreamForge`
- Description: `Shape Debezium-style Kafka CDC events with StreamForge before downstream lake, warehouse, Spark, Flink, or custom consumers read them.`
- Thumbnail text: `CDC -> Clean Topics`
- Pinned comment: `The reference config is examples/production/cdc-to-datalake.yaml. This demo focuses on Kafka-side shaping, not lake sink configuration.`

**Publish Copy:** Use Demo 4 from `social-posts.md`.

## Package 5: AI-Ready Event Stream

**Objective:** Show a practical AI infrastructure use case: create a safe real-time topic for AI or ML systems without exposing raw operational payloads.

**Target Viewer:** AI infrastructure teams, MLOps teams, platform engineers, and data engineers.

**Screen Plan:**

1. Show raw business event.
2. Explain why raw operational topics are too broad for AI systems.
3. Show config that filters and projects the AI-facing fields.
4. Produce raw event.
5. Consume `ai-features-orders` or equivalent destination topic.
6. Explain downstream consumers: features, model monitoring, RAG/event context, experimentation.

**Terminal Commands:**

Use the selective replication config as the base:

```bash
cargo run --quiet --bin streamforge-validate -- examples/redpanda/selective-replication.yaml
CONFIG_FILE=examples/redpanda/selective-replication.yaml cargo run --release --bin streamforge
```

For a dedicated recording, copy the config and name the destination topic `ai-features-orders` before recording.

**Expected Proof Points:**

- Raw event contains more fields than the AI-facing contract.
- AI-facing topic contains approved business fields.
- Raw email is absent or hashed.
- Narration avoids implying that StreamForge runs model inference.

**Human Audio Script:**

"The AI use case here is not an LLM wrapper. It is safer real-time data infrastructure."

"AI and ML systems need fresh business events, but they should not automatically consume raw operational Kafka topics."

"StreamForge creates an AI-facing contract close to Kafka: approved event types, stable fields, and raw PII removed or hashed."

"That topic can feed feature pipelines, model monitoring, event context, experimentation, or analytics."

"The key idea is simple: AI infrastructure still needs good data engineering."

**YouTube Metadata:**

- Title: `Build PII-Safe Real-Time Streams for AI Systems with Kafka and StreamForge`
- Description: `Create an AI-facing Kafka topic by filtering raw events, projecting stable business fields, and keeping raw PII out of downstream AI and ML systems.`
- Thumbnail text: `AI-Ready Kafka`
- Pinned comment: `This demo is about data contracts for AI infrastructure. It does not run model inference; it prepares safer real-time streams for downstream AI systems.`

**Publish Copy:** Use Demo 5 from `social-posts.md`.

## Package 6: AWS Production Deployment

**Objective:** Show a production-style AWS path for StreamForge using EKS, MSK or a clearly labeled Kafka-compatible fallback, Helm/operator deployment, verification, and cleanup.

**Target Viewer:** Cloud platform teams, production data infrastructure teams, and evaluators who need cloud credibility.

**Screen Plan:**

1. Show architecture and cost controls.
2. Show AWS identity and region without exposing sensitive details.
3. Show EKS cluster.
4. Show MSK bootstrap or fallback broker.
5. Install StreamForge with Helm.
6. Apply pipeline config.
7. Produce and consume verification events.
8. Show metrics/logs.
9. Run cleanup commands.

**Terminal Commands:**

Use `docs/marketing/streamforge-launch/aws-demo-runbook.md` as the source of truth. Minimum visible checks:

```bash
aws sts get-caller-identity
kubectl get nodes
helm install streamforge ./helm/streamforge-operator --namespace streamforge --create-namespace
kubectl get pods -n streamforge
kubectl get streamforgepipeline -n streamforge
```

Cleanup:

```bash
helm uninstall streamforge -n streamforge
kubectl delete namespace streamforge
eksctl delete cluster --name streamforge-demo --region us-west-2
```

**Expected Proof Points:**

- AWS region and account context are controlled.
- EKS is running StreamForge.
- Kafka broker is reachable from the cluster.
- Produce/consume verification succeeds.
- Cleanup plan is shown before resources are created.

**Human Audio Script:**

"Local demos are useful, but production teams also need to see the cloud pattern."

"This is a production-style temporary setup: EKS runs StreamForge, MSK or a clearly labeled Kafka-compatible broker provides topics, and Helm installs the operator and UI."

"I am showing the cleanup path up front because cloud demos should not leave expensive resources behind."

"The important verification is Kafka-level. Pods running is not enough. I want to see a raw input event become the expected downstream output."

"This is not a complete production hardening guide. It is the deploy-and-verify path."

**YouTube Metadata:**

- Title: `Deploy StreamForge on AWS: EKS, MSK, Helm, and Kafka Pipeline Verification`
- Description: `Run a production-style StreamForge demo on AWS with EKS, MSK or a Kafka-compatible fallback, Helm/operator deployment, Kafka verification, metrics, and cleanup.`
- Thumbnail text: `Kafka Pipelines on AWS`
- Pinned comment: `AWS resources can incur cost. Follow docs/marketing/streamforge-launch/aws-demo-runbook.md and clean up resources after recording.`

**Publish Copy:** Use Demo 6 from `social-posts.md`.

## Package 7: Observability and Scaling

**Objective:** Show the operational signals for StreamForge: health, metrics, lag, throughput, latency, scaling considerations, retry, and DLQ behavior.

**Target Viewer:** SREs, platform engineers, Kafka operators, and production owners.

**Screen Plan:**

1. Show config with observability enabled.
2. Start StreamForge.
3. Curl health and metrics endpoints.
4. Generate traffic.
5. Show consumed, produced, filtered, error, latency, and lag metrics.
6. Discuss partition-aware scaling.
7. Show retry and DLQ notes or a controlled failure if the environment is prepared.

**Terminal Commands:**

```bash
CONFIG_FILE=examples/config.with-observability.yaml cargo run --release --bin streamforge
curl http://localhost:9090/health
curl http://localhost:9090/metrics
```

Prometheus queries to prepare:

```promql
rate(streamforge_messages_consumed_total[5m])
sum(rate(streamforge_messages_produced_total[5m])) by (destination)
sum(streamforge_consumer_lag)
histogram_quantile(0.99, rate(streamforge_processing_duration_seconds_bucket[5m]))
```

**Expected Proof Points:**

- Health endpoint responds.
- Metrics endpoint exposes StreamForge counters/gauges/histograms.
- Traffic changes consumed and produced metrics.
- Consumer lag is visible.
- Scaling discussion is tied to Kafka partitions and consumer groups.

**Human Audio Script:**

"A Kafka pipeline is not production-ready just because it processed one message."

"For selective replication, I need to know whether the service is healthy, whether it is keeping up, how many messages are filtered, how many are produced, and whether errors are increasing."

"Consumer lag is the basic production question: is the pipeline falling behind the source topic?"

"Scaling is tied to Kafka partitions. More replicas help only when there are enough partitions to assign."

"Retry and DLQ behavior are the safety rails for bad records or downstream failures."

**YouTube Metadata:**

- Title: `Operating StreamForge: Kafka Lag, Prometheus Metrics, Scaling, Retry, and DLQ`
- Description: `Inspect StreamForge health, Prometheus metrics, consumer lag, throughput, latency, scaling behavior, retry, and DLQ signals for selective Kafka replication.`
- Thumbnail text: `Operate It`
- Pinned comment: `Start with docs/OBSERVABILITY_QUICKSTART.md for metrics and lag monitoring. Scale replicas with Kafka partition count in mind.`

**Publish Copy:** Use Demo 7 from `social-posts.md`.
