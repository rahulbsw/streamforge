# Multi-stage build using Chainguard hardened images
# Stage 1: Build
FROM cgr.dev/chainguard/rust:latest-dev@sha256:01ee5e47325bc99f25a253593c0dc5cf26183ecd2c5c543855856e0a7f4a3d26 AS builder

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
COPY crates/streamforge-config-model/Cargo.toml ./crates/streamforge-config-model/Cargo.toml
COPY crates/streamforge-config-model/src ./crates/streamforge-config-model/src

# Cache dependencies with dummy binary
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --bin streamforge && \
    rm -rf src

# Copy source and build
COPY src ./src
COPY wit ./wit
RUN touch src/main.rs && \
    cargo build --release --locked --bin streamforge

# Stage 2: Runtime
FROM cgr.dev/chainguard/rust:latest@sha256:9403ae8340434a1c2a18b41fd55850a61c7851e6189905efe9f0813bdc8f083d

LABEL org.opencontainers.image.source="https://github.com/rahulbsw/streamforge"
LABEL org.opencontainers.image.description="High-performance Kafka streaming toolkit"
LABEL org.opencontainers.image.licenses="Apache-2.0"

# Copy binary from builder
COPY --from=builder /build/target/release/streamforge /usr/local/bin/streamforge

ENV RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/streamforge"]
