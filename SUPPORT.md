# Support

## Need Help?

We're here to help you get the most out of Streamforge. Here are the best ways to get support:

### 📚 Documentation

Start with our comprehensive documentation:

- **[Quick Start Guide](docs/QUICKSTART.md)** - Get up and running in 5 minutes
- **[Usage Guide](docs/USAGE.md)** - 8 real-world use cases with complete examples
- **[Documentation Index](docs/DOCUMENTATION_INDEX.md)** - Complete guide to all documentation
- **[Quick Reference](docs/QUICK_REFERENCE.md)** - Handy reference card

### 💬 Community Support

- **GitHub Discussions** - Ask questions and share knowledge
  - [Start a discussion](https://github.com/rahulbsw/streamforge/discussions)
  
- **GitHub Issues** - Report bugs and request features
  - [Report a bug](https://github.com/rahulbsw/streamforge/issues/new)
  - [Search existing issues](https://github.com/rahulbsw/streamforge/issues)

### 🔒 Security Issues

**Please do not report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability, please see our [Security Policy](SECURITY.md) for responsible disclosure instructions.

### 📧 Direct Contact

For other inquiries:
- **Email**: rahul.oracle.db@gmail.com
- **Response Time**: We aim to respond within 48 hours

---

## Before Asking for Help

To help us help you faster, please:

### 1. Check the Documentation

- Browse the [documentation index](docs/DOCUMENTATION_INDEX.md)
- Search the [existing issues](https://github.com/rahulbsw/streamforge/issues)
- Check the [troubleshooting sections](docs/USAGE.md#troubleshooting)

### 2. Gather Information

When asking for help, please provide:

- **Version**: Run `cargo --version` and include Streamforge version
- **Operating System**: OS name and version
- **Configuration**: Relevant config (redact sensitive information)
- **Error Messages**: Complete error messages and stack traces
- **Steps to Reproduce**: Detailed steps to reproduce the issue
- **Expected vs Actual**: What you expected to happen vs what actually happened

### 3. Example Good Question

```
**Environment:**
- Streamforge version: 0.3.0
- OS: Ubuntu 22.04
- Rust: 1.70.0

**Issue:**
When running with multi-destination routing, messages are not being 
filtered correctly by the REGEX filter.

**Configuration:**
```yaml
routing:
  destinations:
    - output: validated-users
      filter: "REGEX:/email,^[\\w\\.-]+@[\\w\\.-]+\\.\\w+$"
```

**Error:**
[Error message here]

**Steps to Reproduce:**
1. Start Streamforge with config above
2. Send message with email field
3. Observe that message is not filtered
```

---

## Common Issues

### Installation Issues

See [Quick Start Guide - Prerequisites](docs/QUICKSTART.md#prerequisites)

### Configuration Issues

- [YAML Configuration Guide](docs/YAML_CONFIGURATION.md)
- [Configuration Examples](examples/README.md)
- [Advanced DSL Guide](docs/ADVANCED_DSL_GUIDE.md)

### Performance Issues

- [Performance Tuning Guide](docs/PERFORMANCE.md)
- [Scaling Guide](docs/SCALING.md)
- [Benchmark Results](benchmarks/results/)

### Security Configuration

- [Security Configuration Guide](docs/SECURITY_CONFIGURATION.md)
- [Docker Security](docs/DOCKER.md#security)
- [Kubernetes Security](docs/KUBERNETES.md#security)

### Deployment Issues

- [Docker Deployment](docs/DOCKER.md)
- [Kubernetes Deployment](docs/KUBERNETES.md)

---

## Contributing

Interested in contributing? That's great! See our [Contributing Guide](docs/CONTRIBUTING.md) to get started.

We welcome:
- 🐛 Bug reports
- 💡 Feature requests
- 📖 Documentation improvements
- 🔧 Code contributions
- ⚡ Performance improvements

---

## Response Times

| Support Channel | Expected Response Time |
|----------------|----------------------|
| Security Issues | 24-48 hours |
| Bug Reports | 2-5 business days |
| Feature Requests | 1-2 weeks |
| Questions | 2-7 days |
| Pull Requests | 1-2 weeks |

*Note: These are estimates. Response times may vary depending on complexity and maintainer availability.*

---

## Commercial Support

For commercial support, consulting, or custom development:
- **Email**: rahul.oracle.db@gmail.com
- **Subject**: "Commercial Support Inquiry - Streamforge"

We can help with:
- Enterprise deployment and configuration
- Custom feature development
- Performance optimization
- Training and workshops
- SLA-backed support contracts

---

## Code of Conduct

Please note that all interactions are governed by our [Code of Conduct](CODE_OF_CONDUCT.md). We're committed to providing a welcoming and inclusive environment for everyone.

---

## Resources

### Official Documentation
- [GitHub Repository](https://github.com/rahulbsw/streamforge)
- [Documentation Site](docs/)
- [Changelog](docs/CHANGELOG.md)

### Related Projects
- [Apache Kafka](https://kafka.apache.org/)
- [rdkafka-rust](https://github.com/fede1024/rust-rdkafka)

---

**Thank you for using Streamforge!** 🚀
