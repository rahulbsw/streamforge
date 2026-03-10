# Docker Deployment Guide

## Overview

Two Dockerfile options are provided:

1. **`Dockerfile`** - Dynamic linking (recommended for most use cases)
   - Runtime: `cgr.dev/chainguard/glibc-dynamic`
   - Size: ~20-30MB
   - Includes necessary shared libraries

2. **`Dockerfile.static`** - Fully static binary (maximum security)
   - Runtime: `cgr.dev/chainguard/static`
   - Size: ~10-15MB
   - No dependencies, ultra-minimal

## Why Chainguard Images?

- ✅ **Minimal attack surface** - Only essential components
- ✅ **Daily updates** - Automatic CVE patching
- ✅ **Non-root by default** - Enhanced security
- ✅ **SBOM included** - Software Bill of Materials
- ✅ **Signed with Sigstore** - Supply chain security
- ✅ **No CVEs** - Zero known vulnerabilities

## Quick Start

### 1. Build the Image

**Dynamic version (recommended):**
```bash
docker build -t wap-mirrormaker-rust:latest .
```

**Static version:**
```bash
docker build -f Dockerfile.static -t wap-mirrormaker-rust:static .
```

### 2. Create Configuration

```bash
# Copy example config
cp config.example.json config.json

# Edit for your environment
vim config.json
```

### 3. Run the Container

```bash
docker run -d \
  --name wap-mirrormaker \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  -e RUST_LOG=info \
  --restart unless-stopped \
  wap-mirrormaker-rust:latest
```

### 4. Check Logs

```bash
docker logs -f wap-mirrormaker
```

## Docker Compose

### Basic Usage

```bash
# Start with your config
docker-compose up -d

# View logs
docker-compose logs -f mirrormaker

# Stop
docker-compose down
```

### With Local Kafka (for testing)

```bash
# Start Kafka + MirrorMaker
docker-compose --profile kafka up -d

# Check all services
docker-compose --profile kafka ps
```

### Static Version

```bash
# Use the static build
docker-compose --profile static up -d mirrormaker-static
```

## Configuration Options

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CONFIG_FILE` | `/app/config/config.json` | Path to config file |
| `RUST_LOG` | `info` | Log level (trace, debug, info, warn, error) |

### Volume Mounts

```bash
docker run -d \
  --name wap-mirrormaker \
  -v $(pwd)/config.json:/app/config/config.json:ro \  # Config (read-only)
  -v $(pwd)/logs:/app/logs \                          # Logs (optional)
  wap-mirrormaker-rust:latest
```

### Network Modes

**Bridge mode (default):**
```bash
docker run --network bridge ...
```

**Host mode (for local Kafka):**
```bash
docker run --network host ...
```

**Custom network:**
```bash
docker network create kafka-network
docker run --network kafka-network ...
```

## Resource Limits

### Recommended Settings

```bash
docker run -d \
  --name wap-mirrormaker \
  --cpus="2" \
  --memory="512m" \
  --memory-reservation="256m" \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  wap-mirrormaker-rust:latest
```

### In docker-compose.yml

```yaml
deploy:
  resources:
    limits:
      cpus: '2'
      memory: 512M
    reservations:
      cpus: '1'
      memory: 256M
```

## Health Checks

### Built-in Health Check

The Dockerfile includes a health check:

```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep -f wap-mirrormaker-rust || exit 1
```

### Check Health Status

```bash
docker inspect --format='{{json .State.Health}}' wap-mirrormaker | jq
```

## Logging

### View Logs

```bash
# Follow logs
docker logs -f wap-mirrormaker

# Last 100 lines
docker logs --tail 100 wap-mirrormaker

# With timestamps
docker logs -f --timestamps wap-mirrormaker
```

### Structured Logging

Set `RUST_LOG` for different verbosity:

```bash
# Info level (default)
docker run -e RUST_LOG=info ...

# Debug level
docker run -e RUST_LOG=debug ...

# Module-specific
docker run -e RUST_LOG=wap_mirrormaker_rust::kafka=debug,wap_mirrormaker_rust::processor=trace ...
```

## Image Size Comparison

| Image | Size | Security | Use Case |
|-------|------|----------|----------|
| Dynamic | ~25MB | High | Production (recommended) |
| Static | ~12MB | Highest | Maximum security |
| Java equivalent | ~200MB+ | Medium | Legacy |

## Multi-Architecture Builds

### Build for ARM64

```bash
docker buildx build \
  --platform linux/arm64 \
  -t wap-mirrormaker-rust:arm64 \
  .
```

### Multi-arch Manifest

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t wap-mirrormaker-rust:latest \
  --push \
  .
```

## Security Best Practices

### 1. Run as Non-Root ✅

Both Dockerfiles use non-root user by default.

```bash
# Verify
docker run --rm wap-mirrormaker-rust:latest id
# Should show: uid=65532(nonroot) gid=65532(nonroot)
```

### 2. Read-Only Root Filesystem

```bash
docker run -d \
  --read-only \
  --tmpfs /tmp \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  wap-mirrormaker-rust:latest
```

### 3. Drop Capabilities

```bash
docker run -d \
  --cap-drop=ALL \
  --security-opt=no-new-privileges:true \
  wap-mirrormaker-rust:latest
```

### 4. Complete Secure Configuration

```bash
docker run -d \
  --name wap-mirrormaker-secure \
  --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=10m \
  --cap-drop=ALL \
  --security-opt=no-new-privileges:true \
  --cpus="2" \
  --memory="512m" \
  --pids-limit=100 \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  wap-mirrormaker-rust:latest
```

## Kubernetes Deployment

### Basic Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: wap-mirrormaker
spec:
  replicas: 3
  selector:
    matchLabels:
      app: wap-mirrormaker
  template:
    metadata:
      labels:
        app: wap-mirrormaker
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 65532
        fsGroup: 65532
      containers:
      - name: mirrormaker
        image: wap-mirrormaker-rust:latest
        imagePullPolicy: Always
        env:
        - name: CONFIG_FILE
          value: /app/config/config.json
        - name: RUST_LOG
          value: info
        resources:
          requests:
            memory: "256Mi"
            cpu: "500m"
          limits:
            memory: "512Mi"
            cpu: "2000m"
        volumeMounts:
        - name: config
          mountPath: /app/config
          readOnly: true
        securityContext:
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
      volumes:
      - name: config
        configMap:
          name: mirrormaker-config
```

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: mirrormaker-config
data:
  config.json: |
    {
      "appid": "wap-mirrormaker",
      "bootstrap": "kafka-broker:9092",
      "input": "source-topic",
      "output": "destination-topic",
      "offset": "latest",
      "threads": 4
    }
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker logs wap-mirrormaker

# Run interactively
docker run --rm -it \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  wap-mirrormaker-rust:latest
```

### Config Validation

```bash
# Test config file
docker run --rm \
  -v $(pwd)/config.json:/app/config/config.json:ro \
  wap-mirrormaker-rust:latest --help
```

### Network Issues

```bash
# Test connectivity to Kafka
docker run --rm --network host nicolaka/netshoot \
  nc -zv kafka-broker 9092
```

### Permission Issues

```bash
# Check file permissions
ls -l config.json

# Should be readable by all
chmod 644 config.json
```

## Performance Monitoring

### Container Stats

```bash
docker stats wap-mirrormaker
```

### Resource Usage

```bash
# CPU and memory
docker inspect wap-mirrormaker | jq '.[0].HostConfig.Memory'

# Current usage
docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" wap-mirrormaker
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Build and Push Docker Image

on:
  push:
    branches: [ main ]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3

    - name: Build Docker image
      run: docker build -t wap-mirrormaker-rust:${{ github.sha }} .

    - name: Run tests
      run: docker run --rm wap-mirrormaker-rust:${{ github.sha }} cargo test

    - name: Push to registry
      run: |
        echo "${{ secrets.REGISTRY_PASSWORD }}" | docker login -u "${{ secrets.REGISTRY_USERNAME }}" --password-stdin
        docker push wap-mirrormaker-rust:${{ github.sha }}
```

## Best Practices Summary

✅ Use Chainguard base images for security
✅ Multi-stage builds to minimize size
✅ Run as non-root user (uid 65532)
✅ Mount config as read-only
✅ Set resource limits
✅ Use health checks
✅ Enable structured logging
✅ Read-only root filesystem
✅ Drop all capabilities
✅ Regular image updates

## Image Registry

### Push to Registry

```bash
# Tag
docker tag wap-mirrormaker-rust:latest your-registry.com/wap-mirrormaker-rust:latest

# Push
docker push your-registry.com/wap-mirrormaker-rust:latest
```

### Pull from Registry

```bash
docker pull your-registry.com/wap-mirrormaker-rust:latest
```

## Questions?

See:
- `README.md` - Application overview
- `QUICKSTART.md` - Getting started
- `IMPLEMENTATION_NOTES.md` - Architecture details
