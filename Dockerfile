# Multi-stage build using Chainguard hardened images
# Stage 1: Build
FROM cgr.dev/chainguard/rust:latest-dev AS builder

USER root

# Install build dependencies (Wolfi/Alpine package names)
RUN apk add --no-cache \
    cyrus-sasl-dev \
    openssl-dev \
    zstd-dev \
    curl-dev \
    cmake \
    clang \
    llvm-dev \
    pkgconf

WORKDIR /build

# Copy dependency manifests for layer caching
COPY Cargo.toml Cargo.lock ./
COPY benches ./benches

# Cache dependencies with dummy binary
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --bin streamforge && \
    rm -rf src

# Copy source and build
COPY src ./src
RUN touch src/main.rs && \
    cargo build --release --locked --bin streamforge

# Stage 2: Runtime
FROM cgr.dev/chainguard/rust:latest

LABEL org.opencontainers.image.source="https://github.com/rahulbsw/streamforge"
LABEL org.opencontainers.image.description="High-performance Kafka streaming toolkit"
LABEL org.opencontainers.image.licenses="Apache-2.0"

# Copy binary from builder
COPY --from=builder /build/target/release/streamforge /usr/local/bin/streamforge

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/streamforge"]
