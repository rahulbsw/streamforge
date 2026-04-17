# Usage Guide

Complete guide covering various use cases for StreamForge.

## Table of Contents

- [Basic Usage](#basic-usage)
- [Use Cases](#use-cases)
  - [Simple Cross-Cluster Mirroring](#use-case-1-simple-cross-cluster-mirroring)
  - [Content-Based Routing](#use-case-2-content-based-routing)
  - [Data Validation Pipeline](#use-case-3-data-validation-pipeline)
  - [Multi-Environment Deployment](#use-case-4-multi-environment-deployment)
  - [Event Streaming Platform](#use-case-5-event-streaming-platform)
  - [Data Lake Ingestion](#use-case-6-data-lake-ingestion)
  - [Real-time Analytics](#use-case-7-real-time-analytics)
  - [Microservices Integration](#use-case-8-microservices-integration)
- [Configuration Patterns](#configuration-patterns)
- [Troubleshooting](#troubleshooting)

## Basic Usage

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd streamforge

# Build the application
cargo build --release

# Binary location
./target/release/streamforge
```

### Running

```bash
# Using config file
CONFIG_FILE=config.yaml ./target/release/streamforge

# With logging
RUST_LOG=info CONFIG_FILE=config.yaml ./target/release/streamforge

# With debug logging
RUST_LOG=debug CONFIG_FILE=config.yaml ./target/release/streamforge
```

### Docker

```bash
# Build image
docker build -t streamforge:latest .

# Run with config
docker run -d \
  --name streamforge \
  -v $(pwd)/config.yaml:/app/config/config.yaml:ro \
  -e RUST_LOG=info \
  streamforge:latest
```

## Use Cases

### Use Case 1: Simple Cross-Cluster Mirroring

**Scenario**: Mirror all messages from a topic in Cluster A to Cluster B without modification.

**Configuration:**

```yaml
appid: simple-mirror
bootstrap: cluster-a:9092
target_broker: cluster-b:9092
input: source-topic
output: destination-topic
offset: latest
threads: 4

compression:
  compression_type: raw
  compression_algo: gzip
```

**Use When:**
- Disaster recovery (DR) setup
- Data center replication
- Cluster migration
- Backup/archive purposes

**Performance Tuning:**
```yaml
threads: 8

consumer_properties:
  fetch.min.bytes: "1048576"
  fetch.wait.max.ms: "500"

producer_properties:
  batch.size: "65536"
  linger.ms: "10"
  compression.type: "gzip"
```

### Use Case 2: Content-Based Routing

**Scenario**: Route messages to different topics based on content (e.g., route by event type, user tier, or region).

**Configuration:**

```yaml
appid: content-router
bootstrap: kafka:9092
target_broker: kafka:9092
input: events

routing:
  destinations:
    # User events
    - name: user-events
      output: users
      filter: 'msg["eventType"].starts_with("user.")'
      transform: 'msg["payload"]'
      partition: '/userId'
    
    # Order events with structured output
    - name: order-events
      output: orders
      filter: 'msg["eventType"].starts_with("order.")'
      transform: |
        #{
          orderId: msg["orderId"],
          amount: msg["amount"],
          status: msg["status"]
        }
      partition: '/orderId'
    
    # Payment events
    - name: payment-events
      output: payments
      filter: 'msg["eventType"].starts_with("payment.")'
      transform: 'msg["payload"]'
      partition: '/paymentId'
    
    # Audit trail (all events)
    - name: audit-all
      output: audit-log
      transform: |
        #{
          eventType: msg["eventType"],
          timestamp: msg["timestamp"],
          userId: msg["userId"]
        }
```

**Use When:**
- Event-driven architectures
- Domain-specific topic routing
- Service-to-service communication
- Multi-tenant applications

### Use Case 3: Data Validation Pipeline

**Scenario**: Validate incoming data and route to valid/invalid topics for processing or DLQ.

**Configuration:**

```yaml
appid: validator
bootstrap: kafka:9092
input: raw-data

routing:
  destinations:
    # Valid emails with all required fields
    - name: valid-emails
      output: validated-users
      filter:
        - 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$")'
        - 'not_null(msg["name"]) && msg["name"] != ""'
        - 'msg["age"] > 0'
      transform: |
        #{
          id: msg["id"],
          email: msg["email"].to_lower(),
          name: msg["name"],
          age: msg["age"]
        }
      partition: '/id'
    
    # Invalid email format
    - name: invalid-email-format
      output: validation-errors
      filter: '!msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$")'
      transform: |
        #{
          error: "email_format_invalid",
          record: msg
        }
      partition: '/id'
    
    # Missing required fields
    - name: missing-required-fields
      output: validation-errors
      filter: |
        is_null_or_empty(msg["name"]) ||
        is_null_or_empty(msg["email"]) ||
        msg["age"] <= 0
      transform: |
        #{
          error: "missing_required_fields",
          record: msg
        }
      partition: '/id'
```

**Use When:**
- Data quality enforcement
- ETL pipelines
- Input sanitization
- Compliance checking

### Use Case 4: Multi-Environment Deployment

**Scenario**: Mirror production data to staging/testing environments with data masking.

**Configuration:**

```yaml
appid: prod-to-staging
bootstrap: prod-kafka:9092
target_broker: staging-kafka:9092
input: production-events

routing:
  destinations:
    # Staging with PII masked
    - name: staging-safe-data
      output: staging-events
      filter: '!msg["sensitive"]'
      transform: |
        #{
          id: msg["id"],
          type: msg["type"],
          timestamp: msg["timestamp"],
          # Mask PII
          emailHash: hash_sha256(msg["email"].to_lower()),
          phoneHash: hash_sha256(msg["phone"]),
          # Non-sensitive data preserved
          data: msg["nonSensitiveData"]
        }
      partition: '/id'
    
    # Test sample (small subset)
    - name: test-sample
      output: test-events
      filter: |
        !msg["sensitive"] &&
        msg["testFlag"] == true &&
        msg["id"] % 100 == 0
      transform: 'msg'
      partition: '/id'
```

**Use When:**
- Testing with production-like data
- Development environment setup
- QA validation
- Performance testing

### Use Case 5: Event Streaming Platform

**Scenario**: Build a central event bus that distributes events to multiple downstream consumers.

**Configuration:**

```yaml
appid: event-bus
bootstrap: central-kafka:9092
input: event-stream

routing:
  destinations:
    # Analytics events
    - name: analytics
      output: analytics-events
      filter: 'msg["tags"].any(|t| t == "analytics")'
      transform: |
        #{
          eventId: msg["eventId"],
          type: msg["type"],
          userId: msg["userId"],
          timestamp: msg["timestamp"],
          metrics: msg["metrics"]
        }
    
    # High priority notifications
    - name: notifications
      output: notification-queue
      filter: 'msg["priority"] == "high" || msg["priority"] == "urgent"'
      transform: |
        #{
          userId: msg["userId"],
          message: msg["message"],
          type: msg["notificationType"]
        }
    
    # Billing events
    - name: billing
      output: billing-events
      filter: 'msg["type"].matches("^(usage|subscription|payment)")'
      transform: |
        #{
          userId: msg["userId"],
          amount: msg["amount"],
          type: msg["type"],
          timestamp: msg["timestamp"]
        }
    
    # Audit trail for state changes
    - name: audit
      output: audit-trail
      filter: 'msg["action"].matches("^(create|update|delete)")'
      transform: 'msg'
    
    # ML feature extraction
    - name: ml-features
      output: ml-training
      filter: 'msg["mlRelevant"] == true'
      transform: 'msg["features"].map(|f| f["value"])'
```

**Use When:**
- Event-driven microservices
- Real-time data distribution
- CQRS pattern implementation
- Event sourcing

### Use Case 6: Data Lake Ingestion

**Scenario**: Ingest data into data lake while maintaining real-time processing streams.

**Configuration:**

```yaml
appid: data-lake-ingestion
bootstrap: kafka:9092
input: application-events

routing:
  destinations:
    # Raw archive (everything)
    - name: raw-archive
      output: datalake-raw
      transform: 'msg'
    
    # Processed metrics
    - name: processed-metrics
      output: datalake-metrics
      filter: 'msg["metrics"] != () && msg["metrics"].len() > 0'
      transform: |
        #{
          timestamp: msg["timestamp"],
          source: msg["source"],
          metrics: msg["metrics"]
        }
    
    # Real-time high priority
    - name: realtime-high-priority
      output: realtime-processing
      filter: |
        msg["priority"] == "high" &&
        msg["processingTime"] < 1000
      transform: |
        #{
          id: msg["id"],
          priority: msg["priority"],
          data: msg["data"]
        }
    
    # Anomaly detection
    - name: anomaly-detection
      output: anomaly-queue
      filter: |
        msg["errorRate"] > 0.1 ||
        msg["responseTime"] > 5000 ||
        msg["statusCode"] >= 500
      transform: |
        #{
          source: msg["source"],
          metric: msg["metricName"],
          value: msg["metricValue"],
          threshold: msg["threshold"]
        }
```

**Use When:**
- Big data analytics
- Long-term storage
- Historical analysis
- Compliance/retention

### Use Case 7: Real-time Analytics

**Scenario**: Calculate metrics and route aggregated data for real-time dashboards.

**Configuration:**

```yaml
appid: realtime-analytics
bootstrap: kafka:9092
input: raw-metrics

routing:
  destinations:
    # Revenue calculation
    - name: revenue-total
      output: revenue-metrics
      filter: 'msg["order"]["status"] == "completed"'
      transform: |
        msg["order"]["amount"] + msg["order"]["tax"]
      partition: '/order/customerId'
    
    # Conversion rate
    - name: conversion-rate
      output: conversion-metrics
      filter: 'msg["visits"] > 0 && msg["conversions"] > 0'
      transform: |
        msg["conversions"] / msg["visits"]
      partition: '/campaignId'
    
    # User engagement
    - name: user-engagement
      output: engagement-metrics
      filter: 'msg["sessions"].any(|s| s["duration"] > 300)'
      transform: |
        #{
          userId: msg["userId"],
          sessionDurations: msg["sessions"].map(|s| s["duration"]),
          totalTime: msg["sessions"].map(|s| s["duration"]).sum()
        }
      partition: '/userId'
    
    # Error rates
    - name: error-rates
      output: error-metrics
      filter: 'msg["requests"] > 0 && msg["errors"] > 0'
      transform: |
        #{
          serviceId: msg["serviceId"],
          errorRate: msg["errors"] / msg["requests"],
          errors: msg["errors"],
          requests: msg["requests"]
        }
      partition: '/serviceId'
```

**Use When:**
- Real-time dashboards
- KPI monitoring
- Business intelligence
- Alerting systems

### Use Case 8: Microservices Integration

**Scenario**: Connect multiple microservices through event-driven communication.

**Configuration:**

```yaml
appid: microservices-hub
bootstrap: kafka:9092
input: service-events

routing:
  destinations:
    # User service events
    - name: user-service
      output: user-service-events
      filter: 'msg["aggregate"].starts_with("User")'
      transform: |
        #{
          aggregateId: msg["aggregateId"],
          eventType: msg["eventType"],
          payload: msg["payload"]
        }
      partition: '/aggregateId'
    
    # Order service events
    - name: order-service
      output: order-service-events
      filter: 'msg["aggregate"].starts_with("Order")'
      transform: |
        #{
          aggregateId: msg["aggregateId"],
          eventType: msg["eventType"],
          payload: msg["payload"]
        }
      partition: '/aggregateId'
    
    # Inventory service events
    - name: inventory-service
      output: inventory-service-events
      filter: 'msg["aggregate"].matches("^(Inventory|Product)")'
      transform: |
        #{
          aggregateId: msg["aggregateId"],
          eventType: msg["eventType"],
          payload: msg["payload"]
        }
      partition: '/aggregateId'
    
    # Notification service (based on tags)
    - name: notification-service
      output: notification-events
      filter: 'msg["tags"].any(|t| t == "notify")'
      transform: |
        #{
          userId: msg["userId"],
          message: msg["message"],
          channels: msg["channels"].map(|c| c["type"])
        }
    
    # Saga coordinator
    - name: cross-service-saga
      output: saga-coordinator
      filter: 'not_null(msg["sagaId"]) && msg["sagaId"] != ""'
      transform: |
        #{
          sagaId: msg["sagaId"],
          step: msg["step"],
          status: msg["status"],
          data: msg["data"]
        }
```

**Use When:**
- Event-driven microservices
- Saga pattern
- Domain events
- Service choreography

## Configuration Patterns

### Pattern 1: Broadcast with Transformation

Send the same message to multiple topics with different transformations:

```yaml
routing:
  destinations:
    # Full archive
    - name: full-archive
      output: archive
      transform: 'msg'
    
    # Minimal log
    - name: minimal-log
      output: logs
      transform: |
        #{
          id: msg["id"],
          timestamp: msg["timestamp"],
          level: msg["level"]
        }
    
    # Errors only
    - name: error-only
      output: errors
      filter: 'msg["level"].matches("^(ERROR|FATAL)")'
      transform: |
        #{
          id: msg["id"],
          error: msg["error"],
          stackTrace: msg["stackTrace"]
        }
```

### Pattern 2: Conditional Routing

Route based on complex conditions:

```yaml
routing:
  destinations:
    # Premium fast lane
    - name: premium-fast-lane
      output: premium-queue
      filter: |
        msg["user"]["tier"] == "premium" &&
        msg["priority"] == "high"
      transform: 'msg'
    
    # Standard processing
    - name: standard-processing
      output: standard-queue
      filter: |
        !(msg["user"]["tier"] == "premium" && msg["priority"] == "high")
      transform: 'msg'
```

### Pattern 3: Data Enrichment

Add calculated fields:

```yaml
routing:
  destinations:
    - name: enriched-orders
      output: orders-enriched
      transform: |
        let subtotal = msg["subtotal"];
        let tax = subtotal * 0.08;
        let total = subtotal + tax;
        #{
          orderId: msg["orderId"],
          subtotal: subtotal,
          tax: tax,
          total: total,
          items: msg["items"]
        }
```

### Pattern 4: Filter Pipeline

Progressive filtering:

```yaml
routing:
  destinations:
    # Stage 1: Valid format
    - name: stage1-valid-format
      output: stage1
      filter: 'msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$")'
    
    # Stage 2: Corporate emails
    - name: stage2-corporate
      output: stage2
      filter: |
        msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$") &&
        msg["email"].ends_with("@company.com")
    
    # Stage 3: Active corporate emails
    - name: stage3-active
      output: stage3
      filter: |
        msg["email"].matches("^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$") &&
        msg["email"].ends_with("@company.com") &&
        msg["active"] == true
```

## Troubleshooting

### Common Issues

#### 1. Messages Not Being Routed

**Symptoms**: Messages consumed but not sent to any destination.

**Check:**
```bash
# Enable debug logging
RUST_LOG=debug CONFIG_FILE=config.yaml ./streamforge
```

**Common Causes:**
- Filter not matching (check filter logic)
- Transform error (check field paths)
- All destinations filtered out

**Solution:**
```yaml
# Add catchall destination for debugging
routing:
  destinations:
    # Your filtered destinations...
    
    # Catchall (no filter = accepts all)
    - name: catchall
      output: unrouted
```

#### 2. Performance Issues

**Symptoms**: Low throughput, high latency.

**Check:**
- Thread count
- Batch size
- Network latency

**Solution:**
```yaml
threads: 8

consumer_properties:
  fetch.min.bytes: "1048576"
  max.poll.records: "500"

producer_properties:
  batch.size: "65536"
  linger.ms: "10"
```

#### 3. Memory Usage High

**Symptoms**: Process uses excessive memory.

**Check:**
- Array sizes in messages
- Number of destinations
- Batch sizes

**Solution:**
```yaml
consumer_properties:
  max.poll.records: "100"
  fetch.max.bytes: "52428800"
```

#### 4. Connection Failures

**Symptoms**: Cannot connect to Kafka.

**Check:**
```bash
# Test connectivity
nc -zv kafka-broker 9092

# Check DNS
nslookup kafka-broker

# Check config
cat config.yaml | grep bootstrap
```

**Solution:**
- Verify bootstrap servers
- Check network/firewall rules
- Verify authentication config

#### 5. Filter Not Working

**Symptoms**: Expected messages not matching filter.

**Debug:**
```yaml
# Send all to debug topic
routing:
  destinations:
    - name: debug-all
      output: debug-topic
      transform: 'msg'
```

**Check:**
- Field paths (case-sensitive)
- Value types (string vs number)
- Null values
- Regex escaping

### Debugging Tips

1. **Start Simple**: Begin with no filter, add complexity gradually
2. **Use Debug Logs**: `RUST_LOG=debug` shows filter evaluation
3. **Test Regex Separately**: Use online regex testers
4. **Validate Field Access**: Check field names and nesting
5. **Monitor Metrics**: Watch filtered vs completed counts

### Getting Help

- Check [RHAI_QUICK_REFERENCE.md](RHAI_QUICK_REFERENCE.md) for Rhai syntax
- See [ADVANCED_FILTERS.md](ADVANCED_FILTERS.md) for filter examples
- Review [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) for complete guide
- Check [PERFORMANCE.md](PERFORMANCE.md) for tuning tips
- Enable debug logging for detailed troubleshooting

## Next Steps

- [PERFORMANCE.md](PERFORMANCE.md) - Performance optimization
- [CONTRIBUTING.md](CONTRIBUTING.md) - Contributing guide
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete DSL reference
- [RHAI_QUICK_REFERENCE.md](RHAI_QUICK_REFERENCE.md) - Quick syntax reference
