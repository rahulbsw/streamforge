---
title: Usage Guide
nav_order: 3
---

# Usage Guide

Complete guide covering various use cases for StreamForge implementation.

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
# Using default config location
CONFIG_FILE=config.json ./target/release/streamforge

# With logging
RUST_LOG=info CONFIG_FILE=config.json ./target/release/streamforge

# With debug logging
RUST_LOG=debug CONFIG_FILE=config.json ./target/release/streamforge
```

### Docker

```bash
# Build image
docker build -t streamforge:latest .

# Run with config
docker run -d \
  --name mirrormaker \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  -e RUST_LOG=info \
  streamforge:latest
```

## Use Cases

### Use Case 1: Simple Cross-Cluster Mirroring

**Scenario**: Mirror all messages from a topic in Cluster A to Cluster B without modification.

**Configuration:**

```json
{
  "appid": "simple-mirror",
  "bootstrap": "cluster-a:9092",
  "target_broker": "cluster-b:9092",
  "input": "source-topic",
  "output": "destination-topic",
  "offset": "latest",
  "threads": 4,
  "compression": {
    "compression_type": "raw",
    "compression_algo": "gzip"
  }
}
```

**Use When:**
- Disaster recovery (DR) setup
- Data center replication
- Cluster migration
- Backup/archive purposes

**Performance Tuning:**
```json
{
  "threads": 8,
  "consumer_properties": {
    "fetch.min.bytes": "1048576",
    "fetch.wait.max.ms": "500"
  },
  "producer_properties": {
    "batch.size": "65536",
    "linger.ms": "10",
    "compression.type": "gzip"
  }
}
```

### Use Case 2: Content-Based Routing

**Scenario**: Route messages to different topics based on content (e.g., route by event type, user tier, or region).

**Configuration:**

```json
{
  "appid": "content-router",
  "bootstrap": "kafka:9092",
  "target_broker": "kafka:9092",
  "input": "events",
  "routing": {
    "routing_type": "content",
    "path": "/eventType",
    "destinations": [
      {
        "name": "user-events",
        "output": "users",
        "filter": "REGEX:/eventType,^user\\.",
        "transform": "/payload",
        "partition": "/userId"
      },
      {
        "name": "order-events",
        "output": "orders",
        "filter": "REGEX:/eventType,^order\\.",
        "transform": "CONSTRUCT:orderId=/orderId:amount=/amount:status=/status",
        "partition": "/orderId"
      },
      {
        "name": "payment-events",
        "output": "payments",
        "filter": "REGEX:/eventType,^payment\\.",
        "transform": "/payload",
        "partition": "/paymentId"
      },
      {
        "name": "audit-all",
        "output": "audit-log",
        "transform": "CONSTRUCT:eventType=/eventType:timestamp=/timestamp:userId=/userId"
      }
    ]
  }
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

```json
{
  "appid": "validator",
  "bootstrap": "kafka:9092",
  "input": "raw-data",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "valid-emails",
        "output": "validated-users",
        "filter": "AND:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$:/name,!=,:/age,>,0",
        "transform": "CONSTRUCT:id=/id:email=/email:name=/name:age=/age",
        "partition": "/id"
      },
      {
        "name": "invalid-email-format",
        "output": "validation-errors",
        "filter": "NOT:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w{2,}$",
        "transform": "CONSTRUCT:error=email_format_invalid:record=/",
        "partition": "/id"
      },
      {
        "name": "missing-required-fields",
        "output": "validation-errors",
        "filter": "OR:/name,==,:/email,==,:/age,<=,0",
        "transform": "CONSTRUCT:error=missing_required_fields:record=/",
        "partition": "/id"
      }
    ]
  }
}
```

**Use When:**
- Data quality enforcement
- ETL pipelines
- Input sanitization
- Compliance checking

### Use Case 4: Multi-Environment Deployment

**Scenario**: Mirror production data to staging/testing environments with data masking.

**Configuration:**

```json
{
  "appid": "prod-to-staging",
  "bootstrap": "prod-kafka:9092",
  "target_broker": "staging-kafka:9092",
  "input": "production-events",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "staging-safe-data",
        "output": "staging-events",
        "filter": "NOT:/sensitive,==,true",
        "transform": "CONSTRUCT:id=/id:type=/type:timestamp=/timestamp:data=/nonSensitiveData",
        "partition": "/id"
      },
      {
        "name": "test-sample",
        "output": "test-events",
        "filter": "AND:NOT:/sensitive,==,true:/testFlag,==,true",
        "transform": "/",
        "partition": "/id"
      }
    ]
  }
}
```

**Use When:**
- Testing with production-like data
- Development environment setup
- QA validation
- Performance testing

### Use Case 5: Event Streaming Platform

**Scenario**: Build a central event bus that distributes events to multiple downstream consumers.

**Configuration:**

```json
{
  "appid": "event-bus",
  "bootstrap": "central-kafka:9092",
  "input": "event-stream",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "analytics",
        "output": "analytics-events",
        "filter": "ARRAY_ANY:/tags,/value,==,analytics",
        "transform": "CONSTRUCT:eventId=/eventId:type=/type:userId=/userId:timestamp=/timestamp:metrics=/metrics"
      },
      {
        "name": "notifications",
        "output": "notification-queue",
        "filter": "OR:/priority,==,high:/priority,==,urgent",
        "transform": "CONSTRUCT:userId=/userId:message=/message:type=/notificationType"
      },
      {
        "name": "billing",
        "output": "billing-events",
        "filter": "REGEX:/type,^(usage|subscription|payment)",
        "transform": "CONSTRUCT:userId=/userId:amount=/amount:type=/type:timestamp=/timestamp"
      },
      {
        "name": "audit",
        "output": "audit-trail",
        "filter": "REGEX:/action,^(create|update|delete)",
        "transform": "/"
      },
      {
        "name": "ml-features",
        "output": "ml-training",
        "filter": "/mlRelevant,==,true",
        "transform": "ARRAY_MAP:/features,/value"
      }
    ]
  }
}
```

**Use When:**
- Event-driven microservices
- Real-time data distribution
- CQRS pattern implementation
- Event sourcing

### Use Case 6: Data Lake Ingestion

**Scenario**: Ingest data into data lake while maintaining real-time processing streams.

**Configuration:**

```json
{
  "appid": "data-lake-ingestion",
  "bootstrap": "kafka:9092",
  "input": "application-events",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "raw-archive",
        "output": "datalake-raw",
        "transform": "/",
        "comment": "Archive everything"
      },
      {
        "name": "processed-metrics",
        "output": "datalake-metrics",
        "filter": "/metrics,>,0",
        "transform": "CONSTRUCT:timestamp=/timestamp:source=/source:metrics=/metrics"
      },
      {
        "name": "realtime-high-priority",
        "output": "realtime-processing",
        "filter": "AND:/priority,==,high:/processingTime,<,1000",
        "transform": "CONSTRUCT:id=/id:priority=/priority:data=/data"
      },
      {
        "name": "anomaly-detection",
        "output": "anomaly-queue",
        "filter": "OR:/errorRate,>,0.1:/responseTime,>,5000",
        "transform": "CONSTRUCT:source=/source:metric=/metricName:value=/metricValue:threshold=/threshold"
      }
    ]
  }
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

```json
{
  "appid": "realtime-analytics",
  "bootstrap": "kafka:9092",
  "input": "raw-metrics",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "revenue-total",
        "output": "revenue-metrics",
        "filter": "/order/status,==,completed",
        "transform": "ARITHMETIC:ADD,/order/amount,/order/tax",
        "partition": "/order/customerId"
      },
      {
        "name": "conversion-rate",
        "output": "conversion-metrics",
        "filter": "AND:/visits,>,0:/conversions,>,0",
        "transform": "ARITHMETIC:DIV,/conversions,/visits",
        "partition": "/campaignId"
      },
      {
        "name": "user-engagement",
        "output": "engagement-metrics",
        "filter": "ARRAY_ANY:/sessions,/duration,>,300",
        "transform": "CONSTRUCT:userId=/userId:sessions=ARRAY_MAP:/sessions,/duration",
        "partition": "/userId"
      },
      {
        "name": "error-rates",
        "output": "error-metrics",
        "filter": "AND:/requests,>,0:/errors,>,0",
        "transform": "ARITHMETIC:DIV,/errors,/requests",
        "partition": "/serviceId"
      }
    ]
  }
}
```

**Use When:**
- Real-time dashboards
- KPI monitoring
- Business intelligence
- Alerting systems

### Use Case 8: Microservices Integration

**Scenario**: Connect multiple microservices through event-driven communication.

**Configuration:**

```json
{
  "appid": "microservices-hub",
  "bootstrap": "kafka:9092",
  "input": "service-events",
  "routing": {
    "routing_type": "content",
    "destinations": [
      {
        "name": "user-service",
        "output": "user-service-events",
        "filter": "REGEX:/aggregate,^User",
        "transform": "CONSTRUCT:aggregateId=/aggregateId:eventType=/eventType:payload=/payload",
        "partition": "/aggregateId"
      },
      {
        "name": "order-service",
        "output": "order-service-events",
        "filter": "REGEX:/aggregate,^Order",
        "transform": "CONSTRUCT:aggregateId=/aggregateId:eventType=/eventType:payload=/payload",
        "partition": "/aggregateId"
      },
      {
        "name": "inventory-service",
        "output": "inventory-service-events",
        "filter": "REGEX:/aggregate,^(Inventory|Product)",
        "transform": "CONSTRUCT:aggregateId=/aggregateId:eventType=/eventType:payload=/payload",
        "partition": "/aggregateId"
      },
      {
        "name": "notification-service",
        "output": "notification-events",
        "filter": "ARRAY_ANY:/tags,/value,==,notify",
        "transform": "CONSTRUCT:userId=/userId:message=/message:channels=ARRAY_MAP:/channels,/type"
      },
      {
        "name": "cross-service-saga",
        "output": "saga-coordinator",
        "filter": "/sagaId,!=,",
        "transform": "CONSTRUCT:sagaId=/sagaId:step=/step:status=/status:data=/data"
      }
    ]
  }
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

```json
{
  "destinations": [
    {
      "name": "full-archive",
      "output": "archive",
      "transform": "/"
    },
    {
      "name": "minimal-log",
      "output": "logs",
      "transform": "CONSTRUCT:id=/id:timestamp=/timestamp:level=/level"
    },
    {
      "name": "error-only",
      "output": "errors",
      "filter": "REGEX:/level,^(ERROR|FATAL)",
      "transform": "CONSTRUCT:id=/id:error=/error:stackTrace=/stackTrace"
    }
  ]
}
```

### Pattern 2: Conditional Routing

Route based on complex conditions:

```json
{
  "destinations": [
    {
      "name": "premium-fast-lane",
      "output": "premium-queue",
      "filter": "AND:/user/tier,==,premium:/priority,==,high",
      "transform": "/"
    },
    {
      "name": "standard-processing",
      "output": "standard-queue",
      "filter": "NOT:AND:/user/tier,==,premium:/priority,==,high",
      "transform": "/"
    }
  ]
}
```

### Pattern 3: Data Enrichment

Add calculated fields:

```json
{
  "destinations": [
    {
      "name": "with-total",
      "output": "enriched-orders",
      "transform": "CONSTRUCT:orderId=/orderId:subtotal=/subtotal:tax=ARITHMETIC:MUL,/subtotal,0.08:total=ARITHMETIC:MUL,/subtotal,1.08"
    }
  ]
}
```

Note: Nested transforms not yet supported; apply sequentially.

### Pattern 4: Filter Pipeline

Progressive filtering:

```json
{
  "destinations": [
    {
      "name": "stage1-valid-format",
      "output": "stage1",
      "filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
    },
    {
      "name": "stage2-corporate",
      "output": "stage2",
      "filter": "AND:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$:REGEX:/email,@company\\.com$"
    },
    {
      "name": "stage3-active",
      "output": "stage3",
      "filter": "AND:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$:REGEX:/email,@company\\.com$:/active,==,true"
    }
  ]
}
```

## Troubleshooting

### Common Issues

#### 1. Messages Not Being Routed

**Symptoms**: Messages consumed but not sent to any destination.

**Check:**
```bash
# Enable debug logging
RUST_LOG=debug CONFIG_FILE=config.json ./streamforge
```

**Common Causes:**
- Filter not matching (check filter logic)
- Transform error (check field paths)
- All destinations filtered out

**Solution:**
```json
{
  "destinations": [
    {
      "name": "catchall",
      "output": "unrouted",
      "comment": "No filter = accepts all"
    }
  ]
}
```

#### 2. Performance Issues

**Symptoms**: Low throughput, high latency.

**Check:**
- Thread count
- Batch size
- Network latency

**Solution:**
```json
{
  "threads": 8,
  "consumer_properties": {
    "fetch.min.bytes": "1048576",
    "max.poll.records": "500"
  },
  "producer_properties": {
    "batch.size": "65536",
    "linger.ms": "10"
  }
}
```

#### 3. Memory Usage High

**Symptoms**: Process uses excessive memory.

**Check:**
- Array sizes in messages
- Number of destinations
- Batch sizes

**Solution:**
```json
{
  "consumer_properties": {
    "max.poll.records": "100",
    "fetch.max.bytes": "52428800"
  }
}
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
cat config.json | grep bootstrap
```

**Solution:**
- Verify bootstrap servers
- Check network/firewall rules
- Verify authentication config

#### 5. Filter Not Working

**Symptoms**: Expected messages not matching filter.

**Debug:**
```json
{
  "destinations": [
    {
      "name": "debug-all",
      "output": "debug-topic",
      "transform": "/",
      "comment": "Send all to debug topic"
    }
  ]
}
```

**Check:**
- Field paths (case-sensitive)
- Value types (string vs number)
- Regex escaping (`\\` for special chars)

### Debugging Tips

1. **Start Simple**: Begin with no filter, add complexity gradually
2. **Use Debug Logs**: `RUST_LOG=debug` shows filter evaluation
3. **Test Regex Separately**: Use online regex testers
4. **Validate JSON Paths**: Check field names and nesting
5. **Monitor Metrics**: Watch filtered vs completed counts

### Getting Help

- Check `ADVANCED_DSL_GUIDE.md` for DSL syntax
- See `PERFORMANCE.md` for tuning tips
- Review example configs in `config*.example.json`
- Enable debug logging for detailed troubleshooting

## Next Steps

- [PERFORMANCE.md](PERFORMANCE.md) - Performance optimization
- [CONTRIBUTING.md](CONTRIBUTING.md) - Contributing guide
- [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete DSL reference
