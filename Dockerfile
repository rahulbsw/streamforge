# Multi-stage Dockerfile with Debian bookworm
# Stage 1: Builder - Compile the Rust application
FROM rust:1.85-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    libsasl2-dev \
    libssl-dev \
    libzstd-dev \
    pkg-config \
    cmake \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Copy dependency manifests first (for layer caching)
COPY Cargo.toml Cargo.lock ./
COPY benches ./benches

# Create a dummy main.rs to cache dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --bin streamforge && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the real application
# Touch main.rs to force rebuild after dummy
RUN touch src/main.rs && \
    cargo build --release --locked --bin streamforge

# Stage 2: Runtime - Minimal Debian runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libsasl2-2 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 65532 nonroot

# Copy the compiled binary from builder stage
COPY --from=builder /build/target/release/streamforge /usr/local/bin/streamforge

USER nonroot
WORKDIR /home/nonroot

# Set environment variables
ENV RUST_LOG=info

# Run the application
ENTRYPOINT ["/usr/local/bin/streamforge"]
