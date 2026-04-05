---
title: Contributing
nav_order: 12
---

# Contributing Guide

Thank you for considering contributing to StreamForge! This guide will help you get started.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Environment](#development-environment)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [Code Style](#code-style)
- [Adding Features](#adding-features)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)

## Getting Started

### Prerequisites

**Required:**
- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Cargo (comes with Rust)
- Git

**Optional:**
- Docker (for containerized testing)
- Kafka cluster (or use Docker Compose)
- IDE with Rust support (VS Code, IntelliJ IDEA, etc.)

### Quick Start

```bash
# Clone the repository
git clone <repository-url>
cd streamforge

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Run with example config
cargo run -- config.example.json
```

## Development Environment

### Local Setup

#### 1. Install Rust

```bash
# Install rustup (Rust installer)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add to PATH
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### 2. Install Development Tools

```bash
# Formatting
rustup component add rustfmt

# Linting
rustup component add clippy

# IDE support
cargo install rust-analyzer
```

#### 3. Clone and Build

```bash
# Clone repository
git clone <repository-url>
cd streamforge

# Build debug version
cargo build

# Build release version
cargo build --release

# Binary locations
./target/debug/streamforge      # Debug
./target/release/streamforge    # Release
```

### IDE Setup

#### Visual Studio Code

1. Install extensions:
   - rust-analyzer
   - CodeLLDB (for debugging)
   - Better TOML

2. Configure `.vscode/settings.json`:
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "editor.formatOnSave": true
}
```

3. Configure `.vscode/launch.json` for debugging:
```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug",
      "cargo": {
        "args": ["build", "--bin=streamforge"],
        "filter": {
          "name": "streamforge",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "CONFIG_FILE": "config.example.json",
        "RUST_LOG": "debug"
      }
    }
  ]
}
```

#### IntelliJ IDEA

1. Install "Rust" plugin
2. Open project
3. Configure run configuration:
   - Program arguments: `config.example.json`
   - Environment variables: `RUST_LOG=debug`

### Local Kafka Setup

#### Option 1: Docker Compose

```bash
# Start Kafka (included in project)
docker-compose --profile kafka up -d

# Verify
docker-compose ps

# Create test topic
docker exec -it kafka kafka-topics.sh \
  --bootstrap-server localhost:9092 \
  --create \
  --topic test-input \
  --partitions 3 \
  --replication-factor 1

# Stop
docker-compose down
```

#### Option 2: Manual Kafka

```bash
# Download Kafka
wget https://downloads.apache.org/kafka/3.6.0/kafka_2.13-3.6.0.tgz
tar -xzf kafka_2.13-3.6.0.tgz
cd kafka_2.13-3.6.0

# Start Zookeeper
bin/zookeeper-server-start.sh config/zookeeper.properties

# Start Kafka (in another terminal)
bin/kafka-server-start.sh config/server.properties

# Create topic
bin/kafka-topics.sh --bootstrap-server localhost:9092 \
  --create --topic test-input --partitions 3

# Produce test messages
bin/kafka-console-producer.sh \
  --bootstrap-server localhost:9092 \
  --topic test-input
```

## Project Structure

```
streamforge/
├── src/
│   ├── main.rs              # Application entry point
│   ├── lib.rs               # Library root
│   ├── error.rs             # Error types
│   ├── config.rs            # Configuration parsing
│   ├── filter.rs            # Filter implementations
│   ├── filter_parser.rs     # DSL parser
│   ├── compression.rs       # Compression algorithms
│   ├── partitioner.rs       # Partitioning strategies
│   ├── processor.rs         # Message processing
│   ├── metrics.rs           # Metrics collection
│   └── kafka/
│       ├── mod.rs           # Kafka module
│       └── sink.rs          # KafkaSink implementation
├── Cargo.toml               # Dependencies
├── Cargo.lock               # Dependency lock file
├── Dockerfile               # Dynamic binary image
├── Dockerfile.static        # Static binary image
├── docker-compose.yml       # Docker compose config
├── config*.json             # Example configurations
└── docs/
    ├── README.md            # Main documentation
    ├── USAGE.md             # Usage guide
    ├── PERFORMANCE.md       # Performance guide
    └── CONTRIBUTING.md      # This file
```

### Module Overview

**Core Modules:**
- `main.rs` - Application startup and configuration loading
- `lib.rs` - Public API and module organization
- `error.rs` - Error types and Result alias

**Kafka Integration:**
- `kafka/sink.rs` - Producer implementation, multi-destination routing
- `kafka/mod.rs` - Kafka module exports

**Filtering & Transformation:**
- `filter.rs` - Filter and Transform traits, implementations
- `filter_parser.rs` - DSL string parsing

**Processing:**
- `processor.rs` - Message routing and processing logic
- `compression.rs` - Message compression
- `partitioner.rs` - Partition assignment strategies
- `metrics.rs` - Performance metrics

## Development Workflow

### 1. Create a Branch

```bash
# Create feature branch
git checkout -b feature/my-new-feature

# Create bugfix branch
git checkout -b bugfix/issue-123
```

### 2. Make Changes

```bash
# Edit files
vim src/filter.rs

# Format code
cargo fmt

# Check for issues
cargo clippy

# Run tests
cargo test

# Build
cargo build
```

### 3. Test Locally

```bash
# Unit tests
cargo test --lib

# Integration tests
cargo test --test integration_tests

# Specific test
cargo test test_array_filter

# With output
cargo test -- --nocapture

# Run application
RUST_LOG=debug cargo run -- config.example.json
```

### 4. Commit Changes

```bash
# Stage changes
git add src/filter.rs

# Commit with descriptive message
git commit -m "Add array filter support for ARRAY_ALL and ARRAY_ANY"

# Push to remote
git push origin feature/my-new-feature
```

## Testing

### Running Tests

```bash
# All tests
cargo test

# Library tests only
cargo test --lib

# Specific module
cargo test filter::tests

# Specific test
cargo test test_array_filter_all_mode

# With output
cargo test -- --nocapture

# With logging
RUST_LOG=debug cargo test -- --nocapture
```

### Writing Tests

**Unit Tests:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_my_feature() {
        let filter = MyFilter::new("test");
        let msg = json!({"field": "value"});
        assert!(filter.evaluate(&msg).unwrap());
    }
}
```

**Integration Tests:**

Create file in `tests/`:

```rust
// tests/integration_test.rs
use streamforge::filter::*;
use serde_json::json;

#[test]
fn test_end_to_end() {
    // Test complete workflow
}
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --out Html

# View report
open tarpaulin-report.html
```

### Benchmarking

```bash
# Add to Cargo.toml
[dev-dependencies]
criterion = "0.5"

# Create benchmark file
# benches/filter_bench.rs

# Run benchmarks
cargo bench
```

Example benchmark:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use streamforge::filter::*;
use serde_json::json;

fn filter_benchmark(c: &mut Criterion) {
    let filter = JsonPathFilter::new("/message/siteId", ">", "10000").unwrap();
    let msg = json!({"message": {"siteId": 15000}});

    c.bench_function("simple filter", |b| {
        b.iter(|| filter.evaluate(black_box(&msg)))
    });
}

criterion_group!(benches, filter_benchmark);
criterion_main!(benches);
```

## Code Style

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting
cargo fmt -- --check

# Format specific file
rustfmt src/filter.rs
```

### Linting

```bash
# Run clippy
cargo clippy

# Strict mode
cargo clippy -- -D warnings

# Fix automatically (where possible)
cargo clippy --fix
```

### Style Guidelines

**Naming:**
- Types: `PascalCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

**Documentation:**
```rust
/// Short description.
///
/// Longer description with details.
///
/// # Examples
///
/// ```
/// let filter = MyFilter::new("test");
/// assert!(filter.is_valid());
/// ```
///
/// # Errors
///
/// Returns error if input is invalid.
pub fn my_function() -> Result<()> {
    // implementation
}
```

**Error Handling:**
```rust
// Use Result type
pub fn process() -> Result<Value> {
    let value = read_value()?;  // Use ? operator
    transform(value)
}

// Provide context
.map_err(|e| MirrorMakerError::Processing(
    format!("Failed to parse JSON: {}", e)
))?
```

**Testing:**
```rust
#[test]
fn test_feature() {
    // Arrange
    let input = create_test_input();

    // Act
    let result = process(input).unwrap();

    // Assert
    assert_eq!(result, expected);
}
```

## Adding Features

### Adding a New Filter

1. **Define the filter in `src/filter.rs`:**

```rust
/// My new filter description
pub struct MyFilter {
    field: String,
}

impl MyFilter {
    pub fn new(field: &str) -> Result<Self> {
        Ok(Self {
            field: field.to_string(),
        })
    }
}

impl Filter for MyFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        // Implementation
        Ok(true)
    }
}
```

2. **Add parser support in `src/filter_parser.rs`:**

```rust
pub fn parse_filter(expr: &str) -> Result<Arc<dyn Filter>> {
    // ...
    match parts[0] {
        // ...
        "MY_FILTER" => Ok(Arc::from(parse_my_filter(&parts[1..])?)),
        _ => // ...
    }
}

fn parse_my_filter(parts: &[&str]) -> Result<Box<dyn Filter>> {
    // Parse and return filter
    Ok(Box::new(MyFilter::new(parts[0])?))
}
```

3. **Add tests:**

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_my_filter() {
        let filter = MyFilter::new("test").unwrap();
        let msg = json!({"test": "value"});
        assert!(filter.evaluate(&msg).unwrap());
    }

    #[test]
    fn test_parse_my_filter() {
        let filter = parse_filter("MY_FILTER:arg").unwrap();
        let msg = json!({"field": "value"});
        assert!(filter.evaluate(&msg).unwrap());
    }
}
```

4. **Update documentation:**
   - Add to `ADVANCED_DSL_GUIDE.md`
   - Add examples
   - Update README.md

### Adding a New Transform

Similar process as filters, but implement `Transform` trait:

```rust
pub struct MyTransform {
    config: String,
}

impl Transform for MyTransform {
    fn transform(&self, value: Value) -> Result<Value> {
        // Implementation
        Ok(value)
    }
}
```

### Adding Dependencies

```bash
# Add dependency
cargo add serde_json

# Add dev dependency
cargo add --dev mockall

# Update Cargo.toml manually
# [dependencies]
# new_crate = "1.0"
```

## Documentation

### Code Documentation

```rust
/// Brief description.
///
/// More detailed explanation.
///
/// # Arguments
///
/// * `param` - Description
///
/// # Returns
///
/// Description of return value
///
/// # Errors
///
/// When this returns an error
///
/// # Examples
///
/// ```
/// let result = function(param);
/// assert_eq!(result, expected);
/// ```
pub fn function(param: Type) -> Result<Type> {
    // implementation
}
```

### Generate Documentation

```bash
# Generate docs
cargo doc

# Generate and open
cargo doc --open

# Include private items
cargo doc --document-private-items

# Check doc tests
cargo test --doc
```

### Documentation Files

Update relevant files:
- `README.md` - Overview and quick start
- `USAGE.md` - Use cases and examples
- `ADVANCED_DSL_GUIDE.md` - DSL reference
- `PERFORMANCE.md` - Performance tuning
- `CHANGELOG.md` - Version history

## Pull Request Process

### Before Submitting

**Checklist:**
- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New tests added for new features
- [ ] Code formatted with `cargo fmt`
- [ ] Linted with `cargo clippy`
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] No breaking changes (or documented)

### Submitting PR

1. **Push your branch:**
```bash
git push origin feature/my-feature
```

2. **Create Pull Request:**
   - Clear title describing the change
   - Detailed description of what and why
   - Reference any related issues
   - Include screenshots/examples if applicable

3. **PR Template:**
```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
How was this tested?

## Checklist
- [ ] Tests pass
- [ ] Documentation updated
- [ ] CHANGELOG updated
```

### Code Review

- Address reviewer comments
- Update based on feedback
- Keep discussion professional
- Ask questions if unclear

### After Merge

```bash
# Update local main
git checkout main
git pull origin main

# Delete feature branch
git branch -d feature/my-feature
git push origin --delete feature/my-feature
```

## Development Tips

### Debugging

```bash
# Debug logging
RUST_LOG=debug cargo run

# Specific module
RUST_LOG=streamforge::filter=trace cargo run

# With debugger (VS Code)
# Set breakpoints and press F5
```

### Performance Profiling

```bash
# Install perf tools
# Linux: apt-get install linux-tools-generic
# macOS: brew install flamegraph

# Profile
cargo build --release
perf record --call-graph dwarf ./target/release/streamforge

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > flamegraph.svg
```

### Common Issues

**Build Errors:**
```bash
# Clean and rebuild
cargo clean
cargo build

# Update dependencies
cargo update
```

**Test Failures:**
```bash
# Run specific failing test
cargo test failing_test -- --nocapture

# Check for data races
cargo test -- --test-threads=1
```

**Clippy Warnings:**
```bash
# Fix automatically
cargo clippy --fix

# Allow specific warning
#[allow(clippy::warning_name)]
```

## Getting Help

- Check existing documentation
- Search issues for similar problems
- Ask in pull request comments
- Contact maintainers

## License

Apache License 2.0 - See [LICENSE](../LICENSE) for details.

Copyright 2025 Rahul Jain

## Thank You!

Thank you for contributing to Streamforge! Your contributions help make this project better for everyone.
