---
title: All Docs
nav_order: 14
---

# Documentation Index

Complete guide to all Streamforge documentation.

## Quick Navigation

### New to Streamforge?
1. [README.md](../README.md) - Project overview
2. [QUICKSTART.md](QUICKSTART.md) - Get started in 5 minutes
3. [USAGE.md](USAGE.md) - Pick a use case

### Need to Configure?
1. [examples/README.md](../examples/README.md) - Configuration examples
2. [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) - YAML vs JSON (recommended reading!)
3. [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) - Complete DSL reference
4. [config.advanced.yaml](../examples/configs/config.advanced.yaml) - 17 production examples

### Ready to Deploy?
1. [DOCKER.md](DOCKER.md) - Docker deployment
2. [PERFORMANCE.md](PERFORMANCE.md) - Performance tuning
3. [SCALING.md](SCALING.md) - Horizontal/vertical scaling

### Want to Contribute?
1. [CONTRIBUTING.md](CONTRIBUTING.md) - Development setup
2. [IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md) - Architecture
3. [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) - Feature status

---

## Documentation Structure

### 📖 Getting Started

#### [README.md](../README.md)
**Main project overview and quick reference**
- Project features and highlights
- Quick start guide
- Configuration examples (YAML and JSON)
- Performance comparison with Java
- Links to all documentation

#### [QUICKSTART.md](QUICKSTART.md)
**Get started in 5 minutes**
- Prerequisites and installation
- Basic configuration
- Running the application
- First mirror setup
- Testing and validation

#### [USAGE.md](USAGE.md)
**Comprehensive use cases guide (700+ lines)**
- 8 real-world use cases with complete examples
- Configuration patterns and best practices
- Troubleshooting common issues
- Debugging tips and solutions
- **Essential reading for practical usage**

---

### ⚙️ Configuration

#### [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) ⭐ **RECOMMENDED**
**YAML configuration guide (400+ lines)**
- Why YAML is better for complex configs
- Format comparison (YAML vs JSON)
- Multi-line strings and comments
- Migration from JSON
- Best practices
- **Much more readable for complex filters!**

#### Example Configuration Files

See [examples/README.md](../examples/README.md) for comprehensive configuration examples and patterns.

**YAML Format (Recommended):**
- [config.example.yaml](../examples/configs/config.example.yaml) - Simple single-destination
- [config.multidest.yaml](../examples/configs/config.multidest.yaml) - Multi-destination with comments
- [config.advanced.yaml](../examples/configs/config.advanced.yaml) - 17 production examples
- 🆕 [config.envelope-simple.yaml](../examples/config.envelope-simple.yaml) - 6 envelope patterns (concise)
- 🆕 [config.envelope-features.yaml](../examples/config.envelope-features.yaml) - 8 envelope examples (comprehensive)
- 🆕 [config.with-observability.yaml](../examples/config.with-observability.yaml) - Observability with metrics

**JSON Format (Backward Compatible):**
- [config.example.json](../examples/configs/config.example.json) - Simple single-destination
- [config.multi-destination.example.json](../examples/configs/config.multi-destination.example.json) - Multi-destination
- [config.advanced.example.json](../examples/configs/config.advanced.example.json) - Advanced examples

---

### 🎯 Features & DSL

#### [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md)
**Complete DSL reference (1000+ lines)**
- Array operations (ARRAY_ALL, ARRAY_ANY, ARRAY_MAP)
- Regular expressions with examples
- Arithmetic operations (ADD/SUB/MUL/DIV)
- 🆕 **Envelope operations** (keys, headers, timestamps)
- Complex real-world examples
- Performance characteristics
- Error handling
- **Most comprehensive DSL guide**

#### [ENVELOPE_MIGRATION_GUIDE.md](ENVELOPE_MIGRATION_GUIDE.md) 🆕
**Envelope features migration guide (600+ lines)**
- What's new in envelope operations
- Backward compatibility assurance
- Migration scenarios (5 real-world examples)
- Performance impact analysis
- Common pitfalls and solutions
- Testing and validation
- **Essential for upgrading to envelope features**

#### [ENVELOPE_FEATURE_DESIGN.md](ENVELOPE_FEATURE_DESIGN.md)
**Envelope feature design document (450+ lines)**
- Complete design and architecture
- DSL syntax specifications
- Implementation strategy
- Performance considerations
- Security considerations
- **Technical deep-dive for contributors**

#### [DSL_FEATURES.md](DSL_FEATURES.md)
**Feature summary and reference**
- Complete feature list
- Configuration examples
- Performance benchmarks
- Comparison with Java JSLT (40x faster!)
- Best practices
- Limitations and workarounds

#### [ADVANCED_FILTERS.md](ADVANCED_FILTERS.md)
**Boolean logic and complex filtering**
- AND/OR/NOT composition
- Nested logic patterns
- Real-world filtering examples
- Performance considerations

#### [QUICK_REFERENCE.md](QUICK_REFERENCE.md)
**Quick reference card**
- DSL syntax cheat sheet
- Common configuration patterns
- Performance targets
- Troubleshooting quick tips
- **Printable reference**

---

### 🚀 Operations & Deployment

#### [DOCKER.md](DOCKER.md)
**Complete Docker deployment guide (480+ lines)**
- Multi-stage Dockerfile guide
- Docker Compose setup
- Security best practices
- Kubernetes deployment
- Health checks and monitoring
- Troubleshooting

#### [SECURITY_CONFIGURATION.md](SECURITY_CONFIGURATION.md)
**Complete security configuration guide (600+ lines)**
- SSL/TLS encryption (one-way and mutual TLS)
- SASL authentication (PLAIN, SCRAM-SHA-256/512, GSSAPI)
- Kerberos configuration
- Cloud provider examples (Confluent Cloud, AWS MSK, Azure Event Hubs)
- Certificate generation and management
- Troubleshooting security issues
- **Essential for production deployments**

#### [PERFORMANCE.md](PERFORMANCE.md)
**Performance tuning and optimization (400+ lines)**
- Detailed benchmarks with hardware specs
- Configuration tuning (threads, batch sizes, etc.)
- Best practices for filters and transforms
- Monitoring and metrics
- Troubleshooting performance issues
- Advanced optimization techniques
- **Essential for production deployments**

#### [OBSERVABILITY_QUICKSTART.md](OBSERVABILITY_QUICKSTART.md) 🆕
**Get metrics running in 5 minutes (400+ lines)**
- Quick start configuration
- Prometheus setup
- Common queries (throughput, errors, lag, latency)
- Grafana dashboard examples
- Alerting rules
- Testing locally
- Troubleshooting
- **Essential for production monitoring**

#### [OBSERVABILITY_METRICS_DESIGN.md](OBSERVABILITY_METRICS_DESIGN.md) 🆕
**Complete metrics design document (2000+ lines)**
- 60+ Prometheus metrics definitions
- Per-destination metrics (throughput, latency, errors)
- Kafka consumer lag monitoring
- Filter and transform operation tracking
- Prometheus queries and dashboards
- Grafana dashboard JSON
- Alerting rules and runbooks
- Performance impact analysis (< 2% overhead)
- **Complete observability reference**

#### [SCALING.md](SCALING.md)
**Horizontal and vertical scaling guide (600+ lines)**
- Scaling fundamentals and architecture
- Horizontal scaling with consumer groups
- Vertical scaling (threads, memory, CPU)
- Kafka partition strategy
- Load balancing patterns
- Kubernetes auto-scaling (HPA)
- Real-world scaling examples
- Troubleshooting scaling issues
- **Essential for high-throughput deployments**

---

### 💻 Development

#### [CONTRIBUTING.md](CONTRIBUTING.md)
**Complete contributing guide (500+ lines)**
- Local development setup (Rust, IDE, Kafka)
- Project structure explanation
- Development workflow
- Testing and benchmarking
- Code style guidelines
- Adding new features (filters, transforms)
- Pull request process
- **Required reading for contributors**

#### [IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md)
**Architecture and design decisions**
- System architecture
- Module breakdown
- Design patterns
- Performance optimizations
- Technical deep-dive

#### [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)
**Feature tracking and roadmap**
- Completed features
- In-progress features
- Feature comparison matrix
- Migration guide from Java
- Future roadmap

#### [CHANGELOG.md](CHANGELOG.md)
**Version history**
- Version 0.2.0 - Advanced DSL + YAML support
- Version 0.1.0 - Initial release
- Detailed feature additions
- Roadmap for future versions

---

### 📊 GitHub Pages

#### [docs/index.md](index.md)
**GitHub Pages landing page**
- Project overview
- Quick links to all documentation
- Feature showcase
- Performance benchmarks
- Use case examples
- Comparison matrices
- FAQ section
- **Designed for external users**

---

## Quick Reference by Use Case

### "I want to..."

#### Get Started Quickly
→ Read [QUICKSTART.md](QUICKSTART.md) (10 min)

#### See Real-World Examples
→ Read [USAGE.md](USAGE.md) - 8 complete use cases (20 min)

#### Learn Configuration Format
→ Read [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md) (15 min)

#### Learn the DSL
→ Read [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md) (20 min)

#### Use Envelope Features (Keys, Headers, Timestamps)
→ Read [ENVELOPE_MIGRATION_GUIDE.md](ENVELOPE_MIGRATION_GUIDE.md) (15 min)  
→ See [config.envelope-simple.yaml](../examples/config.envelope-simple.yaml) for quick patterns

#### Optimize Performance
→ Read [PERFORMANCE.md](PERFORMANCE.md) (20 min)

#### Monitor with Prometheus Metrics
→ Read [OBSERVABILITY_QUICKSTART.md](OBSERVABILITY_QUICKSTART.md) (10 min)  
→ See [config.with-observability.yaml](../examples/config.with-observability.yaml) for setup  
→ Use [test_metrics.sh](../scripts/test_metrics.sh) to verify metrics

#### Scale to High Throughput
→ Read [SCALING.md](SCALING.md) (20 min)

#### Deploy with Docker
→ Read [DOCKER.md](DOCKER.md) (15 min)

#### Contribute Code
→ Read [CONTRIBUTING.md](CONTRIBUTING.md) (30 min)

#### Migrate from Java
→ Read [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) (10 min)

#### Understand Architecture
→ Read [IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md) (20 min)

---

## Documentation Statistics

| Document | Lines | Focus | Audience |
|----------|-------|-------|----------|
| README.md | 260 | Overview | Everyone |
| QUICKSTART.md | ~200 | Getting Started | New Users |
| USAGE.md | 700+ | Use Cases | Operators |
| YAML_CONFIGURATION.md | 400+ | Configuration | Everyone |
| ADVANCED_DSL_GUIDE.md | 1000+ | DSL Reference | Developers |
| ENVELOPE_MIGRATION_GUIDE.md | 600+ | Migration | Operators |
| ENVELOPE_FEATURE_DESIGN.md | 450+ | Design | Contributors |
| DSL_FEATURES.md | 400+ | Feature Summary | Developers |
| ADVANCED_FILTERS.md | 300+ | Boolean Logic | Developers |
| OBSERVABILITY_QUICKSTART.md | 400+ | Metrics Setup | Operators/DevOps |
| OBSERVABILITY_METRICS_DESIGN.md | 2000+ | Observability | Operators/DevOps |
| PERFORMANCE.md | 400+ | Optimization | Operators |
| SCALING.md | 600+ | Scaling | Operators/DevOps |
| SECURITY_CONFIGURATION.md | 600+ | Security | DevOps |
| CONTRIBUTING.md | 500+ | Development | Contributors |
| DOCKER.md | 480+ | Deployment | DevOps |
| QUICK_REFERENCE.md | 200+ | Quick Ref | Everyone |
| IMPLEMENTATION_NOTES.md | 300+ | Architecture | Developers |
| IMPLEMENTATION_STATUS.md | 200+ | Features | Everyone |
| CHANGELOG.md | 200+ | History | Everyone |
| docs/index.md | 600+ | Overview | External Users |

**Total**: ~10,000+ lines of documentation

---

## For New Users

**Recommended Reading Order:**

1. **[README.md](../README.md)** - Overview (5 min)
2. **[QUICKSTART.md](QUICKSTART.md)** - Get it running (10 min)
3. **[examples/README.md](../examples/README.md)** - Configuration examples (10 min)
4. **[YAML_CONFIGURATION.md](YAML_CONFIGURATION.md)** - Learn config format (15 min)
5. **[USAGE.md](USAGE.md)** - Pick a use case (15 min)
6. **[ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md)** - Learn DSL (20 min)
7. **[PERFORMANCE.md](PERFORMANCE.md)** - Tune for production (15 min)
8. **[OBSERVABILITY_QUICKSTART.md](OBSERVABILITY_QUICKSTART.md)** - Enable metrics (10 min)
9. **[SCALING.md](SCALING.md)** - Scale if needed (20 min)

**Total**: ~120 minutes to go from zero to production-ready at scale with full observability

---

## For Contributors

**Recommended Reading Order:**

1. **[README.md](../README.md)** - Understand the project (5 min)
2. **[CONTRIBUTING.md](CONTRIBUTING.md)** - Setup environment (30 min)
3. **[IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md)** - Understand architecture (20 min)
4. **[IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)** - See what's needed (10 min)
5. **Run benchmarks** - Baseline performance (5 min)

**Total**: ~70 minutes to become productive contributor

---

## Performance Benchmarks

### Performance & Benchmarks

**Overview:**
- **[benchmarks/README.md](../benchmarks/README.md)** - Benchmarks overview and methodology

**Performance Results:**
- **[benchmarks/results/CONCURRENT_PROCESSING_RESULTS.md](../benchmarks/results/CONCURRENT_PROCESSING_RESULTS.md)** - 132x throughput improvement (83 → 11,000 msg/s)
- **[benchmarks/results/SCALING_TEST_RESULTS.md](../benchmarks/results/SCALING_TEST_RESULTS.md)** - Linear scaling validation (8 threads, 8 partitions)
- **[benchmarks/results/BENCHMARKS.md](../benchmarks/results/BENCHMARKS.md)** - Comprehensive benchmark analysis
- **[benchmarks/results/BENCHMARK_RESULTS.md](../benchmarks/results/BENCHMARK_RESULTS.md)** - DSL micro-benchmark results
- **[benchmarks/results/DELIVERY_SEMANTICS_IMPLEMENTATION.md](../benchmarks/results/DELIVERY_SEMANTICS_IMPLEMENTATION.md)** - At-least-once vs at-most-once

**Test Configurations:**
- **[benchmarks/configs/](../benchmarks/configs/)** - Test YAML configurations for reproducing benchmarks

**Running Micro-Benchmarks:**
```bash
# Run all criterion benchmarks
cargo bench

# Specific benchmarks
cargo bench filter_benchmarks
cargo bench transform_benchmarks

# Save baseline for comparison
cargo bench -- --save-baseline main
```

**Benchmark Code:**
- **[benches/filter_benchmarks.rs](../benches/filter_benchmarks.rs)** - Filter DSL micro-benchmarks (44-145ns)
- **[benches/transform_benchmarks.rs](../benches/transform_benchmarks.rs)** - Transform DSL micro-benchmarks (810-1,633ns)

---

## Configuration Format Guide

### YAML (Recommended)

**Advantages:**
- ✅ Comments for documentation
- ✅ Multi-line strings
- ✅ Less punctuation
- ✅ 20-30% fewer lines
- ✅ Much more readable

**Use for:**
- Multi-destination configs (3+)
- Complex filters and transforms
- Production configurations
- Team collaboration

**Example:**
```yaml
routing:
  destinations:
    # Email validation pipeline
    - output: validated-users
      description: Validate email format
      filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
      transform: "CONSTRUCT:email=/user/email:name=/user/name"
```

### JSON (Backward Compatible)

**Advantages:**
- ✅ Programmatically generated
- ✅ Strict schema validation
- ✅ Widely supported

**Use for:**
- Simple single-destination configs
- CI/CD templates
- API responses

**Example:**
```json
{
  "routing": {
    "destinations": [{
      "output": "validated-users",
      "filter": "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
    }]
  }
}
```

---

## Documentation Quality

### ✅ Complete Coverage

- [x] Getting started guide
- [x] Comprehensive use cases
- [x] Complete DSL reference
- [x] Configuration formats (YAML & JSON)
- [x] Performance tuning guide
- [x] Scaling guide
- [x] Contributing guide with setup
- [x] Docker deployment guide
- [x] Architecture documentation
- [x] Example configurations
- [x] Benchmarks and tests
- [x] GitHub Pages landing page
- [x] Quick reference card

### ✅ Best Practices

- [x] Real-world examples
- [x] Code snippets with explanations
- [x] Troubleshooting sections
- [x] Performance considerations
- [x] Security best practices
- [x] Error handling guidance
- [x] Quick reference tables
- [x] Comparison matrices

### ✅ Maintenance

- [x] Version history (CHANGELOG.md)
- [x] Feature status tracking
- [x] Roadmap for future work
- [x] Clear ownership
- [x] Contributing process
- [x] Update guidelines

---

## Maintenance Guide

### When Adding Features

Update relevant docs:
- [ ] README.md (if core feature)
- [ ] ADVANCED_DSL_GUIDE.md (if DSL feature)
- [ ] DSL_FEATURES.md (if DSL feature)
- [ ] YAML_CONFIGURATION.md (if config change)
- [ ] IMPLEMENTATION_STATUS.md (feature tracking)
- [ ] CHANGELOG.md (version history)

Add examples:
- [ ] Code snippets in docs
- [ ] Configuration examples (YAML and JSON)
- [ ] Test cases

Update benchmarks:
- [ ] Add benchmark tests
- [ ] Run and document results
- [ ] Update performance tables

### Documentation Review Checklist

Before release:
- [ ] All links work
- [ ] Code examples compile
- [ ] Benchmarks run successfully
- [ ] No outdated information
- [ ] Consistent formatting
- [ ] Clear navigation
- [ ] YAML examples updated
- [ ] JSON examples still work

---

## Deprecated Documentation (Removed)

The following files have been **removed** as they are outdated or superseded:

- ~~`PROJECT_SUMMARY.md`~~ - Superseded by README.md
- ~~`FILTERS_AND_TRANSFORMS.md`~~ - Superseded by ADVANCED_DSL_GUIDE.md
- ~~`JMESPATH_GUIDE.md`~~ - We use custom DSL, not JMESPath
- ~~`FILTER_SUMMARY.md`~~ - Superseded by DSL_FEATURES.md
- ~~`FINAL_SUMMARY.md`~~ - Temporary progress file
- ~~`DOCUMENTATION_COMPLETE.md`~~ - Internal tracking file
- ~~`YAML_SUPPORT_SUMMARY.md`~~ - Merged into YAML_CONFIGURATION.md

---

## External Links

### Official Documentation
- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs/tokio/tutorial)
- [rdkafka Documentation](https://docs.rs/rdkafka/)
- [Kafka Documentation](https://kafka.apache.org/documentation/)

### Related Projects
- [Kafka Connect](https://docs.confluent.io/platform/current/connect/index.html)
- [MirrorMaker 2](https://cwiki.apache.org/confluence/display/KAFKA/KIP-382%3A+MirrorMaker+2.0)
- [Chainguard Images](https://www.chainguard.dev/chainguard-images)

### YAML Resources
- [YAML Specification](https://yaml.org/spec/)
- [YAML Validator](https://www.yamllint.com/)
- [yq - YAML Processor](https://github.com/mikefarah/yq)

---

## Feedback

Documentation improvements are always welcome!

- Found an error? Open an issue
- Missing information? Submit a PR
- Have a question? Start a discussion

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

---

**Last Updated**: 2026-04-03
**Documentation Version**: 2.1 (Added Envelope Operations - keys, headers, timestamps)
**Project Version**: 0.3.0
