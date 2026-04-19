# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Which versions are eligible for receiving such patches depends on the CVSS v3.0 Rating:

| Version | Supported          |
| ------- | ------------------ |
| 1.0.x   | :white_check_mark: |
| 0.4.x   | :white_check_mark: (LTS until 2026-10-18) |
| < 0.4   | :x:                |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you discover a security vulnerability, please send an email to **rahul.oracle.db@gmail.com**.

You should receive a response within 48 hours. If for some reason you do not, please follow up via email to ensure we received your original message.

Please include the following information (as much as you can provide) to help us better understand the nature and scope of the possible issue:

* Type of issue (e.g. buffer overflow, SQL injection, cross-site scripting, etc.)
* Full paths of source file(s) related to the manifestation of the issue
* The location of the affected source code (tag/branch/commit or direct URL)
* Any special configuration required to reproduce the issue
* Step-by-step instructions to reproduce the issue
* Proof-of-concept or exploit code (if possible)
* Impact of the issue, including how an attacker might exploit the issue

This information will help us triage your report more quickly.

## Preferred Languages

We prefer all communications to be in English.

## Disclosure Policy

When we receive a security bug report, we will:

1. Confirm the problem and determine the affected versions.
2. Audit code to find any potential similar problems.
3. Prepare fixes for all supported releases.
4. Release new security fix versions as soon as possible.

## Security Update Process

1. Security vulnerabilities will be fixed in the next patch release.
2. We will publish a security advisory on GitHub.
3. The CHANGELOG will be updated with security fix information.
4. We will notify users through:
   - GitHub Security Advisories
   - Release notes
   - README updates

## Bug Bounty

We do not currently have a bug bounty program. However, we deeply appreciate security researchers who responsibly disclose vulnerabilities to us and will publicly acknowledge their contribution (unless they prefer to remain anonymous).

## Security Best Practices

When using Streamforge in production:

### 1. Secure Configuration

✅ **Do:**
- Use SSL/TLS for Kafka connections
- Use SASL/SCRAM or mutual TLS for authentication
- Store credentials in environment variables or secret management systems
- Enable Kafka ACLs to restrict access

❌ **Don't:**
- Use PLAINTEXT protocol in production
- Store passwords in configuration files
- Commit credentials to version control
- Disable hostname verification

### 2. Network Security

- Run Streamforge in a private network/VPC
- Use firewall rules to restrict Kafka broker access
- Enable TLS for all Kafka connections
- Use VPN or private links for cross-region/cloud transfers

### 3. Container Security

- Use minimal base images (Chainguard recommended)
- Run as non-root user
- Scan images for vulnerabilities
- Keep dependencies up to date
- Use read-only filesystems where possible

### 4. Monitoring

- Monitor for authentication failures
- Set up alerts for unusual traffic patterns
- Log security events
- Track certificate expiration dates

### 5. Updates

- Subscribe to GitHub Security Advisories
- Keep Streamforge updated to latest patch version
- Review CHANGELOG for security fixes
- Test updates in staging before production

## Known Security Considerations

### Dependency Security

Streamforge uses the following security-sensitive dependencies:

- **rdkafka** - Kafka client library (includes SSL/SASL support)
- **tokio** - Async runtime
- **serde** - Serialization

We regularly update dependencies to address known vulnerabilities. Run:

```bash
cargo audit
```

To check for known vulnerabilities in dependencies.

### Configuration Security

- **Credentials in config files**: Never commit config files with credentials to version control
- **Environment variables**: Use environment variables for sensitive data
- **File permissions**: Set config files to 600 (owner read/write only)

Example secure configuration:

```yaml
security:
  protocol: SASL_SSL
  ssl:
    ca_location: /etc/streamforge/ca-cert.pem
  sasl:
    mechanism: SCRAM-SHA-256
    username: ${KAFKA_USERNAME}  # From environment
    password: ${KAFKA_PASSWORD}  # From environment
```

### Container Image Security

Our official Docker images:

- Use Chainguard base images (minimal, no CVEs)
- Run as non-root user (UID 65532)
- Don't include unnecessary tools
- Are regularly scanned for vulnerabilities

Verify image signatures:

```bash
docker pull rahulbsw/streamforge:latest
docker inspect rahulbsw/streamforge:latest
```

## Security Contacts

- **Primary**: rahul.oracle.db@gmail.com
- **GitHub**: @rahulbsw

## Hall of Fame

We thank the following security researchers for responsibly disclosing vulnerabilities:

(None yet - be the first!)

## Additional Resources

- [Kafka Security Documentation](https://kafka.apache.org/documentation/#security)
- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [Rust Security Advisory Database](https://rustsec.org/)
- [CIS Docker Benchmark](https://www.cisecurity.org/benchmark/docker)

---

**Last Updated**: 2026-04-18
**Version**: 1.0.0
