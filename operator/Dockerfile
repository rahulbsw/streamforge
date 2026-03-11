# Multi-stage build for Streamforge Operator
FROM rust:1.75 AS builder

WORKDIR /build

# Copy manifests
COPY operator/Cargo.toml operator/Cargo.lock ./

# Cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source
COPY operator/src ./src

# Build operator
RUN touch src/main.rs && \
    cargo build --release

# Runtime stage
FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/streamforge-operator /usr/local/bin/streamforge-operator

USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/streamforge-operator"]
