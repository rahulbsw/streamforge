---
title: Podman
nav_order: 1
parent: Deployment
---

# Run StreamForge with Podman

The repository contains two container builds:

- `Dockerfile` builds the default Chainguard-based image.
- `Dockerfile.static` builds an x86-64 musl binary on a Chainguard static
  runtime.

Build and scan the exact image revision that you plan to deploy. Do not infer
current vulnerability status from a base-image brand or an unpinned `latest`
tag.

## Build

```bash
podman build --pull --tag streamforge:local .
```

For the static x86-64 image:

```bash
podman build --pull \
  --file Dockerfile.static \
  --tag streamforge:static-local .
```

For a reproducible release, replace moving base-image tags with approved
digests in your release process and record the resulting StreamForge image
digest.

## Prepare a configuration

Build the local validation binary:

```bash
cp examples/configs/config.example.yaml streamforge.yaml
cargo build --release --locked --bin streamforge-validate
target/release/streamforge-validate streamforge.yaml --fail-on-warnings
```

Do not commit credentials in `streamforge.yaml`. If the file contains secrets,
render it from an approved secret store into a protected runtime path.

## Run without public exposure

```bash
podman run --detach \
  --name streamforge \
  --restart unless-stopped \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --mount type=bind,src="$(pwd)/streamforge.yaml",dst=/run/streamforge/config.yaml,readonly \
  --env CONFIG_FILE=/run/streamforge/config.yaml \
  --env RUST_LOG=info \
  --publish 127.0.0.1:9090:9090 \
  streamforge:local
```

Binding the published metrics port to `127.0.0.1` prevents remote network
access. Omit `--publish` when Prometheus shares a private Podman network with
StreamForge.

The metrics server has no authentication or TLS. Never publish it on
`0.0.0.0` on an internet-reachable host.

## Verify

```bash
podman logs --tail 200 streamforge
curl --fail http://127.0.0.1:9090/health
curl --fail http://127.0.0.1:9090/metrics
```

`/health` proves that the HTTP process responds; it does not prove Kafka source
or destination health. Produce a controlled source record and verify the
destination independently before accepting a deployment.

## Private container network

Create a dedicated network when StreamForge and a private Kafka endpoint are
containerized on the same host:

```bash
podman network create streamforge-private
podman run --detach \
  --name streamforge \
  --network streamforge-private \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid \
  --cap-drop ALL \
  --security-opt no-new-privileges:true \
  --mount type=bind,src="$(pwd)/streamforge.yaml",dst=/run/streamforge/config.yaml,readonly \
  --env CONFIG_FILE=/run/streamforge/config.yaml \
  streamforge:local
```

Attach only the required private Kafka and monitoring services to that network.
Do not use host networking as a generic connectivity fix.

## Resource controls

Set CPU and memory limits from a representative workload test:

```bash
podman update \
  --cpus 2 \
  --memory 1g \
  --memory-swap 1g \
  streamforge
```

The values above demonstrate Podman syntax, not production sizing. Observe
consumer lag, broker-acknowledged deliveries, CPU throttling, memory, and
restarts before and after applying limits.

Memory use is affected by payload size, application batch size, processing
parallelism, worker queue capacity, destination fan-out, and queued producer
depth.

## Logs and shutdown

```bash
podman logs --follow streamforge
podman stop --time 30 streamforge
```

Keep application logs on the container output stream. Avoid mounting a writable
host log directory unless retention, rotation, and permissions are managed by
the platform.

After shutdown, verify the last committed source offsets and destination
records according to the selected
[delivery profile](DELIVERY_GUARANTEES.md).

## Image publishing checklist

- Build from a reviewed source revision and locked dependencies.
- Use an immutable registry tag or digest.
- Generate an SBOM and retain it with the release.
- Scan the final image, including the current base layers.
- Sign the image according to the registry policy.
- Run as a non-root identity and verify it in the built image.
- Keep the root filesystem read-only and drop Linux capabilities.
- Mount configuration and certificate material read-only.
- Bind metrics only to loopback or a private container network.
- Test on every published CPU architecture.

Continue with [Security](SECURITY_CONFIGURATION.md) and
[Operations](OPERATIONS.md).
