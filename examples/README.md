# Configuration Examples

This directory contains example configuration files for WAP MirrorMaker in both YAML and JSON formats.

## Quick Start

```bash
# Copy an example and customize
cp examples/config.example.yaml config.yaml
vim config.yaml

# Run with your config
CONFIG_FILE=config.yaml cargo run
```

## File Overview

### Simple Examples

| File | Format | Description |
|------|--------|-------------|
| **config.example.yaml** | YAML | Simple single-destination mirror (recommended) |
| **config.example.json** | JSON | Simple single-destination mirror (backward compatible) |

**Use for**: Basic mirroring without filters or transforms (no security).

**Example**:
```yaml
appid: wap-mirrormaker
bootstrap: kafka:9092
input: source-topic
output: destination-topic
threads: 4
```

---

### Security Examples

| File | Format | Description |
|------|--------|-------------|
| **config.security-ssl.yaml** | YAML | SSL/TLS encryption (one-way or mutual TLS) |
| **config.security-sasl-plain.yaml** | YAML | SASL/PLAIN authentication over SSL |
| **config.security-sasl-scram.yaml** | YAML | SASL/SCRAM-SHA-256/512 authentication |
| **config.security-kerberos.yaml** | YAML | Kerberos/GSSAPI authentication |

**Use for**: Secure connections to production Kafka clusters.

**Example (SSL/TLS)**:
```yaml
security:
  protocol: SSL
  ssl:
    ca_location: /path/to/ca-cert.pem
    certificate_location: /path/to/client-cert.pem  # For mTLS
    key_location: /path/to/client-key.pem          # For mTLS
```

**Example (SASL/SCRAM)**:
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

See [docs/SECURITY.md](../docs/SECURITY.md) for complete security documentation.

---

### Multi-Destination Examples

| File | Format | Description |
|------|--------|-------------|
| **config.multidest.yaml** | YAML | Multi-destination with filters/transforms |
| **config.multi-destination.example.json** | JSON | Multi-destination (backward compatible) |

**Use for**: Routing one input topic to multiple output topics with different filters.

**Example**:
```yaml
routing:
  destinations:
    - output: validated-users
      filter: "REGEX:/email,^[\\w]+@[\\w]+\\.\\w+$"

    - output: premium-orders
      filter: "AND:/total,>,500:/status,==,confirmed"
```

---

### Advanced Examples

| File | Format | Description |
|------|--------|-------------|
| **config.advanced.yaml** | YAML | **17 production examples** with all features |
| **config.advanced.example.json** | JSON | Advanced examples (backward compatible) |
| **config.advanced-filters.example.json** | JSON | Boolean logic examples |
| **config.filter-transform.example.json** | JSON | Filter and transform examples |

**Use for**: Learning all features, production configurations, complex routing.

**Features demonstrated**:
- Email validation with regex
- Boolean logic (AND/OR/NOT)
- Array operations (ARRAY_ALL, ARRAY_ANY, ARRAY_MAP)
- Arithmetic operations (ADD, SUB, MUL, DIV)
- Object construction
- Complex nested logic
- Pattern matching

---

## Format Comparison

### YAML (Recommended)

**Advantages:**
- ✅ Comments for documentation
- ✅ Multi-line strings
- ✅ Less punctuation
- ✅ 20-30% fewer lines
- ✅ Much more readable

**Example:**
```yaml
routing:
  destinations:
    # Email validation pipeline
    - output: validated-users
      description: Users with valid email format
      filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
      transform: |
        CONSTRUCT:email=/user/email:name=/user/name
```

### JSON (Backward Compatible)

**Advantages:**
- ✅ Programmatically generated
- ✅ Strict schema validation
- ✅ Widely supported

**Example:**
```json
{
  "routing": {
    "destinations": [{
      "output": "validated-users",
      "filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$",
      "transform": "CONSTRUCT:email=/user/email:name=/user/name"
    }]
  }
}
```

---

## Configuration Structure

### Basic Fields

```yaml
appid: unique-app-id              # Consumer group ID
bootstrap: kafka:9092              # Source Kafka brokers
target_broker: kafka:9092          # Target Kafka brokers (optional)
input: source-topic                # Input topic name
output: destination-topic          # Output topic (single destination)
offset: latest                     # earliest or latest
threads: 4                         # Processing threads
```

### Compression

```yaml
compression:
  compression_type: raw            # raw, none, or enveloped
  compression_algo: gzip           # gzip, snappy, zstd, lz4
```

### Consumer Properties

```yaml
consumer_properties:
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"
  max.poll.records: "500"
```

### Producer Properties

```yaml
producer_properties:
  batch.size: "65536"
  linger.ms: "10"
  compression.type: "gzip"
```

### Multi-Destination Routing

```yaml
routing:
  routing_type: content
  destinations:
    - output: topic-name
      description: Human-readable description
      filter: "filter-expression"
      transform: "transform-expression"
      partition: /field/path
```

---

## Filter Examples

### Simple Comparison
```yaml
filter: "/order/total,>,1000"
```

### Boolean Logic
```yaml
filter: "AND:/user/active,==,true:/user/tier,==,premium"
filter: "OR:/priority,==,high:/priority,==,urgent"
filter: "NOT:/test,==,true"
```

### Regular Expressions
```yaml
filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
filter: "REGEX:/status,^(active|pending)$"
```

### Array Operations
```yaml
filter: "ARRAY_ALL:/sessions,/active,==,true"
filter: "ARRAY_ANY:/tasks,/priority,==,high"
```

---

## Transform Examples

### Field Extraction
```yaml
transform: "/message/confId"
transform: "/user/profile/email"
```

### Object Construction
```yaml
transform: "CONSTRUCT:id=/user/id:email=/user/email:name=/user/name"
```

### Array Mapping
```yaml
transform: "ARRAY_MAP:/users,/id"
```

### Arithmetic
```yaml
transform: "ARITHMETIC:ADD,/price,/tax"
transform: "ARITHMETIC:MUL,/price,1.2"
transform: "ARITHMETIC:SUB,/total,/discount"
transform: "ARITHMETIC:DIV,/total,/count"
```

---

## Use Cases by Example

### Simple Mirroring
→ Use `config.example.yaml` or `config.example.json`

### Content-Based Routing
→ Use `config.multidest.yaml`

### Email Validation
→ See `config.advanced.yaml` - "validated-users" destination

### Price Calculations
→ See `config.advanced.yaml` - "sales-tax" or "bulk-discount" destinations

### User Session Filtering
→ See `config.advanced.yaml` - "active-sessions" destination

### Complex Business Logic
→ See `config.advanced.yaml` - "premium-or-bulk" destination

---

## Testing Your Configuration

### Syntax Validation

**YAML:**
```bash
# Check YAML syntax
yamllint examples/config.example.yaml

# Or with yq
yq eval examples/config.example.yaml
```

**JSON:**
```bash
# Check JSON syntax
jq . examples/config.example.json
```

### Dry Run

```bash
# Build and test (will fail at Kafka connection, which is expected)
CONFIG_FILE=examples/config.example.yaml cargo run
```

### With Local Kafka

```bash
# Start Kafka with docker-compose
docker-compose --profile kafka up -d

# Run with your config
CONFIG_FILE=examples/config.example.yaml cargo run
```

---

## Creating Your Own Configuration

### Step 1: Choose a Starting Point

```bash
# For simple mirroring
cp examples/config.example.yaml config.yaml

# For multi-destination
cp examples/config.multidest.yaml config.yaml

# For advanced features
cp examples/config.advanced.yaml config.yaml
```

### Step 2: Customize

```yaml
# Update with your Kafka details
appid: my-mirrormaker
bootstrap: my-kafka:9092
target_broker: my-target-kafka:9092
input: my-input-topic

# Adjust destinations
routing:
  destinations:
    - output: my-output-topic
      filter: "/myfield,==,myvalue"
```

### Step 3: Test

```bash
# Validate syntax
yamllint config.yaml

# Test configuration
CONFIG_FILE=config.yaml cargo run
```

---

## Common Patterns

### Pattern 1: Filter and Archive

```yaml
routing:
  destinations:
    # Filtered stream
    - output: active-users
      filter: "/user/active,==,true"
      transform: "/user"

    # Raw archive
    - output: all-users-archive
      transform: "/"
```

### Pattern 2: Split by Type

```yaml
routing:
  destinations:
    - output: user-events
      filter: "REGEX:/type,^user"

    - output: order-events
      filter: "REGEX:/type,^order"

    - output: system-events
      filter: "REGEX:/type,^system"
```

### Pattern 3: Validation Pipeline

```yaml
routing:
  destinations:
    # Valid records
    - output: validated
      filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
      transform: "/data"

    # Invalid records
    - output: validation-errors
      filter: "NOT:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
      transform: "CONSTRUCT:error=invalid_email:record=/"
```

---

## Environment-Specific Configs

### Development

```yaml
appid: mirrormaker-dev
bootstrap: localhost:9092
offset: earliest  # Start from beginning
threads: 2        # Low resource usage
```

### Staging

```yaml
appid: mirrormaker-staging
bootstrap: staging-kafka:9092
offset: latest
threads: 4
```

### Production

```yaml
appid: mirrormaker-prod
bootstrap: prod-kafka-1:9092,prod-kafka-2:9092,prod-kafka-3:9092
offset: latest
threads: 8

consumer_properties:
  fetch.min.bytes: "1048576"
  max.poll.records: "1000"

producer_properties:
  batch.size: "131072"
  compression.type: "snappy"
```

---

## Documentation

For more information:

- **Configuration Guide**: [../docs/YAML_CONFIGURATION.md](../docs/YAML_CONFIGURATION.md)
- **DSL Reference**: [../docs/ADVANCED_DSL_GUIDE.md](../docs/ADVANCED_DSL_GUIDE.md)
- **Use Cases**: [../docs/USAGE.md](../docs/USAGE.md)
- **Quick Reference**: [../docs/QUICK_REFERENCE.md](../docs/QUICK_REFERENCE.md)

---

## Tips

1. **Start Simple**: Begin with `config.example.yaml` and add complexity as needed
2. **Use YAML**: Much more readable for complex configurations
3. **Add Comments**: Document your business logic in YAML
4. **Test Filters**: Verify filters match expected messages
5. **Monitor Metrics**: Watch filtered vs completed message counts
6. **Version Control**: Keep configs in git

---

**Need help?** See [../docs/USAGE.md](../docs/USAGE.md) for comprehensive examples.
