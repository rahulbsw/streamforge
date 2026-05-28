# StreamForge Launch Campaign Assets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the first complete set of campaign assets for the approved StreamForge launch campaign.

**Architecture:** Keep all campaign execution assets in `docs/marketing/streamforge-launch/` and split them by responsibility: overview/calendar, YouTube scripts, social posts, and AWS runbook. The files reference the approved campaign design but are usable directly by someone recording videos or publishing posts.

**Tech Stack:** Markdown documentation, existing StreamForge examples, existing Kubernetes/Helm/operator docs, existing Redpanda quickstart, existing observability docs.

---

## File Structure

- Create: `docs/marketing/streamforge-launch/README.md`
  Campaign execution index, audience framing, publishing sequence, and asset map.

- Create: `docs/marketing/streamforge-launch/youtube-demos.md`
  Detailed outlines, scripts, recording notes, thumbnail ideas, titles, and chapter structures for all seven demos.

- Create: `docs/marketing/streamforge-launch/social-posts.md`
  Exact X launch posts, X thread drafts, and LinkedIn post drafts for each demo.

- Create: `docs/marketing/streamforge-launch/aws-demo-runbook.md`
  AWS production demo runbook with setup, recording path, verification, cleanup, and cost controls.

## Task 1: Create Campaign Execution Index

**Files:**

- Create: `docs/marketing/streamforge-launch/README.md`

- [ ] **Step 1: Add the campaign index**

Create the file with:

```markdown
# StreamForge Launch Campaign

This directory contains execution-ready assets for the approved StreamForge launch campaign.

## Audience

- Platform engineers operating Kafka, Redpanda, Kubernetes, Helm, and internal data platforms.
- Data engineers building analytics, CDC, lake, compliance, and downstream contract pipelines.
- AI infrastructure and MLOps teams that need safe real-time event streams without raw PII.

## Asset Map

- `youtube-demos.md` - video titles, outlines, scripts, chapters, and thumbnail concepts.
- `social-posts.md` - X launch posts, X threads, and LinkedIn posts for each video.
- `aws-demo-runbook.md` - AWS production-style demo setup, recording flow, cleanup, and cost controls.

## Publishing Order

1. Local Redpanda quickstart.
2. Kubernetes UI and operator demo.
3. PII-safe data engineering pipeline.
4. CDC to data lake pipeline.
5. AI-ready event stream.
6. AWS production deployment.
7. Observability and scaling.

## Positioning Guardrails

- Present StreamForge as selective replication and data shaping for Kafka-compatible brokers.
- Use Redpanda as a compatibility target.
- Avoid claiming StreamForge replaces MirrorMaker 2 for active-active replication, consumer offset sync, topic mirroring, ACL mirroring, joins, SQL, or broad stateful stream processing.
```

- [ ] **Step 2: Verify the index**

Run: `sed -n '1,220p' docs/marketing/streamforge-launch/README.md`

Expected: the file prints without placeholder text.

## Task 2: Draft YouTube Demo Assets

**Files:**

- Create: `docs/marketing/streamforge-launch/youtube-demos.md`

- [ ] **Step 1: Add seven demo sections**

Create one section per approved demo:

- 5-Minute Local Redpanda Quickstart
- Kubernetes UI and Operator Demo
- PII-Safe Data Engineering Pipeline
- CDC to Data Lake Pipeline
- AI-Ready Event Stream
- AWS Production Deployment
- Observability and Scaling

Each section must include:

- title options
- target audience
- hook
- recording setup
- demo flow
- script beats
- thumbnail concept
- chapters
- success criteria

- [ ] **Step 2: Verify demo coverage**

Run: `rg -n "Demo [1-7]:|Success Criteria|Thumbnail" docs/marketing/streamforge-launch/youtube-demos.md`

Expected: all seven demos have success criteria and thumbnail content.

## Task 3: Draft Social Assets

**Files:**

- Create: `docs/marketing/streamforge-launch/social-posts.md`

- [ ] **Step 1: Add social post drafts for all demos**

For each demo, add:

- one X launch post
- one X technical thread with three to six posts
- one LinkedIn post
- hashtag set

The posts must stay technical, avoid hype-only phrasing, and use the audience-specific hashtag groups from the campaign design.

- [ ] **Step 2: Verify social coverage**

Run: `rg -n "X Launch Post|X Thread|LinkedIn Post|Hashtags" docs/marketing/streamforge-launch/social-posts.md`

Expected: each label appears seven times.

## Task 4: Draft AWS Demo Runbook

**Files:**

- Create: `docs/marketing/streamforge-launch/aws-demo-runbook.md`

- [ ] **Step 1: Add the AWS production demo runbook**

The runbook must include:

- scope and assumptions
- recommended AWS architecture
- recording flow
- setup checklist
- MSK/EKS deployment path
- verification steps
- observability shots
- cleanup checklist
- cost controls
- safety notes

- [ ] **Step 2: Verify runbook completeness**

Run: `rg -n "Cost Controls|Cleanup|Verification|Safety" docs/marketing/streamforge-launch/aws-demo-runbook.md`

Expected: all major runbook sections are present.

## Task 5: Final Review

**Files:**

- Review: `docs/marketing/streamforge-launch/README.md`
- Review: `docs/marketing/streamforge-launch/youtube-demos.md`
- Review: `docs/marketing/streamforge-launch/social-posts.md`
- Review: `docs/marketing/streamforge-launch/aws-demo-runbook.md`

- [ ] **Step 1: Scan for placeholders and positioning errors**

Run:

```bash
rg -n "TBD|TODO|FIXME|replace me|universal MirrorMaker|active-active replacement" docs/marketing/streamforge-launch
```

Expected: no matches, except acceptable mentions in explicit guardrails.

- [ ] **Step 2: Check git state**

Run: `git status --short`

Expected: the four campaign asset files and this plan are listed as changes.
