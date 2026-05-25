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
helm lint ./helm/streamforge-operator
helm template streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --set ui.enabled=true | rg 'apiGroups: \["streamforge.io"\]'
```

On Apple Silicon Minikube, build and load a local UI image because the published `ghcr.io/rahulbsw/streamforge-ui:latest` image may not include a `linux/arm64` manifest:

```bash
docker build -f ui/Dockerfile -t streamforge-ui:local ui
docker save streamforge-ui:local -o /private/tmp/streamforge-ui-local.tar
minikube image load /private/tmp/streamforge-ui-local.tar
```

Install with UI enabled. Use the local UI image override on Apple Silicon:

```bash
helm upgrade --install streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --set ui.enabled=true \
  --set ui.image.repository=streamforge-ui \
  --set ui.image.tag=local \
  --set ui.image.pullPolicy=IfNotPresent
```

On an amd64 recording machine where the published UI image pulls successfully, omit the three `ui.image.*` overrides:

```bash
helm upgrade --install streamforge-operator ./helm/streamforge-operator \
  --namespace streamforge-system \
  --create-namespace \
  --set ui.enabled=true
```

Verify operator, UI, service, CRD, and RBAC:

```bash
kubectl get pods -n streamforge-system
kubectl get svc -n streamforge-system
kubectl get crd | rg streamforge
kubectl auth can-i list streamforgepipelines.streamforge.io \
  --as=system:serviceaccount:streamforge-system:streamforge-ui \
  -n streamforge-system
```

If using local port-forward:

```bash
kubectl port-forward -n streamforge-system svc/streamforge-operator-ui 3001:3001
```

For a self-contained local Kubernetes recording, deploy a small in-cluster Redpanda broker:

```bash
kubectl create namespace redpanda

kubectl create deployment redpanda -n redpanda \
  --image=docker.redpanda.com/redpandadata/redpanda:v25.1.2 \
  -- /entrypoint.sh redpanda start \
    --overprovisioned \
    --smp 1 \
    --memory 1G \
    --reserve-memory 0M \
    --check=false \
    --node-id 0 \
    --kafka-addr PLAINTEXT://0.0.0.0:9092 \
    --advertise-kafka-addr PLAINTEXT://redpanda.redpanda.svc.cluster.local:9092

kubectl expose deployment redpanda -n redpanda \
  --port=9092 \
  --target-port=9092 \
  --name=redpanda

kubectl rollout status deployment/redpanda -n redpanda --timeout=180s
kubectl exec -n redpanda deployment/redpanda -- rpk cluster info
kubectl exec -n redpanda deployment/redpanda -- \
  rpk topic create raw-orders-ui-demo analytics-orders-ui-demo
```

UI values for the pipeline form:

- Pipeline name: `ui-orders-demo`
- Namespace: `streamforge-system`
- Application ID: `ui-orders-demo`
- Source bootstrap: `redpanda.redpanda.svc.cluster.local:9092`
- Source topic: `raw-orders-ui-demo`
- Consumer group: `streamforge-ui-demo`
- Offset: `earliest`
- Destination bootstrap: `redpanda.redpanda.svc.cluster.local:9092`
- Destination topic: `analytics-orders-ui-demo`
- Filter: `/region,==,us`
- Transform: `CONSTRUCT:order_id=/order_id:amount=/amount:region=/region`
- Replicas: `1`
- Threads: `2`

After the UI create step, verify Kubernetes resources:

```bash
kubectl get sfp ui-orders-demo -n streamforge-system -o yaml
kubectl get pods -n streamforge-system -l streamforge.io/pipeline=ui-orders-demo
kubectl get configmap ui-orders-demo-config -n streamforge-system -o jsonpath='{.data.config\.yaml}'
```

Produce and consume one event:

```bash
printf '%s\n' \
  '{"order_id":"ord-ui-demo-1001","customer":{"id":"cust-42","email":"alice@example.com"},"amount":125,"region":"us","created_at":"2026-05-25T20:35:00Z"}' \
  | kubectl exec -i -n redpanda deployment/redpanda -- \
      rpk topic produce raw-orders-ui-demo

kubectl exec -n redpanda deployment/redpanda -- \
  rpk topic consume analytics-orders-ui-demo -n 1 --offset start
```

With the current chart default pipeline image `ghcr.io/rahulbsw/streamforge:0.3.0`, the UI-created pipeline verifies deployment and Kafka output, but the consumed value may be the raw mirrored event even when a transform is present in the generated ConfigMap. Do not claim transformed output in the recording unless the pipeline image is updated and verified with transformed output.

Cleanup:

```bash
kubectl delete sfp ui-orders-demo -n streamforge-system
helm uninstall streamforge-operator -n streamforge-system
kubectl delete namespace streamforge-system
kubectl delete namespace redpanda
```

**Expected Proof Points:**

- Helm install succeeds.
- UI is reachable.
- Pipeline can be created in form mode.
- Generated YAML is visible before deploy.
- Pipeline becomes Kubernetes state.
- Kafka output verifies that the deployed pipeline is consuming and producing.
- With the current chart default pipeline image, do not claim transform verification unless the output has been re-tested with an updated image.

**Human Audio Script:**

"This demo is for platform teams. A pipeline workflow should be usable, but it should also be reviewable and operable."

"I am installing StreamForge with Helm. The operator gives us Kubernetes-native pipeline management, and the UI gives users a guided way to create pipelines."

"The important part is the YAML preview. The UI is not a black box; the pipeline becomes declarative state."

"After deploying the CRD, I verify behavior from Kafka. A successful UI save is not enough. The proof is input event in, pipeline output out."

"This is the platform story: guided creation for users, Kubernetes control for operators."

**YouTube Metadata:**

- Title: `StreamForge Kubernetes Demo: Helm, Operator, UI, and Kafka Pipeline`
- Description: `Install StreamForge with Helm, create a pipeline in the UI, preview generated YAML, deploy the CRD, and verify Kafka output.`
- Thumbnail text: `UI -> YAML -> CRD`
- Pinned comment: `The UI demo path is documented in docs/UI_MINIKUBE_DEMO.md. The Helm chart lives under helm/streamforge-operator.`

**Publish Copy:** Use Demo 2 from `social-posts.md`.

**Dry-Run Result: 2026-05-25**

- Minikube started and kubectl context switched to `minikube`.
- `helm lint ./helm/streamforge-operator` passed.
- Helm rendering initially exposed a UI RBAC bug: the UI ClusterRole granted `streaming.streamforge.dev`, while the CRD and UI API use `streamforge.io`.
- Patched `helm/streamforge-operator/templates/ui-rbac.yaml` to grant `streamforge.io`.
- `kubectl auth can-i list streamforgepipelines.streamforge.io --as=system:serviceaccount:streamforge-system:streamforge-ui -n streamforge-system` changed from `no` to `yes`.
- On Apple Silicon Minikube, `ghcr.io/rahulbsw/streamforge-ui:latest` failed with `no matching manifest for linux/arm64/v8`.
- Built `streamforge-ui:local`, exported it to `/private/tmp/streamforge-ui-local.tar`, loaded it into Minikube, and installed the chart with the local UI image override.
- Operator and UI pods reached `Running`.
- UI login worked with `admin` / `admin`.
- UI pipeline listing worked after the RBAC fix.
- The repo's `examples/kubernetes/kafka/kafka-standalone.yaml` did not work cleanly in this dry run; the Kafka container failed with an `advertised.listeners` error.
- An in-cluster Redpanda broker worked when deployed through `/entrypoint.sh redpanda start ...`.
- Created `raw-orders-ui-demo` and `analytics-orders-ui-demo`.
- Created `ui-orders-demo` through the UI form and previewed the generated YAML.
- `StreamforgePipeline` resource was created in `streamforge-system`.
- Pipeline pod reached `Running` with `ghcr.io/rahulbsw/streamforge:0.3.0`.
- Produced one event to `raw-orders-ui-demo`.
- Consumed one event from `analytics-orders-ui-demo`.
- The consumed event verified runtime deployment and Kafka output, but it was a raw mirrored event with the chart default `0.3.0` image. Record the current Demo 2 as UI -> YAML -> CRD -> running pipeline -> Kafka output, not as transform verification, unless a newer pipeline image is built/published and re-tested.

## Package 3: PII-Safe Data Engineering Pipeline

**Objective:** Show how StreamForge creates a safer downstream analytics contract by filtering, projecting, and hashing or removing sensitive fields.

**Target Viewer:** Data engineers, analytics engineers, privacy-aware platform teams, and compliance-adjacent teams.

**Screen Plan:**

1. Show a raw event with customer identifiers.
2. Show `examples/production/pii-redaction.yaml`.
3. Show `docs/marketing/streamforge-launch/configs/pii-redaction-local.yaml` for the local recording.
4. Validate both configs.
5. Run StreamForge.
6. Produce the raw event.
7. Consume analytics, marketing, third-party, and compliance outputs.
8. Point out where raw email, name, and IP are absent, and where full internal compliance data is intentionally retained.

**Terminal Commands:**

Pre-build before recording:

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/pii-redaction.yaml
cargo run --quiet --bin streamforge-validate -- docs/marketing/streamforge-launch/configs/pii-redaction-local.yaml
cargo build --release --bin streamforge
```

Main recording commands:

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
docker compose -f examples/redpanda/docker-compose.yml ps
```

In a second terminal, reset and create topics before starting StreamForge:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic delete user-events-raw user-events-analytics user-events-marketing \
    events-third-party user-events-compliance pii-redaction-dlq || true

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic create user-events-raw user-events-analytics user-events-marketing \
    events-third-party user-events-compliance pii-redaction-dlq
```

Back in the first terminal:

```bash
CONFIG_FILE=docs/marketing/streamforge-launch/configs/pii-redaction-local.yaml ./target/release/streamforge
```

Continue in the second terminal:

```bash
printf '%s\n' \
  '{"event_type":"account_created","timestamp":"2026-05-25T21:00:00Z","region":"us","device_type":"ios","email":"alice@example.com","user":{"id":"user-42","email":"alice@example.com","name":"Alice Example"},"consent":{"marketing":true,"third_party":true},"properties":{"non_pii":{"plan":"pro","source":"mobile"},"pii":{"ip":"203.0.113.10"}},"data":{"user_id":"user-42","email":"alice@example.com","name":"Alice Example","event_type":"account_created","region":"us"}}' \
  | docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
      rpk topic produce user-events-raw

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume user-events-analytics -n 1 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume user-events-marketing -n 1 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume events-third-party -n 1 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume user-events-compliance -n 1 --offset start

curl http://localhost:8080/health
curl http://localhost:8080/metrics | rg "streamforge_messages_(consumed|produced)|streamforge_consumer_lag"
```

Cleanup:

```bash
docker compose -f examples/redpanda/docker-compose.yml down
```

**Expected Proof Points:**

- Production PII redaction config validates unchanged.
- Local recording config validates and points at `localhost:9092`.
- Raw event contains sensitive fields.
- Analytics output contains only approved analytics fields and uses a SHA-256 user ID key.
- Marketing output contains only event metadata and uses an MD5 email hash key.
- Third-party output contains only event type and `properties.non_pii`.
- Compliance output intentionally contains full internal compliance data.
- Raw email, raw name, and IP address are absent from the analytics, marketing, and third-party values.

**Human Audio Script:**

"Operational topics often contain more than analytics systems should receive."

"This config is explicit about what leaves the source boundary. It filters the event, projects approved fields, and keeps raw email, name, and IP out of the lower-trust outputs."

"Projection is safer than relying on every downstream consumer to ignore fields."

"There is also a compliance destination. That one keeps the fuller internal payload, which is useful for audit or regulated workflows, but it is separate from analytics and third-party outputs."

"StreamForge does not replace governance, access control, or audit logs. It gives data teams a concrete enforcement point in the Kafka path."

**YouTube Metadata:**

- Title: `PII-Safe Kafka Pipelines: Filter, Transform, and Redact with StreamForge`
- Description: `Use StreamForge to create analytics-safe Kafka topics by filtering events, projecting approved fields, and keeping raw PII out of lower-trust outputs.`
- Thumbnail text: `PII Out`
- Pinned comment: `The production example is examples/production/pii-redaction.yaml. Review your own governance requirements before using any data minimization pattern in production.`

**Publish Copy:** Use Demo 3 from `social-posts.md`.

**Dry-Run Result: 2026-05-25**

- `examples/production/pii-redaction.yaml` validation passed unchanged.
- `docs/marketing/streamforge-launch/configs/pii-redaction-local.yaml` validation passed with four destinations and no warnings.
- Redpanda started from `examples/redpanda/docker-compose.yml`.
- The six demo topics were reset and recreated before producing the sample event.
- StreamForge started cleanly from `./target/release/streamforge`.
- Produced sample event at `user-events-raw` offset `0`.
- `user-events-analytics` output key was SHA-256 hash `6d894aa3ee802549d7f340e7c1cf0d1c1cb14cd84f768d92ffaa6785337c4997`.
- `user-events-analytics` output value:

```json
{"device":"ios","event_type":"account_created","region":"us","timestamp":"2026-05-25T21:00:00Z","user_id":"user-42"}
```

- `user-events-marketing` output key was MD5 hash `c160f8cc69a4f0bf2b0362752353d060`.
- `user-events-marketing` output value:

```json
{"anonymous_id":"user-42","event":"account_created","timestamp":"2026-05-25T21:00:00Z"}
```

- `events-third-party` output key was SHA-256 hash `6d894aa3ee802549d7f340e7c1cf0d1c1cb14cd84f768d92ffaa6785337c4997`.
- `events-third-party` output value:

```json
{"event":"account_created","properties":{"plan":"pro","source":"mobile"}}
```

- `user-events-compliance` output key was `user-42`.
- `user-events-compliance` output value:

```json
{"email":"alice@example.com","event_type":"account_created","name":"Alice Example","region":"us","user_id":"user-42"}
```

- Health endpoint returned `OK`.
- Metrics showed `streamforge_messages_consumed_total 1`, one produced message for each destination, and `streamforge_consumer_lag` at `0`.
- External values verified absent raw `alice@example.com`, `Alice Example`, and `203.0.113.10` in analytics, marketing, and third-party topics.
- If `streamforge-validate` prints `xcrun` cache warnings on macOS, treat those as local toolchain noise when validation still exits `0`.

## Package 4: CDC to Data Lake Pipeline

**Objective:** Show StreamForge as a lightweight shaping layer between CDC topics and lake or warehouse consumers.

**Target Viewer:** Data engineers, lakehouse pipeline owners, analytics platform teams.

**Screen Plan:**

1. Show a Debezium-style CDC envelope.
2. Show `examples/production/cdc-to-datalake.yaml`.
3. Show `docs/marketing/streamforge-launch/configs/cdc-to-datalake-local.yaml` for the local recording.
4. Explain create/update/delete/schema-change routing.
5. Validate both configs.
6. Run StreamForge.
7. Produce sample CDC records.
8. Consume shaped outputs.

**Terminal Commands:**

Pre-build before recording:

```bash
cargo run --quiet --bin streamforge-validate -- examples/production/cdc-to-datalake.yaml
cargo run --quiet --bin streamforge-validate -- docs/marketing/streamforge-launch/configs/cdc-to-datalake-local.yaml
cargo build --release --bin streamforge
```

Main recording commands:

```bash
docker compose -f examples/redpanda/docker-compose.yml up -d
docker compose -f examples/redpanda/docker-compose.yml ps
```

In a second terminal, reset and create topics before starting StreamForge:

```bash
docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic delete dbserver.inventory.orders datalake-orders \
    datalake-orders-deleted datalake-schema-changes cdc-datalake-dlq || true

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic create dbserver.inventory.orders datalake-orders \
    datalake-orders-deleted datalake-schema-changes cdc-datalake-dlq
```

Back in the first terminal:

```bash
CONFIG_FILE=docs/marketing/streamforge-launch/configs/cdc-to-datalake-local.yaml ./target/release/streamforge
```

Continue in the second terminal:

```bash
printf '%s\n' \
  '{"payload":{"op":"c","ts_ms":1779742500000,"after":{"id":"ord-2001","customer_id":"cust-101","status":"created","amount":199.5,"updated_at":"2026-05-25T21:05:00Z"},"before":null,"source":{"db":"inventory","table":"orders"}}}' \
  '{"payload":{"op":"u","ts_ms":1779742560000,"after":{"id":"ord-2001","customer_id":"cust-101","status":"paid","amount":199.5,"updated_at":"2026-05-25T21:06:00Z"},"before":{"id":"ord-2001","customer_id":"cust-101","status":"created","amount":199.5,"updated_at":"2026-05-25T21:05:00Z"},"source":{"db":"inventory","table":"orders"}}}' \
  '{"payload":{"op":"d","ts_ms":1779742620000,"after":null,"before":{"id":"ord-2002","customer_id":"cust-202","status":"cancelled","amount":49.95,"updated_at":"2026-05-25T21:07:00Z"},"source":{"db":"inventory","table":"orders"}}}' \
  '{"payload":{"op":"s","ts_ms":1779742680000,"ddl":"ALTER TABLE orders ADD COLUMN coupon_code VARCHAR(32)","source":{"db":"inventory","table":"orders"}}}' \
  | docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
      rpk topic produce dbserver.inventory.orders

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume datalake-orders -n 2 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume datalake-orders-deleted -n 1 --offset start

docker compose -f examples/redpanda/docker-compose.yml exec -T redpanda \
  rpk topic consume datalake-schema-changes -n 1 --offset start

curl http://localhost:8080/health
curl http://localhost:8080/metrics | rg "streamforge_messages_(consumed|produced)|streamforge_consumer_lag"
```

Cleanup:

```bash
docker compose -f examples/redpanda/docker-compose.yml down
```

**Expected Proof Points:**

- Production and local CDC configs validate.
- Create and update records route to `datalake-orders` and emit only the `payload.after` row.
- Delete records route to `datalake-orders-deleted` and emit `id` plus `deleted_at`.
- Schema-change records route to `datalake-schema-changes`.
- Metrics show four consumed source messages, two produced lake order rows, one produced delete row, one produced schema-change row, and zero lag.
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

**Dry-Run Result: 2026-05-25**

- `examples/production/cdc-to-datalake.yaml` validation passed.
- `docs/marketing/streamforge-launch/configs/cdc-to-datalake-local.yaml` validation passed with three destinations and no warnings.
- The dry run caught and fixed a config issue in the production example: `EXTRACT:/payload/after,order` validated but did not transform at runtime because the supported extraction syntax is `/payload/after`.
- Redpanda started from `examples/redpanda/docker-compose.yml`.
- The five CDC demo topics were reset and recreated before producing sample events.
- StreamForge started cleanly from `./target/release/streamforge`.
- Produced four sample CDC events at `dbserver.inventory.orders` offsets `0` through `3`.
- `datalake-orders` received the create row with key `ord-2001`:

```json
{"amount":199.5,"customer_id":"cust-101","id":"ord-2001","status":"created","updated_at":"2026-05-25T21:05:00Z"}
```

- `datalake-orders` received the update row with key `ord-2001`:

```json
{"amount":199.5,"customer_id":"cust-101","id":"ord-2001","status":"paid","updated_at":"2026-05-25T21:06:00Z"}
```

- `datalake-orders-deleted` received the delete row with key `ord-2002`:

```json
{"deleted_at":1779742620000,"id":"ord-2002"}
```

- `datalake-schema-changes` received the schema-change payload:

```json
{"ddl":"ALTER TABLE orders ADD COLUMN coupon_code VARCHAR(32)","op":"s","source":{"db":"inventory","table":"orders"},"ts_ms":1779742680000}
```

- Health endpoint returned `OK`.
- Metrics showed `streamforge_messages_consumed_total 4`, produced counts of `2` for `datalake-orders`, `1` for `datalake-orders-deleted`, `1` for `datalake-schema-changes`, and `streamforge_consumer_lag` at `0`.
- If `streamforge-validate` prints `xcrun` cache warnings on macOS, treat those as local toolchain noise when validation still exits `0`.

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
