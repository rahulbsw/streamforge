---
title: Security
nav_order: 2
parent: Operations
---

# Security

StreamForge configures independent Kafka clients for its source consumer and
target producer. Secure credentials, Kafka authorization, network paths,
containers, logs, metrics, and release artifacts as one system.

## Supported protocols

The runtime accepts:

- `PLAINTEXT`
- `SSL`
- `SASL_PLAINTEXT`
- `SASL_SSL`
- SASL `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512`, `GSSAPI`, and
  `OAUTHBEARER`

Availability also depends on the linked librdkafka build and broker. Use `SSL`
or `SASL_SSL` on untrusted networks.

## Direct runtime configuration

`security` configures the source. `target_security` configures the destination.
For v1.x compatibility, omitting `target_security` applies `security` to both.

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /run/streamforge/source/ca.pem
    endpoint_identification_algorithm: https
  sasl:
    mechanism: SCRAM-SHA-512
    username_file: /run/streamforge/source/username
    password_file: /run/streamforge/source/password

target_security:
  protocol: SASL_SSL
  ssl:
    ca_location: /run/streamforge/target/ca.pem
    endpoint_identification_algorithm: https
  sasl:
    mechanism: SCRAM-SHA-512
    username_file: /run/streamforge/target/username
    password_file: /run/streamforge/target/password
```

File-backed username, password, and private-key password values are read when
Kafka clients are created. One trailing line ending is removed; empty or
unreadable files fail with a field-specific configuration error that does not
include the credential value.

Inline runtime fields remain accepted for v1.x compatibility, but protected
file injection is preferred:

- `ssl.key_password_file`
- `sasl.username_file`
- `sasl.password_file`

Mount all CA, certificate, key, keytab, and credential files read-only. Do not
disable hostname verification as a production workaround.

## Kubernetes Secret references

`StreamforgePipeline` resources must use Secret references for credential
values. Inline `ssl.keyPassword`, `sasl.username`, and `sasl.password` values
are rejected by the shared validator.

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: secure-orders
spec:
  source:
    brokers: source-kafka:9093
    topic: orders
    security:
      protocol: SASL_SSL
      ssl:
        caSecret:
          name: source-kafka-ca
          key: ca.crt
      sasl:
        mechanism: SCRAM-SHA-512
        usernameSecret:
          name: source-kafka-auth
          key: username
        passwordSecret:
          name: source-kafka-auth
          key: password
  destinations:
    - brokers: target-kafka:9093
      topic: orders-copy
      security:
        protocol: SSL
        ssl:
          caSecret:
            name: target-kafka-tls
            key: ca.crt
          certificateSecret:
            name: target-kafka-tls
            key: tls.crt
          keySecret:
            name: target-kafka-tls
            key: tls.key
```

The operator mounts referenced Secrets with mode `0440`. The generated
ConfigMap contains only paths below `/etc/streamforge/secrets`, never credential
values. Pipeline pods run as UID/GID `65532`, do not mount a service-account
token by default, drop Linux capabilities, and use a read-only root filesystem.

All destinations in one `v1alpha1` pipeline share one target client, so their
broker and security settings must be identical. Use separate pipeline resources
for different target clusters or credentials.

Supported Secret references are:

- `ssl.caSecret`
- `ssl.certificateSecret`
- `ssl.keySecret`
- `ssl.keyPasswordSecret`
- `sasl.usernameSecret`
- `sasl.passwordSecret`
- `sasl.keytabSecret`

Secret names and keys are validated before paths are constructed. Unsafe path
components are rejected.

## Secret handling

StreamForge does not interpolate `${ENVIRONMENT_VARIABLE}` placeholders in
configuration values. Use an approved secret store to mount protected files or
Kubernetes Secrets.

Do not:

- commit credentials, tokens, private keys, `.env` files, or rendered secrets;
- put credential values in a Kubernetes custom resource or ConfigMap;
- print configuration or Secret content in CI, logs, diagnostics, or handoffs;
- pass passwords on a shell command line;
- retain default or example credentials in production.

Rotation requires updating the protected file or Secret and performing a
controlled pipeline restart so Kafka clients reopen it.

## Kafka authorization

Grant only the resources used by the pipeline:

- source topic read and metadata access;
- consumer-group access for the configured `appid`;
- destination topic write and metadata access;
- DLQ topic write when enabled;
- provider-specific metadata permissions that have been verified as necessary.

Prefer a separate principal per environment and pipeline. Avoid wildcard topic
or group grants.

## Network and observability controls

- Keep Kafka listeners on private networks.
- Restrict pipeline egress to Kafka, DNS, required secret services, and
  telemetry.
- Keep `/metrics`, `/health`, and `/ready` private; they do not provide
  authentication or TLS.
- Use `ClusterIP`, restrictive `NetworkPolicy`, or loopback port forwarding.
- Do not expose troubleshooting endpoints with public `NodePort`,
  `LoadBalancer`, or unauthenticated ingress.
- Structured logs redact payloads, headers, and credentials by default; verify
  this with environment-specific tests before production.

## Release and container controls

- Pin exact StreamForge component versions and base-image digests.
- Verify checksums, SBOMs, provenance, signatures, and vulnerability reports.
- Run containers as non-root and drop all capabilities.
- Keep root filesystems read-only except for explicit temporary volumes.
- Review rendered service accounts and operator RBAC.

## Verification

Before production:

1. validate configuration without printing secret values;
2. confirm the process identity can read only the required mounted files;
3. verify broker hostname and certificate-chain validation;
4. verify source read, group, destination write, and DLQ permissions
   independently;
5. confirm unauthorized topic access is denied;
6. test credential and certificate rotation;
7. verify health and metrics are unreachable outside the private network;
8. inspect logs, Kubernetes events, and diagnostics for secret leakage;
9. test restart and rebalance behavior while credentials are valid and invalid.

Continue with [Kubernetes](KUBERNETES.md), [Docker](DOCKER.md), and
[Deployment](DEPLOYMENT.md).
