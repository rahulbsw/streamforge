# Pipeline Secret references

StreamForge pipeline manifests contain Kubernetes Secret references, never
credential values. Create each Secret from an approved secret store, then
reference its name and key in the CRD.

For example:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    caSecret:
      name: kafka-ca
      key: ca.crt
  sasl:
    mechanism: SCRAM-SHA-512
    usernameSecret:
      name: kafka-auth
      key: username
    passwordSecret:
      name: kafka-auth
      key: password
```

The operator mounts source references below
`/etc/streamforge/secrets/source` and the shared target references below
`/etc/streamforge/secrets/destination-0`. The generated pipeline ConfigMap
contains only those file paths.

All destinations in one `v1alpha1` pipeline share one target Kafka client and
must use identical broker and security settings. Use a separate pipeline
resource for a different target cluster or principal.

Inline `ssl.keyPassword`, `sasl.username`, and `sasl.password` values are
rejected. Secret names and keys are validated before a mount path is
constructed.

For Secret creation, rotation, file permissions, authentication mechanisms,
and verification, use the canonical
[Security guide](../../docs/SECURITY_CONFIGURATION.md). Complete working
manifests are:

- [SASL/SCRAM](secure-sasl-pipeline.yaml)
- [mutual TLS](secure-tls-pipeline.yaml)
- [secure transform](03-secure-transform.yaml)
