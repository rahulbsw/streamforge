# StreamForge v1.0 Stability Guarantees

**Version:** 1.0.0  
**Effective Date:** 2026-04-18  
**Stability Promise:** LTS (Long-Term Support)

This document defines the stability guarantees, backward compatibility promises, and support policies for StreamForge v1.x.

---

## Table of Contents

1. [Semantic Versioning](#semantic-versioning)
2. [Stable APIs](#stable-apis)
3. [Backward Compatibility](#backward-compatibility)
4. [Deprecation Policy](#deprecation-policy)
5. [Support Timeline](#support-timeline)
6. [Upgrade Path](#upgrade-path)
7. [Breaking Changes Policy](#breaking-changes-policy)

---

## Semantic Versioning

StreamForge follows [Semantic Versioning 2.0.0](https://semver.org/):

**Version format:** `MAJOR.MINOR.PATCH`

- **MAJOR (1.x.x):** Breaking changes, incompatible API changes
- **MINOR (x.1.x):** New features, backward compatible
- **PATCH (x.x.1):** Bug fixes, backward compatible

### Examples

| Change | Version Bump |
|--------|--------------|
| Fix producer timeout bug | 1.0.0 → 1.0.1 (PATCH) |
| Add new DSL operator | 1.0.0 → 1.1.0 (MINOR) |
| Remove deprecated KEY_SUFFIX | 1.0.0 → 2.0.0 (MAJOR) |
| Add Redis cache backend | 1.0.0 → 1.1.0 (MINOR) |
| Change config file format | 1.0.0 → 2.0.0 (MAJOR) |

### Release Cadence

- **Patch releases:** As needed (bug fixes, security)
- **Minor releases:** Every 3-6 months (new features)
- **Major releases:** Every 12-18 months (breaking changes)

---

## Stable APIs

The following interfaces are **stable** in v1.x and will not change in a backward-incompatible way:

### 1. Configuration Format (config.yaml)

**Stable fields:**
```yaml
appid: string
bootstrap: string
input: string
offset: "earliest" | "latest" | "stored"
threads: integer
commit_strategy: "auto" | "manual" | "per-message" | "time-based"
commit_interval_ms: integer

performance:
  fetch_min_bytes: integer
  fetch_max_wait_ms: integer
  max_partition_fetch_bytes: integer
  queue_buffering_max_ms: integer
  batch_size: integer
  linger_ms: integer
  compression: "none" | "gzip" | "snappy" | "lz4" | "zstd"

retry:
  max_attempts: integer
  initial_delay_ms: integer
  max_delay_ms: integer
  jitter: boolean

dlq:
  enabled: boolean
  topic: string
  include_error_headers: boolean
  max_retries: integer

routing:
  routing_type: "filter" | "fanout" | "passthrough"
  destinations:
    - output: string
      bootstrap: string (optional)
      filter: string
      transform: string
      key_transform: string
      partitioning: "default" | "random" | "hash" | "field"
      partition_field: string
      preserve_timestamp: boolean
      headers: map<string, string>

metrics:
  enabled: boolean
  port: integer
  path: string

cache:
  enabled: boolean
  type: "local" | "redis"
  ttl_seconds: integer
  max_entries: integer
  redis_url: string
```

**Guarantees:**
- All v1.0 config files will work in v1.x
- New fields may be added (with defaults)
- Existing fields will not be removed or renamed
- Field types will not change
- Default values will not change in breaking ways

### 2. DSL Syntax

**Stable operators:**

**Filters:**
- JSON path: `/path,==,value`, `/path,>,value`, `/path,<,value`, `/path,!=,value`
- Composite: `AND:...`, `OR:...`, `NOT:...`
- Regex: `REGEX:/path,pattern`
- Array: `ARRAY_CONTAINS:...`, `ARRAY_ANY:...`, `ARRAY_ALL:...`, `ARRAY_LENGTH:...`
- Envelope: `KEY_PREFIX:prefix`, `KEY_MATCHES:regex`, `HEADER:name,==,value`, `TIMESTAMP_AGE:<,seconds`
- Existence: `EXISTS:/path`, `NOT_EXISTS:/path`

**Transforms:**
- JSON path: `/path` (extraction)
- EXTRACT: `EXTRACT:/path,target_field,default_value`
- CONSTRUCT: `CONSTRUCT:field1=/path1:field2=/path2`
- Hash: `HASH:MD5,/path,target_field`, `HASH:SHA256,/path,target_field`
- String: `UPPERCASE:/path`, `LOWERCASE:/path`, `TRIM:/path`
- Array: `ARRAY_MAP:/array,/field,target`, `ARRAY_FILTER:/array,/field,op,value`
- Arithmetic: `ADD:/field,value`, `MULTIPLY:/field,value`

**Guarantees:**
- All v1.0 DSL expressions will work in v1.x
- New operators may be added
- Existing operators will not be removed or changed
- Syntax will not change (colon-delimited format stable)

### 3. Prometheus Metrics

**Stable metrics:**
```promql
# Counters
streamforge_messages_consumed_total
streamforge_messages_produced_total
streamforge_errors_total{error_type}
streamforge_dlq_messages_total
streamforge_retries_total

# Gauges
streamforge_consumer_lag
up

# Histograms
streamforge_processing_duration_seconds
```

**Guarantees:**
- Metric names will not change
- Metric types will not change (counter, gauge, histogram)
- Labels will not be removed
- New metrics may be added
- New labels may be added

### 4. DLQ Message Headers

**Stable headers:**
```
x-streamforge-error-type: string
x-streamforge-source-topic: string
x-streamforge-source-partition: integer
x-streamforge-source-offset: integer
x-streamforge-filter: string (if filter error)
x-streamforge-transform: string (if transform error)
x-streamforge-retry-attempts: integer
```

**Guarantees:**
- Header names will not change
- Header formats will not change
- New headers may be added

### 5. CLI Interface

**Stable commands:**
```bash
streamforge --config config.yaml [--log-level LEVEL]
streamforge-validate config.yaml [--verbose] [--fail-on-warnings]
```

**Guarantees:**
- Command syntax will not change
- Flags will not be removed
- New flags may be added (with defaults)
- Exit codes will remain stable (0 = success, 1 = error, 2 = warnings)

### 6. Kubernetes CRD (StreamforgePipeline)

**Stable API version:** `streamforge.io/v1alpha1`

**Note:** CRD is in alpha (v1alpha1) and subject to change in v1.x. Will stabilize in v2.0.

---

## Backward Compatibility

### What We Promise

**Within v1.x (1.0.0 → 1.x.x):**
- ✅ All v1.0.0 config files will work without modification
- ✅ All v1.0.0 DSL expressions will work without modification
- ✅ All v1.0.0 metrics will be present
- ✅ DLQ message headers will be compatible
- ✅ CLI flags will remain valid
- ✅ Docker images will be drop-in replacements
- ✅ Kubernetes manifests will work (API version stable)

**Examples of compatible changes (MINOR version):**
- Add new config field with default value
- Add new DSL operator
- Add new metric
- Add new CLI flag
- Add new DLQ header
- Improve error messages
- Optimize performance
- Fix bugs

**Examples of breaking changes (MAJOR version):**
- Remove config field
- Rename config field
- Change DSL syntax
- Remove DSL operator
- Change metric name or type
- Remove CLI flag
- Change DLQ header format

### What We Don't Promise

**Not guaranteed to be stable:**
- Internal APIs (not exposed in config/DSL)
- Exact log message formats (may improve)
- Performance characteristics (may improve)
- Undocumented features
- Experimental features (marked as such)
- Kubernetes CRD (v1alpha1 is unstable)

### Testing Compatibility

We run backward compatibility tests for all releases:

1. **Config compatibility:** v1.0 configs tested against v1.x
2. **DSL compatibility:** v1.0 DSL expressions tested against v1.x
3. **Metric compatibility:** v1.0 metrics present in v1.x
4. **Upgrade tests:** v1.0 → v1.x upgrades tested in CI/CD

---

## Deprecation Policy

### Deprecation Process

**1. Announce deprecation (in MINOR release):**
   - Mark feature as deprecated in docs
   - Add deprecation warning in CLI/logs
   - Provide migration guide
   - Minimum deprecation period: 6 months or 2 minor versions

**2. Remove feature (in MAJOR release):**
   - Feature removed in next major version
   - Migration guide updated
   - Release notes include removal notice

### Current Deprecations

#### DSL Syntax (Deprecated in v1.0)

**`KEY_SUFFIX:suffix`** → Use `KEY_MATCHES:.*suffix$`
- **Deprecated:** v1.0.0 (2026-04-18)
- **Removal:** v2.0.0 (estimated 2027-10-18)
- **Migration:** `streamforge-validate` will warn

**`KEY_CONTAINS:substring`** → Use `KEY_MATCHES:.*substring.*`
- **Deprecated:** v1.0.0 (2026-04-18)
- **Removal:** v2.0.0 (estimated 2027-10-18)
- **Migration:** `streamforge-validate` will warn

### How to Check for Deprecations

```bash
# Validate config and check for deprecations
streamforge-validate config.yaml

# Fail on deprecation warnings (CI/CD)
streamforge-validate config.yaml --fail-on-warnings
```

---

## Support Timeline

### Long-Term Support (LTS)

**v1.x is LTS:** Supported until v3.0 release

| Version | Release Date | End of Support | Support Period |
|---------|--------------|----------------|----------------|
| v1.0.x  | 2026-04-18   | v3.0 release   | 24+ months |
| v2.0.x  | TBD (~2027-10) | v4.0 release | 24+ months |

### Support Levels

**Full Support (Latest MINOR version):**
- Bug fixes
- Security patches
- New features
- Performance improvements

**Security Support Only (Previous MINOR versions):**
- Security patches only
- Critical bug fixes only
- No new features

**Example:**
- v1.2.x: Full support (latest minor)
- v1.1.x: Security support only
- v1.0.x: Security support only

### Security Patches

Security patches are backported to:
- Latest MINOR version (v1.x.x)
- Previous MINOR version (v1.(x-1).x)
- LTS versions (if applicable)

**Example:** If v1.3.0 is latest:
- Security patch → v1.3.1 (latest)
- Security patch → v1.2.5 (previous)
- Security patch → v1.1.10 (if critical)

---

## Upgrade Path

### Minor Version Upgrades (v1.x → v1.(x+1))

**Process:**
1. Read release notes
2. Test in dev/staging
3. Rolling upgrade in production (zero downtime)

**Expectations:**
- Zero downtime
- No config changes required
- No data migration
- Drop-in replacement

**Example:** v1.0.0 → v1.1.0
```bash
# Update image tag
kubectl set image deployment/streamforge streamforge=streamforge:1.1.0

# Rolling update (automatic)
kubectl rollout status deployment/streamforge
```

### Major Version Upgrades (v1.x → v2.x)

**Process:**
1. Read migration guide
2. Update deprecated syntax
3. Test thoroughly in dev/staging
4. Plan maintenance window (if needed)
5. Upgrade

**Expectations:**
- May require downtime
- Config changes may be required
- Migration steps may be needed
- Full regression testing recommended

**Example:** v1.x → v2.0
1. Check deprecations: `streamforge-validate config.yaml`
2. Update config as needed
3. Test with v2.0 in staging
4. Schedule maintenance window
5. Upgrade production

---

## Breaking Changes Policy

### When We Allow Breaking Changes

**Only in MAJOR versions (v1 → v2):**
- Remove deprecated features (after 6+ months)
- Change config format (with migration tool)
- Change DSL syntax (with migration guide)
- Remove or rename APIs
- Change fundamental behavior

### What We Require for Breaking Changes

1. **Deprecation period:** Minimum 6 months or 2 minor releases
2. **Migration guide:** Step-by-step instructions
3. **Migration tool:** If config format changes
4. **Clear communication:** Release notes, blog post, docs
5. **Justification:** Why change is necessary

### Breaking Change Checklist

Before approving a breaking change:
- [ ] Feature deprecated for 6+ months
- [ ] Migration guide written
- [ ] Migration tool provided (if needed)
- [ ] Backward compatibility impossible
- [ ] Benefit outweighs disruption
- [ ] Community feedback collected

---

## Exceptions and Escape Hatches

### Experimental Features

Features marked as "experimental" are **not subject to stability guarantees**:
- May change at any time (even in PATCH releases)
- May be removed without deprecation period
- Clearly marked in documentation and CLI output

**Current experimental features:**
- Kubernetes CRD (v1alpha1)
- Web UI (under active development)
- Redis cache backend (if added in v1.1)

### Security Vulnerabilities

Security fixes may require breaking changes in MINOR or PATCH releases:
- Documented as "security fix" in release notes
- Migration guide provided
- Alternative upgrade path if available
- Advance notice when possible (coordinated disclosure)

### Critical Bugs

Critical bugs that cause data loss may require breaking changes:
- Documented as "critical bug fix"
- Migration guide provided
- Announce on GitHub, mailing list, and docs

---

## Version Compatibility Matrix

### Kafka Compatibility

| StreamForge Version | Kafka Version | Status |
|---------------------|---------------|--------|
| v1.x                | 2.8+          | ✅ Supported |
| v1.x                | 3.x           | ✅ Supported |
| v1.x                | 2.0-2.7       | ⚠️ May work, not tested |
| v1.x                | < 2.0         | ❌ Not supported |

### Kubernetes Compatibility

| StreamForge Version | Kubernetes Version | Status |
|---------------------|--------------------|--------|
| v1.x                | 1.21+              | ✅ Supported |
| v1.x                | 1.19-1.20          | ⚠️ May work, not tested |
| v1.x                | < 1.19             | ❌ Not supported |

### Dependency Compatibility

| Dependency | v1.0 Version | Stability | Notes |
|------------|--------------|-----------|-------|
| rdkafka    | 0.36         | Stable    | Minor updates allowed |
| tokio      | 1.41         | Stable    | Patch updates automatic |
| serde_json | 1.0          | Stable    | Patch updates automatic |

---

## Communication Channels

### Release Announcements

- **GitHub Releases:** https://github.com/rahulbsw/streamforge/releases
- **CHANGELOG.md:** Full version history
- **Documentation:** Updated with each release

### Reporting Compatibility Issues

If you encounter a compatibility issue:

1. Check GitHub issues for existing reports
2. Create new issue with:
   - StreamForge versions (old and new)
   - Config file (redacted if needed)
   - Error message
   - Steps to reproduce
3. Tag with `compatibility` label

**GitHub Issues:** https://github.com/rahulbsw/streamforge/issues

---

## Commitment

**We commit to:**
- Honoring semantic versioning
- Maintaining backward compatibility within v1.x
- Providing deprecation warnings and migration guides
- Supporting LTS versions for 24+ months
- Clear communication about breaking changes
- Listening to community feedback on stability

**We ask users to:**
- Read release notes before upgrading
- Test upgrades in dev/staging first
- Report compatibility issues promptly
- Provide feedback on deprecations

---

## Revision History

| Date       | Version | Changes |
|------------|---------|---------|
| 2026-04-18 | 1.0     | Initial v1.0 stability guarantees |

---

**Document Version:** 1.0.0  
**Last Updated:** 2026-04-18  
**Effective For:** StreamForge v1.x  
**Contact:** https://github.com/rahulbsw/streamforge/issues
