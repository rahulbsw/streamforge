# syntax=docker/dockerfile:1

FROM docker.io/library/rockylinux@sha256:2d05a9266523bbf24f33ebc3a9832e4d5fd74b973c220f2204ca802286aa275d AS builder

ARG RUST_VERSION=1.89.0

RUN dnf install -y \
        clang \
        cmake \
        curl \
        cyrus-sasl-devel \
        gcc \
        gcc-c++ \
        git \
        libcurl-devel \
        libzstd-devel \
        llvm-devel \
        make \
        openssl-devel \
        perl \
        pkgconf-pkg-config \
    && dnf clean all

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain "${RUST_VERSION}"

ENV PATH=/root/.cargo/bin:${PATH}
WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY benches ./benches
RUN mkdir -p src \
    && printf '%s\n' 'fn main() {}' > src/main.rs \
    && cargo build --release --locked --bin streamforge \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs \
    && cargo build --release --locked --bin streamforge

FROM docker.io/confluentinc/cp-kafka@sha256:fbbb6fa11b258a88b83f54d4f0bddfcffbf2279f99d66a843486e3da7bdfbf41

USER root

RUN yum install -y \
        curl \
        cyrus-sasl-lib \
        findutils \
        libcurl \
        openssl-libs \
        procps-ng \
        python3 \
        unzip \
    && yum clean all \
    && curl --proto '=https' --tlsv1.2 -sSf \
        https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip \
        -o /tmp/awscliv2.zip \
    && unzip -q /tmp/awscliv2.zip -d /tmp \
    && /tmp/aws/install \
    && rm -rf /tmp/aws /tmp/awscliv2.zip

WORKDIR /opt/streamforge

COPY --from=builder /build/target/release/streamforge \
    /opt/streamforge/target/release/streamforge
COPY scripts/benchmarks /opt/streamforge/scripts/benchmarks

RUN chmod 0555 \
        /opt/streamforge/target/release/streamforge \
        /opt/streamforge/scripts/benchmarks/*.py \
        /opt/streamforge/scripts/benchmarks/*.sh \
        /opt/streamforge/scripts/benchmarks/aws/run_ecs_benchmark.sh \
    && mkdir -p /results \
    && chmod 0777 /results

ENV BENCHMARK_EXECUTION_MODE=direct
ENV BENCHMARK_KAFKA_BOOTSTRAP=127.0.0.1:9092
ENV BENCHMARK_KAFKA_DATA_DIR=/var/lib/kafka/data
ENV BENCHMARK_KAFKA_IMAGE=docker.io/confluentinc/cp-kafka@sha256:fbbb6fa11b258a88b83f54d4f0bddfcffbf2279f99d66a843486e3da7bdfbf41
ENV BENCHMARK_RUST_VERSION="rustc 1.89.0 (29483883e 2025-08-04)"
ENV BENCHMARK_RESULTS_ROOT=/results
ENV CONTAINER_RUNTIME=ecs
ENV KAFKA_CONTAINER=ecs-task-kafka
ENV INGRESS_CONTAINER=ecs-task-ingress
ENV OUTPUT_CONTAINER=ecs-task-output
ENV RUST_LOG=info

USER appuser

ENTRYPOINT ["/opt/streamforge/scripts/benchmarks/aws/run_ecs_benchmark.sh"]
