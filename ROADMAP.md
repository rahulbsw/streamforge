# Streamforge Roadmap

Vision and planned features for Streamforge.

## Vision

Streamforge aims to be the **fastest, most reliable, and easiest-to-use Kafka streaming toolkit**. We focus on:

1. **Performance** - Always faster than alternatives
2. **Reliability** - Production-grade stability
3. **Usability** - Simple configuration, great documentation
4. **Security** - Enterprise-ready security features
5. **Community** - Welcoming, collaborative ecosystem

---

## Current Version: 0.3.0

### What's Included

✅ **Core Features:**
- Cross-cluster mirroring
- Multi-destination routing
- Advanced filtering (DSL)
- Powerful transformations
- Custom partitioning
- Native compression
- Security (SSL/TLS, SASL, Kerberos)

✅ **Performance:**
- 40x faster than Java JSLT
- 10x less memory usage
- 2.5x higher throughput

✅ **Documentation:**
- 18 documentation files
- 13 example configurations
- Complete DSL reference
- Security guide
- Performance tuning guide

---

## Version 1.0 (Q2 2025)

**Goal**: First stable release

### Features

- [ ] **Avro Support**
  - Avro serialization/deserialization
  - Schema Registry integration
  - Schema evolution handling

- [ ] **Dead Letter Queue**
  - Configurable DLQ for failed messages
  - Retry policies
  - Error categorization

- [ ] **Prometheus Metrics**
  - Native Prometheus exporter
  - Rich metrics (latency, throughput, errors)
  - Grafana dashboard templates

- [ ] **Health Check Endpoint**
  - HTTP health endpoint
  - Kafka connectivity check
  - Resource utilization metrics

- [ ] **Enhanced CLI**
  - Better command-line interface
  - Config validation mode
  - Dry-run mode

### Performance

- [ ] Zero-copy optimizations
- [ ] SIMD operations for filtering
- [ ] Parallel message processing
- Target: 50K+ messages/second

### Documentation

- [ ] Video tutorials
- [ ] Interactive examples
- [ ] Migration guide from MM2
- [ ] Architecture deep-dive

---

## Version 1.1 (Q3 2025)

**Goal**: Advanced features and integrations

### Features

- [ ] **Schema Registry Support**
  - Full Schema Registry integration
  - Schema validation
  - Schema evolution strategies

- [ ] **Message Transformation Enhancements**
  - Nested transform composition
  - Custom transform functions
  - Jinja2-like templating

- [ ] **Exactly-Once Semantics**
  - Transactional processing
  - Idempotent producers
  - Offset management improvements

- [ ] **Web UI** (Maybe)
  - Configuration management
  - Real-time metrics dashboard
  - Pipeline visualization

### Integrations

- [ ] Confluent Cloud CLI integration
- [ ] AWS MSK IAM authentication
- [ ] Azure Event Hubs integration
- [ ] Google Cloud Pub/Sub bridge

---

## Version 1.2 (Q4 2025)

**Goal**: Enterprise features

### Features

- [ ] **Multi-Tenancy**
  - Isolated pipelines
  - Resource quotas
  - Per-tenant metrics

- [ ] **Advanced Routing**
  - Content-based routing rules engine
  - Dynamic routing table updates
  - Conditional transformations

- [ ] **Data Governance**
  - PII detection and masking
  - Data lineage tracking
  - Audit logging

- [ ] **Kubernetes Operator**
  - CRD for pipeline definitions
  - Auto-scaling based on lag
  - GitOps support

### Performance

- [ ] Adaptive batching
- [ ] Smart backpressure
- [ ] Connection pooling optimization
- Target: 100K+ messages/second

---

## Version 2.0 (2026)

**Goal**: Next-generation streaming

### Vision Features

- [ ] **Stream Processing**
  - Windowing operations
  - Joins across streams
  - Aggregations
  - Stateful processing

- [ ] **Machine Learning**
  - Real-time model inference
  - Anomaly detection
  - Auto-scaling predictions

- [ ] **Multi-Protocol Support**
  - Pulsar support
  - NATS integration
  - RabbitMQ bridge
  - HTTP/gRPC sources/sinks

- [ ] **Visual Pipeline Builder**
  - Drag-and-drop pipeline creation
  - Live preview
  - Template marketplace

---

## Community Wishlist

Features requested by the community (vote on GitHub Issues):

### High Priority

- [ ] Avro/Protobuf support (🔥 Most requested)
- [ ] Prometheus metrics
- [ ] Dead letter queue
- [ ] Web UI for monitoring

### Medium Priority

- [ ] Exactly-once semantics
- [ ] Schema Registry integration
- [ ] Kubernetes operator
- [ ] PII masking

### Low Priority

- [ ] Message deduplication
- [ ] Time-based filtering
- [ ] Geographic routing
- [ ] Multi-cloud support

---

## Research & Experiments

Experimental features we're exploring:

### Performance Innovations

- **DPDK Integration**: Zero-copy networking
- **io_uring**: Modern Linux async I/O
- **SIMD Optimization**: Vectorized operations
- **GPU Acceleration**: Parallel filtering/transforms

### New Capabilities

- **Stream SQL**: SQL-like query language
- **CDC Support**: Change Data Capture integration
- **Time-Travel**: Historical replay capabilities
- **Smart Caching**: Intelligent message caching

---

## Non-Goals

Things we explicitly don't plan to do:

❌ **Not a Stream Processor**: Use Kafka Streams or Flink for complex stream processing
❌ **Not a Data Lake**: Use dedicated storage solutions
❌ **Not a Message Queue**: Kafka is the message queue
❌ **Not a Database**: Use proper databases for persistence

---

## How to Influence the Roadmap

### 1. Vote on Issues

Vote with 👍 on GitHub issues for features you want.

### 2. Submit Proposals

Create detailed feature proposals with:
- Use case description
- Technical approach
- Benefits and tradeoffs

### 3. Contribute Code

Implement features yourself! See [CONTRIBUTING.md](docs/CONTRIBUTING.md).

### 4. Sponsor Development

Support development through GitHub Sponsors (coming soon).

---

## Release Schedule

### Cadence

- **Major releases** (X.0.0): Yearly
- **Minor releases** (0.X.0): Quarterly
- **Patch releases** (0.0.X): As needed

### Support Policy

- **Current major version**: Full support
- **Previous major version**: Security fixes for 6 months
- **Older versions**: Community support only

---

## Success Metrics

How we measure success:

### Adoption

- **GitHub stars**: 1K+ (v1.0), 5K+ (v2.0)
- **Downloads**: 10K/month (v1.0), 50K/month (v2.0)
- **Production usage**: 100+ companies (v1.0), 500+ (v2.0)

### Performance

- **Throughput**: 50K msg/s (v1.0), 100K msg/s (v2.0)
- **Latency p99**: <10ms (v1.0), <5ms (v2.0)
- **Memory**: <50MB (v1.0), <30MB (v2.0)

### Community

- **Contributors**: 50+ (v1.0), 200+ (v2.0)
- **Stars**: 1K+ (v1.0), 5K+ (v2.0)
- **Conference talks**: 3+ (v1.0), 10+ (v2.0)

---

## Get Involved

### Ways to Contribute

1. **Use it** - Try Streamforge and provide feedback
2. **Report bugs** - Help us improve quality
3. **Request features** - Tell us what you need
4. **Write docs** - Improve documentation
5. **Write code** - Implement features
6. **Spread the word** - Star, tweet, blog about it

### Communication

- **GitHub Issues**: Feature requests and bugs
- **GitHub Discussions**: Questions and ideas
- **Email**: rahul.oracle.db@gmail.com

---

## Changelog

See [docs/CHANGELOG.md](docs/CHANGELOG.md) for version history.

---

**Last Updated**: 2025-03-09
**Current Version**: 0.3.0
**Next Release**: 1.0.0 (Q2 2025)
