---
title: Quickstart
nav_order: 2
---

# Quick Start Guide

## Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install required system dependencies (macOS)
brew install cmake pkg-config openssl

# For Linux
# apt-get install cmake pkg-config libssl-dev libsasl2-dev
```

## Build & Test

```bash
# Clone and navigate to project
git clone https://github.com/rahulbsw/streamforge.git
cd streamforge

# Build
cargo build --release

# Run tests
cargo test

# Check for issues
cargo clippy
```

## Running Locally with Docker Kafka

### 1. Start Kafka (Docker Compose)

Create `docker-compose.yml`:

```yaml
version: '3'
services:
  zookeeper:
    image: confluentinc/cp-zookeeper:7.5.0
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181

  kafka:
    image: confluentinc/cp-kafka:7.5.0
    depends_on:
      - zookeeper
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
```

```bash
docker-compose up -d
```

### 2. Create Topics

```bash
# Create input topic
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic input-topic \
  --partitions 3

# Create output topic
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic output-topic \
  --partitions 3
```

### 3. Configure MirrorMaker

Create `config.json`:

```json
{
  "appid": "streamforge-local",
  "bootstrap": "localhost:9092",
  "input": "input-topic",
  "output": "output-topic",
  "offset": "latest",
  "threads": 4,
  "compression": {
    "compression_type": "raw",
    "compression_algo": "gzip"
  }
}
```

### 4. Run MirrorMaker

```bash
RUST_LOG=info CONFIG_FILE=config.json \
  ./target/release/streamforge
```

### 5. Send Test Messages

In another terminal:

```bash
# Producer
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-console-producer \
  --bootstrap-server localhost:9092 \
  --topic input-topic

# Type messages (JSON):
{"message": "hello", "confId": 123}
{"message": "world", "confId": 456}
```

### 6. Verify Output

In another terminal:

```bash
# Consumer
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic output-topic \
  --from-beginning
```

You should see your messages appear!

## Multi-Destination Routing Example

### 1. Create Multiple Output Topics

```bash
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic meeting-events \
  --partitions 3

docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic quality-events \
  --partitions 3
```

### 2. Configure Multi-Destination Routing

Create `config-routing.yaml`:

```yaml
appid: streamforge-routing
bootstrap: localhost:9092
input: events
offset: latest
threads: 4

routing:
  destinations:
    - output: meeting-events
      filter: 'msg["eventType"] == "meeting"'
      partition: '/confId'
    
    - output: quality-events
      filter: 'msg["eventType"] == "quality"'
      partition: '/siteId'
```

### 3. Run with Routing

```bash
RUST_LOG=info CONFIG_FILE=config-routing.json \
  ./target/release/streamforge
```

### 4. Send Different Event Types

```bash
# Create events topic
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic events \
  --partitions 3

# Produce events
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-console-producer \
  --bootstrap-server localhost:9092 \
  --topic events

# Send meeting event
{"eventType": "meeting", "confId": 123, "data": "meeting started"}

# Send quality event
{"eventType": "quality", "siteId": 456, "data": "quality report"}
```

### 5. Verify Routing

```bash
# Check meeting-events topic
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic meeting-events \
  --from-beginning

# Check quality-events topic
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-console-consumer \
  --bootstrap-server localhost:9092 \
  --topic quality-events \
  --from-beginning
```

## Cross-Cluster Mirroring

### Setup

```yaml
# docker-compose-two-clusters.yml
version: '3'
services:
  # Source cluster
  source-zookeeper:
    image: confluentinc/cp-zookeeper:7.5.0
    environment:
      ZOOKEEPER_CLIENT_PORT: 2181

  source-kafka:
    image: confluentinc/cp-kafka:7.5.0
    depends_on:
      - source-zookeeper
    ports:
      - "9092:9092"
    environment:
      KAFKA_BROKER_ID: 1
      KAFKA_ZOOKEEPER_CONNECT: source-zookeeper:2181
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9092
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1

  # Target cluster
  target-zookeeper:
    image: confluentinc/cp-zookeeper:7.5.0
    environment:
      ZOOKEEPER_CLIENT_PORT: 2182

  target-kafka:
    image: confluentinc/cp-kafka:7.5.0
    depends_on:
      - target-zookeeper
    ports:
      - "9093:9093"
    environment:
      KAFKA_BROKER_ID: 2
      KAFKA_ZOOKEEPER_CONNECT: target-zookeeper:2182
      KAFKA_ADVERTISED_LISTENERS: PLAINTEXT://localhost:9093
      KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR: 1
```

### Configuration

```json
{
  "appid": "cross-cluster-mirror",
  "bootstrap": "localhost:9092",
  "input": "source-topic",
  "output": "mirrored-topic",
  "target_broker": "localhost:9093",
  "offset": "earliest",
  "compression": {
    "compression_type": "raw",
    "compression_algo": "zstd"
  }
}
```

## Monitoring & Debugging

### Enable Debug Logging

```bash
# All modules
RUST_LOG=debug ./target/release/streamforge

# Specific module
RUST_LOG=streamforge::kafka::sink=trace \
  ./target/release/streamforge

# Multiple modules
RUST_LOG=streamforge::kafka=debug,streamforge::processor=info \
  ./target/release/streamforge
```

### Metrics

Watch metrics output:
```
[2024-03-09T10:15:30Z INFO  streamforge] Stats: processed=1000 (100.0/s),
filtered=0 (0.0/s), transformed=0 (0.0/s), completed=1000 (100.0/s), errors=0 (0.0/s)
```

### Common Issues

#### 1. Connection Refused

```
Error: Kafka error: BrokerTransportFailure
```

**Fix:** Check Kafka is running:
```bash
docker ps | grep kafka
```

#### 2. Topic Not Found

```
Error: Kafka error: UnknownTopicOrPartition
```

**Fix:** Create the topic:
```bash
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-topics --create \
  --bootstrap-server localhost:9092 \
  --topic your-topic
```

#### 3. Consumer Group Lag

```bash
# Check consumer lag
docker exec -it $(docker ps -qf "name=kafka") \
  kafka-consumer-groups \
  --bootstrap-server localhost:9092 \
  --group streamforge \
  --describe
```

## Performance Testing

### Load Testing Script

```bash
#!/bin/bash
# load-test.sh

TOPIC="input-topic"
BOOTSTRAP="localhost:9092"
MESSAGES=10000
BATCH_SIZE=100

for i in $(seq 1 $MESSAGES); do
  echo "{\"id\": $i, \"timestamp\": $(date +%s), \"data\": \"test message $i\"}"
done | docker exec -i $(docker ps -qf "name=kafka") \
  kafka-console-producer \
  --bootstrap-server $BOOTSTRAP \
  --topic $TOPIC \
  --batch-size $BATCH_SIZE
```

### Benchmark Results

```bash
# Run load test
chmod +x load-test.sh
./load-test.sh

# Monitor metrics
watch -n 1 "docker logs -f mirrormaker-container | tail -1"
```

## Deployment

### Docker Container

Create `Dockerfile`:

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/streamforge /usr/local/bin/
CMD ["streamforge"]
```

Build and run:

```bash
docker build -t streamforge .
docker run -e CONFIG_FILE=/config/config.json \
  -v $(pwd)/config.json:/config/config.json \
  streamforge
```

## Next Steps

1. Review [IMPLEMENTATION_NOTES.md](IMPLEMENTATION_NOTES.md) for architecture details
2. Check [README.md](../README.md) for feature documentation
3. Run side-by-side with Java for validation
