# StreamForge Launch Campaign

This directory contains execution-ready assets for the approved StreamForge launch campaign.

The campaign promotes StreamForge as selective replication and data shaping for Kafka-compatible brokers, with Redpanda as a compatibility target. The message stays focused: move only the records and fields downstream systems need, with filtering, transforms, PII-safe routing, Kubernetes deployment, and observability.

## Audience

- Platform engineers operating Kafka, Redpanda, Kubernetes, Helm, and internal data platforms.
- Data engineers building analytics, CDC, lake, compliance, and downstream contract pipelines.
- AI infrastructure and MLOps teams that need safe real-time event streams without raw PII.

## Asset Map

- `youtube-demos.md` - video titles, outlines, scripts, chapters, thumbnail concepts, and success criteria.
- `social-posts.md` - X launch posts, X threads, LinkedIn posts, and hashtag sets for each video.
- `aws-demo-runbook.md` - AWS production-style demo setup, recording flow, cleanup, and cost controls.

## Publishing Order

1. Local Redpanda quickstart.
2. Kubernetes UI and operator demo.
3. PII-safe data engineering pipeline.
4. CDC to data lake pipeline.
5. AI-ready event stream.
6. AWS production deployment.
7. Observability and scaling.

## Cadence

- Week 1: publish the Local Redpanda quickstart and Kubernetes UI/operator demos.
- Week 2: publish the PII-safe data engineering pipeline and CDC to data lake demos.
- Week 3: publish the AI-ready event stream demo.
- Week 4: publish the AWS production deployment and observability/scaling demos.

For each video, publish the YouTube demo, one X launch post, one short X thread, one LinkedIn post, and one short clip. Follow up within 48 hours with a command, YAML snippet, metric, or output topic from the demo.

## Positioning Guardrails

- Present StreamForge as selective replication and data shaping for Kafka-compatible brokers.
- Use Redpanda as a compatibility target.
- Mention Kafka Connect and MirrorMaker 2 only to clarify fit, not to attack them.
- Avoid claiming StreamForge replaces MirrorMaker 2 for active-active replication, consumer offset sync, topic mirroring, ACL mirroring, joins, SQL, or broad stateful stream processing.
- Keep AI language practical: safe real-time data contracts for AI systems, not vague AI transformation claims.

## Success Metrics

- GitHub stars and forks.
- README demo clicks.
- YouTube views, average view duration, and click-through rate.
- X impressions, reposts, profile clicks, and link clicks.
- LinkedIn impressions, saves, practitioner comments, and repo clicks.
- Issues or discussions opened by users trying the demos.

The strongest signal is a practitioner reproducing a demo, opening an issue, asking for a feature, or starring the repo after watching.
