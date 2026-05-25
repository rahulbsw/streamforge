# StreamForge AWS Production Demo Runbook

This runbook supports the AWS production-style campaign video. It is designed for recording a credible cloud deployment without pretending to be a full production hardening guide.

Official references:

- Amazon EKS getting started with `eksctl`: https://docs.aws.amazon.com/eks/latest/userguide/getting-started-eksctl.html
- Amazon MSK Serverless guide: https://docs.aws.amazon.com/en_us/msk/latest/developerguide/serverless-getting-started.html
- Amazon MSK Serverless overview and IAM note: https://docs.aws.amazon.com/en_us/msk/latest/developerguide/serverless.html
- Amazon MSK pricing: https://aws.amazon.com/msk/pricing/
- StreamForge security configuration: `docs/SECURITY_CONFIGURATION.md`
- StreamForge Helm chart: `helm/streamforge-operator/README.md`

## Scope

Record a temporary AWS deployment that shows:

- EKS running StreamForge.
- Amazon MSK or a clearly named Kafka-compatible broker as the source and destination broker.
- Helm/operator deployment.
- Pipeline config with secure connection settings.
- Kafka-level verification: produce an input event, consume transformed output.
- Metrics or lag visibility.
- Cleanup and cost control.

The demo should not claim that this is a complete production architecture. It should show the deployment pattern and verification path.

## Recommended Architecture

Use this architecture for the main video:

```text
Developer terminal
  |
  | kubectl / helm
  v
Amazon EKS cluster
  |
  | StreamForge operator + StreamForge pipeline pods
  v
Amazon MSK cluster
  |
  | source topic: raw-orders
  | destination topics: analytics-orders, pii-safe-orders
  v
Kafka-compatible consumers
```

Recommended recording path:

1. Use EKS for StreamForge.
2. Use Amazon MSK Provisioned with SASL/SCRAM or TLS if you want the closest match to StreamForge's existing documented security configuration.
3. Use MSK Serverless only after a connection smoke test, because MSK Serverless requires IAM access control. Confirm the StreamForge container and librdkafka configuration can authenticate before making it the public recording path.
4. If MSK auth setup blocks the recording, use Redpanda or Kafka inside EKS and label it clearly as the Kafka-compatible fallback. Keep the AWS demo focused on EKS, Helm, operator, metrics, and cleanup.

## Cost Controls

Do these before recording:

- Use a dedicated AWS region for the demo, such as `us-west-2`.
- Add tags to every resource:
  - `Project=streamforge-demo`
  - `Owner=<your-aws-user-or-team>`
  - `Expires=<recording-date>`
- Create or confirm an AWS Budget alert before provisioning.
- Keep the EKS node count small.
- Keep topics and partitions small for the demo.
- Avoid cross-region traffic unless the video is specifically about cross-region behavior.
- Delete resources immediately after recording.

AWS pricing changes over time. Check the current Amazon MSK pricing page before recording: https://aws.amazon.com/msk/pricing/

## Local Prerequisites

Install and authenticate:

```bash
aws --version
kubectl version --client
eksctl version
helm version
docker version
```

Verify account and region:

```bash
export AWS_REGION=us-west-2
export EKS_CLUSTER=streamforge-demo
export NAMESPACE=streamforge

aws sts get-caller-identity
aws configure get region
```

If `aws configure get region` does not match `AWS_REGION`, set it:

```bash
aws configure set region "$AWS_REGION"
```

## Recording Flow

Use this flow in the video:

1. Show the architecture diagram or explain it in one terminal-friendly view.
2. Show the cleanup plan before provisioning.
3. Create or show the EKS cluster.
4. Show MSK bootstrap servers or the Kafka-compatible fallback broker.
5. Install StreamForge operator and UI with Helm.
6. Create Kubernetes secrets for Kafka credentials if using SASL/TLS.
7. Apply a StreamForge pipeline config.
8. Produce a raw event.
9. Consume transformed destination topics.
10. Show health, metrics, pods, and logs.
11. Run cleanup commands.

## EKS Setup

Create a small temporary cluster:

```bash
eksctl create cluster \
  --name "$EKS_CLUSTER" \
  --region "$AWS_REGION" \
  --nodes 2 \
  --node-type t3.large \
  --tags Project=streamforge-demo,Expires=recording
```

Verify:

```bash
kubectl get nodes -o wide
kubectl get pods -A
```

If the cluster already exists:

```bash
aws eks update-kubeconfig --name "$EKS_CLUSTER" --region "$AWS_REGION"
kubectl get nodes
```

## Kafka Broker Setup

### Option A: MSK Provisioned with SASL/SCRAM or TLS

Use this option for the public recording if it is already available in the AWS account. It aligns best with StreamForge's existing security docs and examples.

Capture these values:

```bash
export KAFKA_BOOTSTRAP="b-1.example:9096,b-2.example:9096,b-3.example:9096"
export KAFKA_SECURITY_PROTOCOL="SASL_SSL"
export KAFKA_SASL_MECHANISM="SCRAM-SHA-512"
```

Create Kubernetes secrets:

```bash
kubectl create namespace "$NAMESPACE"

kubectl create secret generic kafka-sasl-credentials \
  --from-literal=username="$KAFKA_USERNAME" \
  --from-literal=password="$KAFKA_PASSWORD" \
  -n "$NAMESPACE"
```

If using a custom CA bundle:

```bash
kubectl create secret generic kafka-ca-cert \
  --from-file=ca.crt=./ca.crt \
  -n "$NAMESPACE"
```

### Option B: MSK Serverless

MSK Serverless is attractive for demos because capacity is managed by AWS, but it requires IAM access control. Use it only after verifying StreamForge can authenticate from the recording environment.

Smoke test checklist:

- EKS pod can resolve and reach the MSK bootstrap endpoint.
- StreamForge client configuration supports the IAM auth method used by the MSK cluster.
- A test producer can create or write to `raw-orders`.
- A test consumer can read from `analytics-orders`.

If the smoke test fails, do not make MSK Serverless the recorded path. Use Option A or the fallback broker and mention MSK Serverless as a follow-up target after IAM auth validation.

### Option C: Kafka-Compatible Fallback on EKS

Use this only if MSK setup blocks recording. Label it clearly as a Kafka-compatible fallback, not as Amazon MSK.

The video can still demonstrate:

- EKS.
- Helm/operator deployment.
- Kubernetes pipeline management.
- Produce and consume verification.
- Metrics and logs.
- Cleanup.

## Install StreamForge

Install operator and UI:

```bash
helm install streamforge ./helm/streamforge-operator \
  --namespace "$NAMESPACE" \
  --create-namespace
```

Verify:

```bash
kubectl get pods -n "$NAMESPACE"
kubectl get svc -n "$NAMESPACE"
kubectl get crd | rg streamforge
```

For UI recording, port-forward if needed:

```bash
kubectl port-forward svc/streamforge-ui 3000:3000 -n "$NAMESPACE"
```

## Pipeline Config for Recording

Use a pipeline based on the selective replication demo. Keep topics small and obvious:

- Source: `raw-orders`
- Destination 1: `analytics-orders`
- Destination 2: `pii-safe-orders`

For secure clusters, use the existing Kubernetes secret pattern from `examples/pipelines/secure-sasl-pipeline.yaml` or `examples/pipelines/secure-tls-pipeline.yaml`.

Export the captured bootstrap string:

```bash
export KAFKA_BOOTSTRAP="b-1.streamforge-demo.example.c2.kafka.us-west-2.amazonaws.com:9096,b-2.streamforge-demo.example.c2.kafka.us-west-2.amazonaws.com:9096,b-3.streamforge-demo.example.c2.kafka.us-west-2.amazonaws.com:9096"
```

Use this CRD template shape for the recording. Render it with `envsubst` or by using the same value directly in the UI/YAML editor before applying it:

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: aws-orders-selective-replication
  namespace: streamforge
spec:
  source:
    brokers: "${KAFKA_BOOTSTRAP}"
    topic: "raw-orders"
    offset: "earliest"
    groupId: "streamforge-aws-demo"
    security:
      protocol: "SASL_SSL"
      sasl:
        mechanism: "SCRAM-SHA-512"
        usernameSecret:
          name: kafka-sasl-credentials
          key: username
        passwordSecret:
          name: kafka-sasl-credentials
          key: password
  destinations:
    - brokers: "${KAFKA_BOOTSTRAP}"
      topic: "analytics-orders"
      security:
        protocol: "SASL_SSL"
        sasl:
          mechanism: "SCRAM-SHA-512"
          usernameSecret:
            name: kafka-sasl-credentials
            key: username
          passwordSecret:
            name: kafka-sasl-credentials
            key: password
    - brokers: "${KAFKA_BOOTSTRAP}"
      topic: "pii-safe-orders"
      security:
        protocol: "SASL_SSL"
        sasl:
          mechanism: "SCRAM-SHA-512"
          usernameSecret:
            name: kafka-sasl-credentials
            key: username
          passwordSecret:
            name: kafka-sasl-credentials
            key: password
  replicas: 2
  threads: 4
  appid: "streamforge-aws-demo"
```

Before recording, render the manifest with the real bootstrap string and confirm the operator accepts the exact spec shape.

## Verification

Show these checks on camera:

```bash
kubectl get pods -n "$NAMESPACE"
kubectl get streamforgepipeline -n "$NAMESPACE"
kubectl logs -n "$NAMESPACE" deploy/streamforge-operator --tail=100
```

Create topics using the Kafka admin path available for the selected broker. For MSK Provisioned, use either AWS topic APIs where supported or Kafka tools from a client with network access to the cluster.

Produce one event:

```json
{"order_id":"ord-aws-demo-1001","customer":{"id":"cust-42","email":"alice@example.com"},"amount":125,"region":"us","created_at":"2026-05-25T18:00:00Z"}
```

Consume and verify:

- `analytics-orders` contains the approved business fields.
- `pii-safe-orders` does not expose raw customer email in the value payload.
- StreamForge logs show successful processing.
- Metrics or lag output shows the pipeline is alive.

## Observability Shots

Capture at least three:

```bash
kubectl get pods -n "$NAMESPACE"
kubectl top pods -n "$NAMESPACE"
kubectl logs -n "$NAMESPACE" -l app=streamforge --tail=100
```

If metrics are exposed:

```bash
kubectl port-forward svc/streamforge-metrics 9090:9090 -n "$NAMESPACE"
curl http://localhost:9090/health
curl http://localhost:9090/metrics
```

Useful metrics to mention:

- messages consumed
- messages produced per destination
- messages filtered
- processing latency
- processing errors
- consumer lag

## Cleanup

Run cleanup before ending the recording session.

Delete StreamForge resources:

```bash
helm uninstall streamforge -n "$NAMESPACE"
kubectl delete namespace "$NAMESPACE"
```

Delete the EKS cluster:

```bash
eksctl delete cluster --name "$EKS_CLUSTER" --region "$AWS_REGION"
```

Delete or stop broker resources:

- Delete the MSK demo cluster if it was created only for recording.
- Delete demo topics if the MSK cluster is shared and should remain.
- Delete ECR images created only for the demo.
- Delete temporary CloudWatch log groups if they were created only for the demo.

Verify remaining tagged resources:

```bash
aws resourcegroupstaggingapi get-resources \
  --tag-filters Key=Project,Values=streamforge-demo \
  --region "$AWS_REGION"
```

## Safety Notes

- Do not show real account IDs, access keys, private bootstrap endpoints, passwords, or production topic names in the recording.
- Use a temporary AWS account or sandbox account if possible.
- Use demo credentials and demo topics only.
- Blur AWS account identifiers in the console.
- Do not record secrets, kubeconfig contents, or terminal history containing credentials.
- Keep IAM permissions scoped to the demo where practical.
- Say "production-style" instead of "production-ready" unless all hardening is actually complete.

## Recording Checklist

- Budget alert exists.
- Region and AWS account are confirmed.
- Cleanup commands are prepared before resource creation.
- EKS cluster is reachable.
- Kafka broker is reachable from EKS.
- StreamForge Helm install succeeds.
- Pipeline config applies cleanly.
- Input event is produced.
- Destination output is consumed.
- Metrics or logs are captured.
- Resources are deleted or intentionally retained with tags and owner.
