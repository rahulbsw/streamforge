# Multi-stage Dockerfile using Chainguard base images
# Stage 1: Builder - Compile the Rust application
FROM cgr.dev/chainguard/rust:latest AS builder

# Install build dependencies
USER root
RUN apk add --no-cache \
    cmake \
    openssl-dev \
    cyrus-sasl-dev \
    zstd-dev \
    lz4-dev \
    build-base \
    pkgconfig

# Set working directory
WORKDIR /build

# Copy dependency manifests first (for layer caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the real application
# Touch main.rs to force rebuild after dummy
RUN touch src/main.rs && \
    cargo build --release --locked

# Verify the binary was built
RUN ls -lh /build/target/release/wap-mirrormaker-rust

# Stage 2: Runtime - Minimal Chainguard runtime image
FROM cgr.dev/chainguard/glibc-dynamic:latest

# Install only runtime dependencies (no build tools)
USER root
RUN apk add --no-cache \
    libssl3 \
    cyrus-sasl \
    zstd-libs \
    lz4-libs \
    libgcc \
    libstdc++

# Create non-root user for running the application
RUN addgroup -g 65532 nonroot && \
    adduser -D -u 65532 -G nonroot nonroot

# Create directory for configuration
RUN mkdir -p /app/config && \
    chown -R nonroot:nonroot /app

# Copy the compiled binary from builder stage
COPY --from=builder --chown=nonroot:nonroot \
    /build/target/release/wap-mirrormaker-rust \
    /app/wap-mirrormaker-rust

# Copy example configurations (optional)
COPY --chown=nonroot:nonroot config*.example.json /app/config/

# Switch to non-root user
USER nonroot
WORKDIR /app

# Set environment variables
ENV CONFIG_FILE=/app/config/config.json
ENV RUST_LOG=info

# Expose no ports (Kafka client only)

# Health check (optional - checks if process is running)
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD pgrep -f wap-mirrormaker-rust || exit 1

# Run the application
ENTRYPOINT ["/app/wap-mirrormaker-rust"]
