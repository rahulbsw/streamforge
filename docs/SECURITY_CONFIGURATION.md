# Security Configuration Guide

Complete guide for securing Kafka connections with SSL/TLS encryption and SASL authentication.

## Table of Contents

- [Overview](#overview)
- [Security Protocols](#security-protocols)
- [SSL/TLS Encryption](#ssltls-encryption)
- [SASL Authentication](#sasl-authentication)
- [Cloud Provider Examples](#cloud-provider-examples)
- [Troubleshooting](#troubleshooting)
- [Best Practices](#best-practices)

---

## Overview

WAP MirrorMaker supports all standard Kafka security features:

| Feature | Support | Use Case |
|---------|---------|----------|
| **SSL/TLS** | ✅ Full | Encrypted connections |
| **Mutual TLS** | ✅ Full | Certificate-based authentication |
| **SASL/PLAIN** | ✅ Full | Username/password (simple) |
| **SASL/SCRAM-SHA-256** | ✅ Full | Username/password (secure) |
| **SASL/SCRAM-SHA-512** | ✅ Full | Username/password (more secure) |
| **SASL/GSSAPI** | ✅ Full | Kerberos authentication |
| **SASL/OAUTHBEARER** | ✅ Full | OAuth 2.0 token authentication |

---

## Security Protocols

Kafka supports four security protocols:

### 1. PLAINTEXT (Default)
No encryption, no authentication. **Not recommended for production.**

```yaml
# No security configuration needed
appid: mirrormaker
bootstrap: kafka:9092
```

### 2. SSL
Encryption only, optional certificate-based authentication (mutual TLS).

```yaml
security:
  protocol: SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
```

### 3. SASL_PLAINTEXT
Authentication without encryption. **Not recommended for production.**

```yaml
security:
  protocol: SASL_PLAINTEXT
  sasl:
    mechanism: PLAIN
    username: user
    password: pass
```

### 4. SASL_SSL (Recommended)
Both encryption (SSL) and authentication (SASL).

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
  sasl:
    mechanism: SCRAM-SHA-256
    username: user
    password: pass
```

---

## SSL/TLS Encryption

### Simple SSL (One-Way TLS)

Client verifies broker's certificate:

```yaml
security:
  protocol: SSL
  ssl:
    # CA certificate to verify broker
    ca_location: /path/to/ca-cert.pem

    # Verify broker hostname (recommended)
    endpoint_identification_algorithm: https
```

**Use Case:** Basic encryption for data in transit.

### Mutual TLS (mTLS)

Both client and broker verify each other:

```yaml
security:
  protocol: SSL
  ssl:
    # CA certificate to verify broker
    ca_location: /path/to/ca-cert.pem

    # Client certificate for authentication
    certificate_location: /path/to/client-cert.pem
    key_location: /path/to/client-key.pem
    key_password: optional-key-password

    # Verify broker hostname
    endpoint_identification_algorithm: https
```

**Use Case:** Certificate-based authentication, high security environments.

### Generating SSL Certificates

```bash
# Generate CA certificate
openssl req -new -x509 -keyout ca-key.pem -out ca-cert.pem -days 365

# Generate client key and certificate
openssl req -new -keyout client-key.pem -out client-cert-req.pem -days 365
openssl x509 -req -in client-cert-req.pem -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out client-cert.pem -days 365
```

---

## SASL Authentication

### SASL/PLAIN

Simple username/password authentication:

```yaml
security:
  protocol: SASL_SSL  # Always use SSL with PLAIN
  ssl:
    ca_location: /path/to/ca-cert.pem
  sasl:
    mechanism: PLAIN
    username: your-username
    password: your-password
```

**Pros:**
- ✅ Simple to configure
- ✅ Works with most Kafka brokers

**Cons:**
- ⚠️ Password transmitted in plain text (must use SSL!)
- ⚠️ Less secure than SCRAM

**Use Case:** Development, testing, Confluent Cloud.

### SASL/SCRAM-SHA-256

Secure Challenge-Response Authentication Mechanism:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
  sasl:
    mechanism: SCRAM-SHA-256
    username: your-username
    password: your-password
```

**Pros:**
- ✅ Password never transmitted over network
- ✅ Mutual authentication
- ✅ Replay attack protection

**Use Case:** Modern Kafka clusters, AWS MSK, production environments.

### SASL/SCRAM-SHA-512

More secure variant of SCRAM:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
  sasl:
    mechanism: SCRAM-SHA-512  # Changed from SHA-256
    username: your-username
    password: your-password
```

**Use Case:** High-security environments requiring stronger hashing.

### SASL/GSSAPI (Kerberos)

Enterprise authentication with Kerberos:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
  sasl:
    mechanism: GSSAPI
    kerberos_service_name: kafka
    kerberos_principal: client@EXAMPLE.COM
    kerberos_keytab: /path/to/client.keytab
```

**Prerequisites:**
1. Install Kerberos libraries:
   ```bash
   # Ubuntu/Debian
   apt-get install libkrb5-dev

   # RHEL/CentOS
   yum install krb5-devel
   ```

2. Configure `/etc/krb5.conf`:
   ```ini
   [libdefaults]
     default_realm = EXAMPLE.COM

   [realms]
     EXAMPLE.COM = {
       kdc = kdc.example.com
       admin_server = admin.example.com
     }
   ```

3. Test Kerberos:
   ```bash
   kinit -kt /path/to/client.keytab client@EXAMPLE.COM
   klist  # Verify ticket
   ```

**Use Case:** Enterprise Hadoop/Kafka clusters, legacy systems.

---

## Cloud Provider Examples

### Confluent Cloud

```yaml
appid: mirrormaker-confluent
bootstrap: pkc-xxxxx.us-east-1.aws.confluent.cloud:9092
input: source-topic
output: destination-topic

security:
  protocol: SASL_SSL
  sasl:
    mechanism: PLAIN
    username: <API_KEY>
    password: <API_SECRET>
```

**How to get credentials:**
1. Go to Confluent Cloud Console
2. Select your cluster
3. API Keys → Create Key
4. Copy API Key (username) and Secret (password)

### AWS MSK (Managed Streaming for Kafka)

#### Option 1: SASL/SCRAM

```yaml
appid: mirrormaker-msk
bootstrap: b-1.msk-cluster.xxxxx.kafka.us-east-1.amazonaws.com:9096
input: source-topic
output: destination-topic

security:
  protocol: SASL_SSL
  sasl:
    mechanism: SCRAM-SHA-512
    username: <SECRET_USERNAME>
    password: <SECRET_PASSWORD>
```

**How to set up:**
1. Create secret in AWS Secrets Manager
2. Associate secret with MSK cluster
3. Use secret values as username/password

#### Option 2: IAM Authentication

```yaml
appid: mirrormaker-msk-iam
bootstrap: b-1.msk-cluster.xxxxx.kafka.us-east-1.amazonaws.com:9098
input: source-topic
output: destination-topic

# For IAM auth, use custom properties
consumer_properties:
  security.protocol: SASL_SSL
  sasl.mechanism: AWS_MSK_IAM
  sasl.jaas.config: software.amazon.msk.auth.iam.IAMLoginModule required;
  sasl.client.callback.handler.class: software.amazon.msk.auth.iam.IAMClientCallbackHandler

producer_properties:
  security.protocol: SASL_SSL
  sasl.mechanism: AWS_MSK_IAM
  sasl.jaas.config: software.amazon.msk.auth.iam.IAMLoginModule required;
  sasl.client.callback.handler.class: software.amazon.msk.auth.iam.IAMClientCallbackHandler
```

### Azure Event Hubs (Kafka Protocol)

```yaml
appid: mirrormaker-eventhubs
bootstrap: <NAMESPACE>.servicebus.windows.net:9093
input: source-topic
output: destination-topic

security:
  protocol: SASL_SSL
  sasl:
    mechanism: PLAIN
    username: $ConnectionString
    password: Endpoint=sb://<NAMESPACE>.servicebus.windows.net/;SharedAccessKeyName=<KEY_NAME>;SharedAccessKey=<KEY>
```

---

## Troubleshooting

### SSL Certificate Issues

**Problem:** `SSL handshake failed`

**Solutions:**
1. Verify CA certificate path:
   ```bash
   openssl verify -CAfile ca-cert.pem broker-cert.pem
   ```

2. Check certificate expiration:
   ```bash
   openssl x509 -in ca-cert.pem -noout -dates
   ```

3. Disable hostname verification (testing only):
   ```yaml
   ssl:
     endpoint_identification_algorithm: ""
   ```

### SASL Authentication Issues

**Problem:** `Authentication failed`

**Solutions:**
1. Verify credentials are correct
2. Check SASL mechanism matches broker configuration
3. For SCRAM, ensure user exists on broker:
   ```bash
   kafka-configs.sh --bootstrap-server kafka:9092 \
     --describe --entity-type users
   ```

### Kerberos Issues

**Problem:** `GSSAPI authentication failed`

**Solutions:**
1. Verify Kerberos ticket:
   ```bash
   klist -e
   ```

2. Check keytab:
   ```bash
   klist -kt client.keytab
   ```

3. Test kinit:
   ```bash
   kinit -kt client.keytab client@EXAMPLE.COM
   ```

4. Check service name matches broker configuration

### Connection Timeout

**Problem:** Connection times out

**Solutions:**
1. Verify broker hostname/port
2. Check firewall rules allow port (9093, 9094, etc.)
3. Verify security group rules (cloud providers)
4. Test with openssl:
   ```bash
   openssl s_client -connect kafka:9093
   ```

---

## Best Practices

### 1. Always Use Encryption

✅ **Do:**
```yaml
security:
  protocol: SASL_SSL  # SSL encryption enabled
```

❌ **Don't:**
```yaml
security:
  protocol: SASL_PLAINTEXT  # No encryption!
```

### 2. Secure Credential Storage

✅ **Do:** Use environment variables or secret management
```bash
export KAFKA_USERNAME="myuser"
export KAFKA_PASSWORD="mypass"
```

❌ **Don't:** Store passwords in configuration files
```yaml
sasl:
  password: "plaintext-password-in-git"  # Bad!
```

### 3. Use Strong Authentication

**Security Ranking:**
1. 🥇 Mutual TLS (mTLS) - Best
2. 🥈 SASL/SCRAM-SHA-512 - Very Good
3. 🥉 SASL/SCRAM-SHA-256 - Good
4. ⚠️ SASL/PLAIN - Acceptable with SSL
5. ❌ PLAINTEXT - Never use in production

### 4. Certificate Management

- ✅ Rotate certificates regularly (90 days recommended)
- ✅ Use separate certificates for each service
- ✅ Monitor certificate expiration
- ✅ Keep private keys secure (chmod 600)

### 5. Network Security

- ✅ Use VPN or private networks for Kafka traffic
- ✅ Restrict broker access with firewall rules
- ✅ Use separate security groups for Kafka
- ✅ Enable VPC peering for cross-account access (AWS)

### 6. Monitoring

Monitor these metrics:
- Authentication failures
- SSL handshake errors
- Certificate expiration warnings
- Connection timeouts

### 7. Testing

Test security configuration before production:

```bash
# Test SSL connection
openssl s_client -connect kafka:9093 -CAfile ca-cert.pem

# Test with kafkacat
kafkacat -b kafka:9093 -L \
  -X security.protocol=SASL_SSL \
  -X sasl.mechanism=SCRAM-SHA-256 \
  -X sasl.username=user \
  -X sasl.password=pass

# Test with MirrorMaker
CONFIG_FILE=examples/config.security-sasl-scram.yaml cargo run
```

---

## Configuration Examples

All security examples are in the `examples/` folder:

- **[config.security-ssl.yaml](../examples/config.security-ssl.yaml)** - SSL/TLS encryption
- **[config.security-sasl-plain.yaml](../examples/config.security-sasl-plain.yaml)** - SASL/PLAIN authentication
- **[config.security-sasl-scram.yaml](../examples/config.security-sasl-scram.yaml)** - SASL/SCRAM authentication
- **[config.security-kerberos.yaml](../examples/config.security-kerberos.yaml)** - Kerberos authentication

---

## References

### Official Documentation
- [Kafka Security](https://kafka.apache.org/documentation/#security)
- [librdkafka Configuration](https://github.com/edenhill/librdkafka/blob/master/CONFIGURATION.md)
- [Confluent Security](https://docs.confluent.io/platform/current/security/index.html)

### Cloud Provider Guides
- [AWS MSK Security](https://docs.aws.amazon.com/msk/latest/developerguide/security.html)
- [Azure Event Hubs Kafka](https://docs.microsoft.com/en-us/azure/event-hubs/event-hubs-for-kafka-ecosystem-overview)
- [Confluent Cloud](https://docs.confluent.io/cloud/current/security/index.html)

---

## Quick Reference

### Security Configuration Template

```yaml
security:
  # Protocol: PLAINTEXT | SSL | SASL_PLAINTEXT | SASL_SSL
  protocol: SASL_SSL

  # SSL Configuration (for SSL or SASL_SSL)
  ssl:
    ca_location: /path/to/ca-cert.pem
    certificate_location: /path/to/client-cert.pem  # Optional (mTLS)
    key_location: /path/to/client-key.pem          # Optional (mTLS)
    key_password: key-password                      # Optional
    endpoint_identification_algorithm: https        # Optional

  # SASL Configuration (for SASL_PLAINTEXT or SASL_SSL)
  sasl:
    # Mechanism: PLAIN | SCRAM-SHA-256 | SCRAM-SHA-512 | GSSAPI | OAUTHBEARER
    mechanism: SCRAM-SHA-256

    # For PLAIN/SCRAM
    username: your-username
    password: your-password

    # For GSSAPI (Kerberos)
    kerberos_service_name: kafka
    kerberos_principal: client@REALM
    kerberos_keytab: /path/to/keytab

    # For OAUTHBEARER
    oauthbearer_token: your-token
```

---

**Need Help?** See [TROUBLESHOOTING.md](TROUBLESHOOTING.md) or open an issue on GitHub.
