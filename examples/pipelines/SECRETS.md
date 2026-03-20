# Kubernetes Secrets for Secure Kafka Connections

The Streamforge operator supports referencing Kubernetes secrets for sensitive credentials, avoiding the need to store passwords and certificates directly in pipeline manifests.

## Why Use Secrets?

✅ **Security**: Credentials are stored encrypted in etcd
✅ **Separation**: Config and secrets managed independently
✅ **RBAC**: Fine-grained access control
✅ **Rotation**: Update credentials without changing pipelines
✅ **Audit**: Track who accessed secrets

## Supported Secret Types

### 1. SASL Authentication

Store SASL usernames and passwords in secrets:

```yaml
spec:
  source:
    brokers: "kafka:9093"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      sasl:
        mechanism: "SCRAM-SHA-512"
        # Reference secrets instead of inline values
        usernameSecret:
          name: kafka-credentials
          key: username
        passwordSecret:
          name: kafka-credentials
          key: password
```

**Create the secret:**
```bash
kubectl create secret generic kafka-credentials \
  --from-literal=username=myuser \
  --from-literal=password=mypassword \
  -n streamforge-system
```

### 2. TLS/SSL Certificates

Store CA certificates, client certificates, and private keys:

```yaml
spec:
  source:
    security:
      protocol: "SSL"
      ssl:
        caSecret:
          name: kafka-tls
          key: ca.crt
        certificateSecret:
          name: kafka-tls
          key: client.crt
        keySecret:
          name: kafka-tls
          key: client.key
        keyPasswordSecret:
          name: kafka-tls
          key: key.password
```

**Create the secret:**
```bash
kubectl create secret generic kafka-tls \
  --from-file=ca.crt=ca-cert.pem \
  --from-file=client.crt=client-cert.pem \
  --from-file=client.key=client-key.pem \
  --from-literal=key.password=keypass \
  -n streamforge-system
```

Or use TLS secret type:
```bash
kubectl create secret tls kafka-client-tls \
  --cert=client-cert.pem \
  --key=client-key.pem \
  -n streamforge-system
```

### 3. Kerberos Keytabs

Store Kerberos keytab files:

```yaml
spec:
  source:
    security:
      protocol: "SASL_SSL"
      sasl:
        mechanism: "GSSAPI"
        kerberosServiceName: "kafka"
        keytabSecret:
          name: kafka-kerberos
          key: krb5.keytab
```

**Create the secret:**
```bash
kubectl create secret generic kafka-kerberos \
  --from-file=krb5.keytab=/path/to/keytab \
  --from-file=krb5.conf=/etc/krb5.conf \
  -n streamforge-system
```

## How Secrets Are Mounted

The operator automatically mounts referenced secrets as volumes in pipeline pods. Secrets are organized by their role (source vs destinations) to avoid conflicts when multiple Kafka clusters use different credentials:

```
/etc/streamforge/secrets/
├── source/
│   ├── kafka-credentials/
│   │   ├── username
│   │   └── password
│   └── kafka-tls/
│       ├── ca.crt
│       ├── client.crt
│       └── client.key
├── destination-0/
│   ├── kafka-credentials/
│   │   ├── username
│   │   └── password
│   └── kafka-ca/
│       └── ca.crt
└── destination-1/
    └── kafka-tls/
        ├── ca.crt
        ├── client.crt
        └── client.key
```

**Path Structure:**
- Source cluster secrets: `/etc/streamforge/secrets/source/{secret-name}/{key}`
- Destination 0 secrets: `/etc/streamforge/secrets/destination-0/{secret-name}/{key}`
- Destination 1 secrets: `/etc/streamforge/secrets/destination-1/{secret-name}/{key}`

This organization ensures that:
✅ Different clusters can use different credentials without conflicts
✅ Secret paths clearly indicate which cluster they belong to
✅ Multiple destinations can have independent authentication

Secrets are mounted as **read-only** with **0400 permissions** (owner read-only).

## Complete Examples

### Example 1: SASL/SCRAM with TLS

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: sasl-secure-pipeline
  namespace: streamforge-system
spec:
  source:
    brokers: "kafka:9093"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      ssl:
        caSecret:
          name: kafka-ca
          key: ca.crt
      sasl:
        mechanism: "SCRAM-SHA-512"
        usernameSecret:
          name: kafka-sasl
          key: username
        passwordSecret:
          name: kafka-sasl
          key: password
  destinations:
    - brokers: "kafka:9093"
      topic: "output"
      # Inherit security config or specify different credentials
  replicas: 2
```

### Example 2: Mutual TLS (mTLS)

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: mtls-pipeline
  namespace: streamforge-system
spec:
  source:
    brokers: "kafka:9093"
    topic: "input"
    security:
      protocol: "SSL"
      ssl:
        caSecret:
          name: kafka-tls
          key: ca.crt
        certificateSecret:
          name: kafka-tls
          key: client.crt
        keySecret:
          name: kafka-tls
          key: client.key
  destinations:
    - brokers: "kafka:9093"
      topic: "output"
  replicas: 2
```

### Example 3: Different Credentials for Source and Destination

```yaml
apiVersion: streamforge.io/v1alpha1
kind: StreamforgePipeline
metadata:
  name: multi-cred-pipeline
  namespace: streamforge-system
spec:
  source:
    brokers: "source-kafka:9093"
    topic: "input"
    security:
      protocol: "SASL_SSL"
      ssl:
        caSecret:
          name: source-ca
          key: ca.crt
      sasl:
        mechanism: "PLAIN"
        usernameSecret:
          name: source-creds
          key: username
        passwordSecret:
          name: source-creds
          key: password
  destinations:
    - brokers: "dest-kafka:9093"
      topic: "output"
      security:
        protocol: "SASL_SSL"
        ssl:
          caSecret:
            name: dest-ca
            key: ca.crt
        sasl:
          mechanism: "SCRAM-SHA-256"
          usernameSecret:
            name: dest-creds
            key: username
          passwordSecret:
            name: dest-creds
            key: password
  replicas: 2
```

## Secret Management Best Practices

### 1. Use External Secret Management

Consider using external secret management solutions:

- **[External Secrets Operator](https://external-secrets.io/)**: Sync from AWS Secrets Manager, Azure Key Vault, HashiCorp Vault
- **[Sealed Secrets](https://github.com/bitnami-labs/sealed-secrets)**: Encrypt secrets in Git
- **[SOPS](https://github.com/mozilla/sops)**: Encrypt YAML files with KMS keys

Example with External Secrets Operator:
```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: kafka-credentials
  namespace: streamforge-system
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: kafka-credentials
  data:
    - secretKey: username
      remoteRef:
        key: prod/kafka/username
    - secretKey: password
      remoteRef:
        key: prod/kafka/password
```

### 2. RBAC Policies

Restrict secret access using Kubernetes RBAC:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: streamforge-secret-reader
  namespace: streamforge-system
rules:
  - apiGroups: [""]
    resources: ["secrets"]
    resourceNames: ["kafka-credentials", "kafka-tls"]
    verbs: ["get"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: streamforge-pipeline-secrets
  namespace: streamforge-system
subjects:
  - kind: ServiceAccount
    name: streamforge-pipeline
roleRef:
  kind: Role
  name: streamforge-secret-reader
  apiGroup: rbac.authorization.k8s.io
```

### 3. Secret Rotation

Rotate secrets without restarting pipelines:

```bash
# Update secret
kubectl create secret generic kafka-credentials \
  --from-literal=username=newuser \
  --from-literal=password=newpassword \
  --dry-run=client -o yaml | kubectl apply -f -

# Restart pipeline pods to pick up new secret
kubectl rollout restart deployment secure-sasl-pipeline \
  -n streamforge-system
```

### 4. Namespace Isolation

Keep secrets in the same namespace as pipelines:

```bash
# Production namespace
kubectl create namespace prod-pipelines
kubectl create secret generic kafka-prod-creds \
  --from-literal=username=prod-user \
  --from-literal=password=prod-pass \
  -n prod-pipelines

# Staging namespace
kubectl create namespace staging-pipelines
kubectl create secret generic kafka-staging-creds \
  --from-literal=username=staging-user \
  --from-literal=password=staging-pass \
  -n staging-pipelines
```

### 5. Audit Logging

Enable audit logging for secret access:

```yaml
# kube-apiserver audit policy
apiVersion: audit.k8s.io/v1
kind: Policy
rules:
  - level: RequestResponse
    verbs: ["get", "list", "watch"]
    resources:
      - group: ""
        resources: ["secrets"]
    namespaces: ["streamforge-system"]
```

## Troubleshooting

### Secret Not Found

**Error:** `MountVolume.SetUp failed: secrets "kafka-credentials" not found`

**Solution:**
```bash
# Check if secret exists
kubectl get secret kafka-credentials -n streamforge-system

# Create if missing
kubectl create secret generic kafka-credentials \
  --from-literal=username=user \
  --from-literal=password=pass \
  -n streamforge-system
```

### Wrong Key in Secret

**Error:** Pipeline pods crash with authentication failures

**Solution:**
```bash
# Verify secret keys
kubectl get secret kafka-credentials -n streamforge-system -o yaml

# Keys should match pipeline spec
data:
  username: dXNlcg==  # base64 encoded
  password: cGFzcw==  # base64 encoded
```

### Permission Denied

**Error:** `Unable to mount volumes: secret "kafka-credentials" is forbidden`

**Solution:**
```bash
# Check ServiceAccount has access
kubectl auth can-i get secret/kafka-credentials \
  --as=system:serviceaccount:streamforge-system:streamforge-pipeline \
  -n streamforge-system

# Add RBAC if needed (see RBAC section above)
```

## Inline Values vs Secrets

You can still use inline values for non-sensitive config:

```yaml
# ✅ OK: Non-sensitive configuration
spec:
  source:
    security:
      protocol: "SASL_SSL"
      ssl:
        caLocation: "/etc/ssl/certs/ca-certificates.crt"  # System CA bundle
      sasl:
        mechanism: "SCRAM-SHA-512"
        # ❌ BAD: Don't put credentials inline!
        # username: "myuser"
        # password: "mypassword"
        # ✅ GOOD: Use secrets for credentials
        usernameSecret:
          name: kafka-creds
          key: username
        passwordSecret:
          name: kafka-creds
          key: password
```

## See Also

- [secure-sasl-pipeline.yaml](./secure-sasl-pipeline.yaml) - Complete SASL example
- [secure-tls-pipeline.yaml](./secure-tls-pipeline.yaml) - Complete mTLS example
- [03-secure-transform.yaml](./03-secure-transform.yaml) - Security with transformations
