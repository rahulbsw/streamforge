# Multi-stage build for Streamforge Operator
FROM rust:1.89-bookworm AS builder

WORKDIR /build

# Copy manifests
COPY operator/Cargo.toml operator/Cargo.lock ./operator/
COPY crates/streamforge-config-model/Cargo.toml ./crates/streamforge-config-model/Cargo.toml
COPY crates/streamforge-config-model/src ./crates/streamforge-config-model/src

# Cache dependencies
RUN mkdir operator/src && \
    echo "fn main() {}" > operator/src/main.rs && \
    cargo build --release --locked --manifest-path operator/Cargo.toml && \
    rm -rf operator/src

# Copy source
COPY operator/src ./operator/src

# Build operator
RUN touch operator/src/main.rs && \
    cargo build --release --locked --manifest-path operator/Cargo.toml

# Runtime stage - Chainguard minimal image for zero CVEs
FROM cgr.dev/chainguard/glibc-dynamic:latest@sha256:57e5704e70a85b90191182eb6110d1c817df0d8e96035cb041195c5a351f0861

COPY --from=builder /build/operator/target/release/streamforge-operator /usr/local/bin/streamforge-operator

USER nonroot:nonroot

ENTRYPOINT ["/usr/local/bin/streamforge-operator"]
