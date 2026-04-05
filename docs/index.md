---
title: Home
nav_order: 1
---

# Streamforge Documentation

**High-performance Kafka streaming toolkit built in Rust**

[![Crates.io](https://img.shields.io/crates/v/streamforge.svg)](https://crates.io/crates/streamforge)
[![Documentation](https://docs.rs/streamforge/badge.svg)](https://docs.rs/streamforge)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../LICENSE)
[![CI](https://github.com/rahulbsw/streamforge/workflows/CI/badge.svg)](https://github.com/rahulbsw/streamforge/actions)

## Quick Links

- [Get Started](#quick-start) - Run in 5 minutes
- [Features](#features) - What it can do
- [Performance](#performance) - Benchmarks and comparisons
- [Documentation](#documentation) - Comprehensive guides
- [Contributing](#contributing) - Join the project

## Overview

StreamForge is a Rust rewrite of the Kafka MirrorMaker service, designed for high performance, reliability, and ease of use. It provides cross-cluster message mirroring with advanced filtering, transformation, and routing capabilities.

### Key Highlights

- 🚀 **40x Faster** than Java JSLT for filtering and transforms
- 💾 **10x Less Memory** (~50MB vs ~500MB)
- ⚡ **2.5x Higher Throughput** (25K+ msg/s vs 10K msg/s)
- 🔒 **Zero CVEs** with Chainguard base images
- 📦 **Minimal Size** (~20MB Docker image)
- 🎯 **Zero Dependencies** for DSL (custom implementation)

## Quick Start

### Installation

```bash
# Clone repository
git clone <repository-url>
cd streamforge

# Build
cargo build --release

# Or use Docker
docker pull streamforge:latest
```

### Basic Configuration

```json
{
  "appid": "mirrormaker",
  "bootstrap": "source-kafka:9092",
  "target_broker": "target-kafka:9092",
  "input": "source-topic",
  "output": "destination-topic",
  "offset": "latest",
  "threads": 4
}
```

### Run

```bash
# Direct execution
CONFIG_FILE=config.json ./target/release/streamforge

# Docker
docker run -d \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  streamforge:latest
```

See [QUICKSTART.md](QUICKSTART.md) for detailed instructions.

## Features

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Cross-Cluster Mirroring** | Mirror messages between different Kafka clusters with minimal latency |
| **Multi-Destination Routing** | Route a single message to multiple topics simultaneously |
| **Advanced Filtering** | JSON path, regex, boolean logic, array operations |
| **Powerful Transforms** | Extract, construct, map, and calculate fields |
| **Custom Partitioning** | Hash-based or field-based partition assignment |
| **Native Compression** | Gzip, Snappy, Zstd support with optimal performance |
| **Lock-Free Metrics** | Real-time performance monitoring with zero overhead |

### DSL Capabilities

The custom Domain-Specific Language supports:

#### Filters

```json
{
  "filter": "AND:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$:/status,==,active:/tier,==,premium"
}
```

- **Comparison**: `>`, `>=`, `<`, `<=`, `==`, `!=`
- **Boolean Logic**: `AND`, `OR`, `NOT`
- **Regular Expressions**: `REGEX:/path,pattern`
- **Array Operations**: `ARRAY_ALL:/path,filter`, `ARRAY_ANY:/path,filter`

#### Transforms

```json
{
  "transform": "CONSTRUCT:userId=/user/id:email=/user/email:total=ARITHMETIC:MUL,/price,1.08"
}
```

- **Field Extraction**: `/path/to/field`
- **Object Construction**: `CONSTRUCT:field1=/path1:field2=/path2`
- **Array Mapping**: `ARRAY_MAP:/users,/id`
- **Arithmetic**: `ARITHMETIC:ADD|SUB|MUL|DIV,op1,op2`

See [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) for complete reference.

## Performance

### Benchmarks

**Hardware**: 4 CPU cores, 8GB RAM
**Message Size**: 1KB
**Partitions**: 10

| Metric | Value |
|--------|-------|
| Throughput | 45K msg/s |
| Latency (p50) | 3ms |
| Latency (p99) | 12ms |
| Memory Usage | 45MB |
| CPU Usage | 150% |

### Operation Performance

| Operation | Time | Throughput |
|-----------|------|------------|
| Simple filter | 100ns | 10M ops/s |
| Boolean logic | 300ns | 3.3M ops/s |
| Regular expression | 500ns | 2M ops/s |
| Array operations | 5µs | 200K ops/s |
| Object construction | 500ns | 2M ops/s |
| Arithmetic | 50ns | 20M ops/s |

### Comparison with Java

| Metric | Java | Rust | Improvement |
|--------|------|------|-------------|
| Throughput | 10K msg/s | 25K msg/s | **2.5x** |
| Memory | 500MB | 50MB | **10x** |
| CPU | 200% | 120% | **1.7x** |
| Latency (p99) | 50ms | 15ms | **3.3x** |
| Startup | 5s | 0.1s | **50x** |
| Filter Performance | 4µs | 100ns | **40x** |

See [PERFORMANCE.md](PERFORMANCE.md) for detailed benchmarks and tuning guide.

## Use Cases

### 1. Cross-Cluster Mirroring

Mirror data between clusters for disaster recovery or data center replication.

```json
{
  "bootstrap": "cluster-a:9092",
  "target_broker": "cluster-b:9092",
  "input": "events",
  "output": "events-replica"
}
```

### 2. Content-Based Routing

Route messages to different topics based on content.

```json
{
  "destinations": [
    {
      "filter": "REGEX:/eventType,^user\\.",
      "output": "user-events"
    },
    {
      "filter": "REGEX:/eventType,^order\\.",
      "output": "order-events"
    }
  ]
}
```

### 3. Data Validation

Validate and route valid/invalid messages.

```json
{
  "destinations": [
    {
      "filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$",
      "output": "validated"
    },
    {
      "filter": "NOT:REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$",
      "output": "validation-errors"
    }
  ]
}
```

### 4. Real-time Analytics

Calculate metrics and route to analytics topics.

```json
{
  "destinations": [
    {
      "transform": "ARITHMETIC:ADD,/price,/tax",
      "output": "total-revenue"
    },
    {
      "transform": "ARITHMETIC:DIV,/conversions,/visits",
      "output": "conversion-rate"
    }
  ]
}
```

See [USAGE.md](USAGE.md) for 8 comprehensive use cases with examples.

## Documentation

### Getting Started

- **[QUICKSTART.md](QUICKSTART.md)** - Get started in 5 minutes
- **[USAGE.md](USAGE.md)** - 8 comprehensive use cases
- **[QUICK_REFERENCE.md](QUICK_REFERENCE.md)** - Quick reference card

### Configuration

- **[examples/README.md](../examples/README.md)** - Configuration examples and patterns
- **[YAML_CONFIGURATION.md](YAML_CONFIGURATION.md)** - YAML vs JSON guide (recommended!)
- **[config.advanced.yaml](../examples/configs/config.advanced.yaml)** - 17 production YAML examples
- **[config.advanced.example.json](../examples/configs/config.advanced.example.json)** - JSON examples

### Features & DSL

- **[ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md)** - Complete DSL reference
- **[DSL_FEATURES.md](DSL_FEATURES.md)** - Feature summary with benchmarks
- **[ADVANCED_FILTERS.md](ADVANCED_FILTERS.md)** - Boolean logic and complex filters

### Operations & Deployment

- **[DOCKER.md](DOCKER.md)** - Docker & Kubernetes deployment
- **[SECURITY_CONFIGURATION.md](SECURITY_CONFIGURATION.md)** - Security configuration (SSL/TLS, SASL, Kerberos)
- **[PERFORMANCE.md](PERFORMANCE.md)** - Performance tuning guide
- **[SCALING.md](SCALING.md)** - Horizontal & vertical scaling guide

### Development

- **[CONTRIBUTING.md](CONTRIBUTING.md)** - Development setup & guidelines
- **[IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md)** - Architecture details
- **[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)** - Feature tracking
- **[CHANGELOG.md](CHANGELOG.md)** - Version history

### Complete Index

- **[DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md)** - Complete documentation index

## Architecture

```
┌─────────────────┐
│  Kafka Consumer │  Read from source
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Filter Engine  │  Evaluate conditions
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Transform Engine│  Modify messages
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Router/Sink    │  Route to destinations
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Kafka Producer │  Write to targets
└─────────────────┘
```

### Key Components

- **Filter Engine**: Evaluates filters with 100ns latency
- **Transform Engine**: Modifies messages with zero-copy when possible
- **Router**: Distributes to multiple destinations in parallel
- **Metrics**: Lock-free counters with atomic operations

## Configuration Examples

**Note**: Both YAML and JSON formats are supported. YAML is recommended for complex configurations.

### Simple Mirroring

**YAML (Recommended):**
```yaml
bootstrap: kafka-a:9092
target_broker: kafka-b:9092
input: events
output: events-mirror
```

**JSON:**
```json
{
  "bootstrap": "kafka-a:9092",
  "target_broker": "kafka-b:9092",
  "input": "events",
  "output": "events-mirror"
}
```

### Advanced Routing

**YAML (Much More Readable):**
```yaml
input: events
routing:
  destinations:
    # Premium active users
    - name: premium-users
      filter: "AND:/user/tier,==,premium:/user/status,==,active"
      transform: "CONSTRUCT:userId=/user/id:tier=/user/tier"
      output: premium-events

    # High-value orders with tax
    - name: high-value
      filter: "/order/total,>,1000"
      transform: "ARITHMETIC:MUL,/order/total,1.08"
      output: high-value-orders
```

**JSON:**
```json
{
  "input": "events",
  "routing": {
    "destinations": [
      {
        "name": "premium-users",
        "filter": "AND:/user/tier,==,premium:/user/status,==,active",
        "transform": "CONSTRUCT:userId=/user/id:tier=/user/tier",
        "output": "premium-events"
      },
      {
        "name": "high-value",
        "filter": "/order/total,>,1000",
        "transform": "ARITHMETIC:MUL,/order/total,1.08",
        "output": "high-value-orders"
      }
    ]
  }
}
```

See [config.advanced.example.json](../examples/configs/config.advanced.example.json) for 17 production-ready examples.

## Contributing

We welcome contributions! Here's how to get started:

### Local Setup

```bash
# Clone and build
git clone <repository-url>
cd streamforge
cargo build

# Run tests
cargo test

# Run benchmarks
cargo bench

# Format code
cargo fmt

# Lint
cargo clippy
```

### Development Workflow

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run `cargo test` and `cargo clippy`
6. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## Comparison Matrix

### Feature Comparison

| Feature | Java MirrorMaker | StreamForge |
|---------|------------------|------------------|
| Cross-cluster mirroring | ✅ | ✅ |
| Multi-destination routing | ✅ | ✅ |
| Custom partitioning | ✅ | ✅ |
| Native compression | ✅ | ✅ |
| JSON path filters | ❌ | ✅ |
| Boolean logic | ❌ | ✅ |
| Regular expressions | ❌ | ✅ |
| Array operations | ❌ | ✅ |
| Arithmetic operations | ❌ | ✅ |
| JSLT transforms | ✅ | ✅ (40x faster) |
| JavaScript transforms | ✅ | ❌ |
| Avro serialization | ✅ | ⚠️ (planned) |

### Performance Comparison

| Metric | Java | Rust | Winner |
|--------|------|------|--------|
| Throughput | 10K msg/s | 25K msg/s | 🦀 Rust |
| Memory | 500MB | 50MB | 🦀 Rust |
| CPU Efficiency | 200% | 120% | 🦀 Rust |
| Latency p99 | 50ms | 15ms | 🦀 Rust |
| Startup Time | 5s | 0.1s | 🦀 Rust |
| Filter Speed | 4µs | 100ns | 🦀 Rust |
| Image Size | 200MB+ | 20MB | 🦀 Rust |

## Deployment Options

### Docker

```bash
# Build
docker build -t streamforge:latest .

# Run
docker run -d \
  --name mirrormaker \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  streamforge:latest
```

### Docker Compose

```bash
docker-compose up -d
```

### Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: streamforge
spec:
  replicas: 3
  template:
    spec:
      containers:
      - name: mirrormaker
        image: streamforge:latest
        resources:
          limits:
            memory: 512Mi
            cpu: 2000m
```

See [DOCKER.md](DOCKER.md) for complete deployment guide.

## FAQ

### Q: How does it compare to Kafka Connect?

A: StreamForge is simpler and more focused. It excels at high-performance mirroring with advanced filtering/transformation. Kafka Connect is better for complex integrations with many connectors.

### Q: Can I use it with Confluent Cloud?

A: Yes! It works with any Kafka cluster, including Confluent Cloud, MSK, HDInsight, etc.

### Q: Is it production-ready?

A: Yes! It's been tested with production workloads and includes comprehensive monitoring, error handling, and deployment options.

### Q: How do I migrate from Java version?

A: See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for migration guide. Most configurations are drop-in replacements with better performance.

### Q: What about Avro support?

A: Avro support is planned for v0.5.0. Currently supports JSON only.

## Support

- 📚 **Documentation**: See links above
- 🐛 **Issues**: GitHub Issues
- 💬 **Discussions**: GitHub Discussions
- 📧 **Contact**: See CONTRIBUTING.md

## License

Apache License 2.0 - See [LICENSE](../LICENSE) for details.

Copyright 2025 Rahul Jain

## Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [Tokio](https://tokio.rs/) - Async runtime
- [rdkafka](https://github.com/fede1024/rust-rdkafka) - Kafka client
- [serde](https://serde.rs/) - Serialization framework
- [Criterion](https://github.com/bheisler/criterion.rs) - Benchmarking

---

**Ready to get started?** Head to [QUICKSTART.md](QUICKSTART.md) and run your first mirror in 5 minutes!
