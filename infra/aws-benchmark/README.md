# Private AWS benchmark infrastructure

This module provisions one short-lived, dedicated StreamForge benchmark
environment in `us-west-2a`. It creates a CodeBuild project and private ECR
repository first. Billable runtime capacity remains off until
`provision_runtime=true`; the module never starts the one-shot task.

## Security and cost boundary

- One On-Demand `c7i.2xlarge` in an Auto Scaling group fixed at one instance.
- One private subnet with no internet gateway, NAT gateway, public IPv4, load
  balancer, SSH key, or inbound workload security-group rule.
- ECS, ECS agent, ECS telemetry, ECR API, ECR Docker, and CloudWatch Logs
  interface endpoints. S3 uses a gateway endpoint.
- The endpoint security group accepts TCP/443 only from the instance and task
  security groups. These are private security-group references, not public
  ingress.
- The root disk is an encrypted, delete-on-termination 40 GiB gp3 volume with
  3,000 IOPS and 125 MiB/s throughput. IMDSv2 is required.
- The runner image lives in a private, encrypted, immutable ECR repository.
  The same image is used for Kafka with its entry point overridden because the
  runner's pinned final base is the Kafka image. No runtime image pull reaches
  Docker Hub. Artifacts use a private, encrypted, force-destroy S3 bucket.
- An absolute Auto Scaling scheduled action forces min/max/desired capacity to
  zero at the required `ExpiresAt` time.

The hard expiry stops the dominant EC2 charge. PrivateLink endpoints, S3, ECR,
and CloudWatch Logs continue to exist and may continue accruing small charges
until `terraform destroy` completes.

## Task shape

The task uses EC2 launch type and `awsvpc`. Kafka and the runner share the task
network namespace, so Kafka is reachable only at `127.0.0.1:9092`; no Kafka
port is published. StreamForge and the three independent benchmark jobs run
inside the runner container, allowing resource measurements without host or
cross-container PID access. Task-scoped control and Kafka-data volumes support
barriers, artifacts, and physical Kafka cleanup checks without a host mount.

The runner is built from `scripts/benchmarks/aws/Dockerfile` and enters through
`/opt/streamforge/scripts/benchmarks/aws/run_ecs_benchmark.sh`. It uses the
maintained controller's ECS-direct mode and must preserve this contract:

- persistent ingress and completed warm-up before the monotonic barrier;
- at least three repetitions of at least 120 seconds each;
- exact input, consumed, produced, delivered, output, and independent observer
  counts with zero processing errors;
- post-window output validation;
- payload hash, clean Git revision, configuration, environment, CPU, and RSS in
  the result manifest;
- artifact upload below
  `s3://$BENCHMARK_ARTIFACT_BUCKET/$BENCHMARK_RUN_ID/` before exit.

Keep `provision_runtime=false` until CodeBuild has pushed the exact image and
its ECR scan has been reviewed.

## Initialize and review

Terraform 1.5.7 or newer and AWS provider credentials for the selected sandbox
account are required. `owner` and the absolute UTC expiry are mandatory.

```bash
cd infra/aws-benchmark
terraform init
terraform plan \
  -var='owner=streamforge-team' \
  -var='expires_at=2026-07-26T03:00:00Z' \
  -var='source_revision=0123456789abcdef0123456789abcdef01234567' \
  -var='benchmark_run_id=sf-aws-20260726-001'
```

Use an expiry comfortably after image upload and the benchmark window, but no
longer than necessary. Review the plan for zero resources of these types:

- internet/NAT gateways;
- public IPs or Elastic IPs;
- load balancers;
- security-group rules sourced from CIDRs;
- SSH keys.

## Build the private runner image

Apply only the build/storage phase first:

```bash
terraform apply \
  -var='owner=streamforge-team' \
  -var='expires_at=2026-07-26T03:00:00Z' \
  -var='source_revision=0123456789abcdef0123456789abcdef01234567' \
  -var='benchmark_run_id=sf-aws-20260726-001'
aws codebuild start-build \
  --region us-west-2 \
  --project-name "$(terraform output -raw codebuild_project_name)"
```

CodeBuild needs outbound access while it fetches the reviewed Git revision,
pinned base images, Rust toolchain, and AWS CLI. It is an AWS-managed ephemeral
builder and has no listener or inbound path into the benchmark VPC.

After the build succeeds, verify the ECR scan and resolve the pushed tag to its
digest. Then enable runtime capacity:

```bash
terraform apply \
  -var='owner=streamforge-team' \
  -var='expires_at=2026-07-26T03:00:00Z' \
  -var='source_revision=0123456789abcdef0123456789abcdef01234567' \
  -var='benchmark_run_id=sf-aws-20260726-001' \
  -var='provision_runtime=true' \
  -var='benchmark_runner_image=123456789012.dkr.ecr.us-west-2.amazonaws.com/streamforge-aws-benchmark/runner@sha256:REPLACE'
```

The Kafka container defaults to that same private image. It overrides the image
entry point to start its pinned KRaft broker, so ZooKeeper is not required.

## Run and observe

Wait until the single ECS container instance is `ACTIVE`, then run the command
from:

```bash
terraform output -raw run_task_command
```

There is no SSH, ECS Exec, or public dashboard. Observe the task through the
private CloudWatch Logs destination:

```bash
aws logs tail "$(terraform output -raw cloudwatch_log_group)" \
  --region us-west-2 \
  --follow
```

Download the schema-version-3 manifest and job artifacts from the private S3
bucket only after the task exits successfully.

## Teardown

Stop any still-running task, then destroy the complete environment:

```bash
aws ecs list-tasks \
  --region us-west-2 \
  --cluster "$(terraform output -json ecs_cluster | jq -r .name)"
terraform destroy \
  -var='owner=streamforge-team' \
  -var='expires_at=2026-07-26T03:00:00Z' \
  -var='source_revision=0123456789abcdef0123456789abcdef01234567' \
  -var='benchmark_run_id=sf-aws-20260726-001' \
  -var='provision_runtime=true'
```

The ECR repositories and S3 bucket intentionally use force deletion because
the environment is disposable. After destroy, inventory the `Project`,
`Owner`, and `ExpiresAt` tags and verify zero live ECS tasks, EC2 instances,
EBS volumes, endpoint ENIs, public IPs, and benchmark S3/ECR resources.
