---
title: Security
nav_order: 2
parent: Operations
---

# Security

StreamForge maps its top-level `security` configuration to librdkafka for both
the source consumer and destination producer. Secure the configuration file,
Kafka authorization, network path, container, and observability endpoint as one
system.

## Supported configuration

The runtime schema accepts:

- `PLAINTEXT`
- `SSL`
- `SASL_PLAINTEXT`
- `SASL_SSL`
- SASL mechanisms `PLAIN`, `SCRAM-SHA-256`, `SCRAM-SHA-512`, `GSSAPI`, and
  `OAUTHBEARER`

Actual availability also depends on the linked librdkafka build and broker
configuration. Validate the mechanism in the target environment before
production use.

Use `SSL` or `SASL_SSL` on untrusted networks. `PLAINTEXT` and
`SASL_PLAINTEXT` do not protect message data in transit.

## TLS

Broker verification:

```yaml
security:
  protocol: SSL
  ssl:
    ca_location: /run/streamforge/tls/ca.pem
    endpoint_identification_algorithm: https
```

Mutual TLS:

```yaml
security:
  protocol: SSL
  ssl:
    ca_location: /run/streamforge/tls/ca.pem
    certificate_location: /run/streamforge/tls/client.pem
    key_location: /run/streamforge/tls/client-key.pem
    endpoint_identification_algorithm: https
```

Mount CA, certificate, and private-key files read-only. Restrict the private key
to the StreamForge runtime identity. Do not disable hostname verification as a
production workaround.

## SASL over TLS

SCRAM example:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /run/streamforge/tls/ca.pem
    endpoint_identification_algorithm: https
  sasl:
    mechanism: SCRAM-SHA-512
    username: rendered-at-runtime
    password: rendered-at-runtime
```

PLAIN transmits credentials inside the TLS session and must not be used without
TLS. GSSAPI requires a compatible librdkafka build and Kerberos environment.
OAUTHBEARER token lifecycle must be tested for the exact client and broker; a
static token in a long-running file is not a rotation strategy.

## Secret injection

StreamForge does not interpolate `${ENVIRONMENT_VARIABLE}` placeholders in
configuration values. A secret manager or entrypoint must render a protected
configuration file before StreamForge starts.

Safe patterns include:

- mounting a complete secret-bearing configuration from a Kubernetes `Secret`;
- rendering into a memory-backed volume from an approved secret sidecar;
- mounting a protected host file into a container read-only;
- rotating the rendered file and performing a controlled restart.

Do not:

- commit credentials, tokens, private keys, or a rendered configuration;
- put a secret-bearing configuration in a Kubernetes `ConfigMap`;
- print the configuration in CI logs or diagnostics;
- pass passwords on a shell command line;
- use example or default credentials.

Ensure temporary rendered files are excluded from backups and removed according
to the platform secret-handling policy.

## Different source and destination credentials

The top-level `security` block is applied to both Kafka clients. When the source
and destination require different settings, explicit `consumer_properties` and
`producer_properties` can override the generated librdkafka properties:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /run/streamforge/tls/ca.pem
    endpoint_identification_algorithm: https

consumer_properties:
  sasl.mechanism: SCRAM-SHA-512
  sasl.username: rendered-source-user
  sasl.password: rendered-source-password

producer_properties:
  sasl.mechanism: SCRAM-SHA-512
  sasl.username: rendered-destination-user
  sasl.password: rendered-destination-password
```

These values are still secrets and require the same protected rendering process.
Validate the full configuration without exposing it in logs.

Do not copy Java callback-handler or JAAS properties into this Rust client.
Provider-specific authentication is supported only when the linked librdkafka
client and StreamForge configuration have been explicitly tested for that
provider.

## Kafka authorization

Grant only the resources used by a pipeline:

- source topic `READ` and metadata access;
- consumer group access for the configured `appid`;
- destination topic `WRITE` and metadata access;
- DLQ topic `WRITE` when enabled;
- any additional permissions required by the broker's authorization model.

Use a separate principal per environment and, where practical, per pipeline.
Avoid wildcard topic and consumer-group grants.

## Network controls

- Keep Kafka listeners on private subnets or cluster networks.
- Restrict StreamForge egress to Kafka, DNS, secret services, and required
  telemetry.
- Do not create public broker listeners for troubleshooting.
- Keep `/metrics` and `/health` private; they have no authentication or TLS.
- Prefer loopback port forwarding or a private monitoring network for
  diagnostics.

If temporary remote access is unavoidable, allow only the operator's verified
IP at the network boundary and remove the rule immediately after use.

## Containers and Kubernetes

- Run as a non-root identity.
- Drop Linux capabilities and disable privilege escalation.
- Use a read-only root filesystem with an explicit temporary filesystem.
- Mount configuration and key material read-only.
- Use `ClusterIP` services and restrictive `NetworkPolicy`.
- Avoid `NodePort`, public `LoadBalancer`, and internet-facing `Ingress`.
- Review service-account and operator RBAC from rendered manifests.

The current Kubernetes operator mounts referenced secrets but does not emit CR
security fields into its generated runtime configuration. Use a directly
managed Deployment for secured Kafka connections until that path is implemented
and verified. See [Kubernetes](KUBERNETES.md).

## Verification

Before production:

1. validate the rendered configuration without printing it;
2. confirm the process identity can read only the required files;
3. verify broker hostname validation and certificate chain;
4. verify source read, group, destination write, and DLQ permissions separately;
5. confirm an unauthorized topic access is denied;
6. test credential and certificate rotation;
7. confirm metrics and health are unreachable from outside the private network;
8. inspect logs and diagnostic bundles for secret leakage.

Useful certificate checks:

```bash
openssl x509 -in /run/streamforge/tls/ca.pem -noout -subject -issuer -dates
openssl verify \
  -CAfile /run/streamforge/tls/ca.pem \
  /run/streamforge/tls/client.pem
```

Continue with [Docker](DOCKER.md), [Kubernetes](KUBERNETES.md), and
[Deployment](DEPLOYMENT.md).
