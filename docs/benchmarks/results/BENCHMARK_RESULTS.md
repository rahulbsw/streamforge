# AWS Sustained Passthrough Baseline

**Status:** Diagnostic baseline; not publication eligible

**Measured:** 2026-07-25 UTC

**Source commit:** `d848f118e62b41c7605250c69c9da087af688d0c`

This is the canonical sustained Kafka baseline for StreamForge. It replaces
the prior Criterion-only report at this path. Criterion results remain useful
for detecting code-path regressions, but they are not end-to-end Kafka
throughput.

## Result

| Metric | Value |
|---|---:|
| Median output-delivery rate | **106,692.65 msg/s** |
| Mean output-delivery rate | 106,714.30 msg/s |
| Minimum | 106,651.68 msg/s |
| Maximum | 106,798.58 msg/s |
| Coefficient of variation | 0.0580% |
| Median StreamForge mean CPU | 1.124 cores |
| Median StreamForge peak RSS | 137.1 MiB |

All three repetitions passed the accounting and repeatability checks captured
by this run. The result does not pass the current release publication gate
because p99 latency was not captured and the payload was not normalized to
exactly 1 KiB.

| Repetition | Delivered records | Output records | Errors | Rate (msg/s) | Classification |
|---:|---:|---:|---:|---:|---|
| 1 | 16,200,000 | 16,200,000 | 0 | 106,798.58 | Engine saturated |
| 2 | 16,200,000 | 16,200,000 | 0 | 106,651.68 | Engine saturated |
| 3 | 16,200,000 | 16,200,000 | 0 | 106,692.65 | Engine saturated |

For every repetition, ingress, input offsets, consumed, produced,
broker-delivered, output offsets, and independently validated output counts
were exactly 16,200,000.

## Workload

| Property | Value |
|---|---|
| Processing mode | `partition_ordered` |
| Producer delivery mode | `queued` |
| Kafka partitions | 8 |
| StreamForge threads | 8 |
| Target ingress | 135,000 msg/s |
| Warm-up | 1,000,000 untimed records per repetition |
| Timed window | 120 seconds per repetition |
| Repetitions | 3 |
| Dataset | 10,000 deterministic JSON messages, replayed |
| Host | AWS `c7i.2xlarge` |
| CPU | Intel Xeon Platinum 8488C |
| Rust | 1.89.0 |
| Container digest | `sha256:b1eb12e47f673dbef4e584e0131372127b42a08a47dadf92ded3271b2fd166d3` |

The ingress job and StreamForge were running before each timed barrier.
Startup, warm-up, drain, output validation, and teardown were excluded from the
120-second measurement window. A persistent ingress worker, timed metrics and
resource observer, and independent output validator ran as separate jobs.

## Checks satisfied by this run

The aggregate was retained as diagnostic evidence after these conditions
passed:

- schema version 3 and aggregate status `passed`;
- three complete 120-second repetitions;
- one million untimed warm-up records per repetition;
- exact ingress, offset, consumed, produced, delivered, and output counts;
- zero processing and delivery errors;
- all repetitions classified as engine-saturated;
- bounded variance across repetitions;
- source revision and environment recorded in the result manifest.

This is not a complete v1.4 publication result. A publication-eligible rerun
must also capture p99 latency and use the exact payload sizes required by the
current measurement contract.

## Security and teardown

The environment was provisioned with Terraform and ran one ECS-on-EC2 task in
a private subnet:

- no public IPv4 address;
- no internet gateway or NAT gateway;
- no load balancer, SSH key, or public listener;
- no CIDR-based inbound security-group rule;
- ECS, ECR, and CloudWatch access through six interface endpoints;
- S3 access through a gateway endpoint;
- endpoint TCP/443 ingress restricted to private security-group references.

The ECR scan completed with zero critical findings. The image still inherited
high and medium findings from its base distribution; it was accepted only for
this isolated, ephemeral benchmark and is not a production image endorsement.

After evidence collection, `terraform destroy` reported 45 resources
destroyed. Direct AWS service inventories showed zero live benchmark
instances, volumes, ENIs, VPCs, endpoints, buckets, ECR repositories,
CodeBuild projects, IAM roles, active task definitions, and Auto Scaling
groups. ECS retained only an `INACTIVE` deleted-cluster record with zero
registered, running, or pending tasks.

## Cost boundary

Billable runtime was limited to about 72 minutes, with one On-Demand
`c7i.2xlarge`, six single-AZ interface endpoints, one 40 GiB gp3 root volume,
short CodeBuild jobs, and small ECR, S3, and log artifacts. The current AWS
Pricing API quoted the instance at $0.357/hour in `us-west-2`; interface
endpoint partial hours are billed as full hours.

The run is estimated to cost less than **$1 USD** before taxes and account
discounts. This is an engineering estimate, not a finalized bill: Cost Explorer
data can lag, and data-processing, build-minute, storage, and logging charges
settle separately. The hard expiry forced the dominant compute capacity to zero
and teardown removed the endpoints that otherwise continue hourly billing.

## Interpretation

The median is about 15.0 times the old 7,094.33 msg/s profiling number, but
that number came from a superseded coupled harness that included source
publication and polling overhead. The ratio demonstrates why the old result
must not be reused; it is not an apples-to-apples optimization claim.

This baseline is also not a comparison with a Java mirror or Jackson. A matched
comparison still requires identical payloads, partitions, acknowledgement
semantics, warm-up, duration, resource limits, and independent record-count
validation.

## Reproduction

- Measurement contract: [`../../PERFORMANCE_TESTING.md`](../../PERFORMANCE_TESTING.md)
- AWS environment: [`../../../infra/aws-benchmark/README.md`](../../../infra/aws-benchmark/README.md)
- Benchmark record index: [`../README.md`](../README.md)
