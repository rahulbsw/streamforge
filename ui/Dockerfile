# Build from the repository root:
# docker build -f ui/Dockerfile .
FROM rust:1.89-bookworm AS validator-builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        clang \
        cmake \
        libclang-dev \
        libcurl4-openssl-dev \
        libsasl2-dev \
        libssl-dev \
        libzstd-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY benches ./benches
COPY crates/streamforge-config-model/Cargo.toml ./crates/streamforge-config-model/Cargo.toml
COPY crates/streamforge-config-model/src ./crates/streamforge-config-model/src
COPY src ./src
COPY wit ./wit
RUN cargo build --release --locked --bin streamforge-validate

FROM node:22-bookworm-slim AS base

# Install dependencies only when needed
FROM base AS deps
WORKDIR /app

COPY ui/package.json ui/package-lock.json* ./
RUN npm ci

# Rebuild the source code only when needed
FROM base AS builder
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/package.json /app/package-lock.json ./
COPY ui/app ./app
COPY ui/components ./components
COPY ui/lib ./lib
COPY ui/public ./public
COPY ui/eslint.config.mjs \
     ui/middleware.ts \
     ui/next.config.ts \
     ui/postcss.config.mjs \
     ui/tailwind.config.ts \
     ui/tsconfig.json \
     ./

RUN npm run build

# Production image, copy all the files and run next
FROM base AS runner
WORKDIR /app

ENV NODE_ENV=production

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libcurl4 \
        libsasl2-2 \
        libssl3 \
        libzstd1 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 1001 nodejs \
    && useradd --system --uid 1001 --gid nodejs nextjs

COPY --from=builder /app/public ./public

# Set the correct permission for prerender cache
RUN mkdir .next
RUN chown nextjs:nodejs .next

# Automatically leverage output traces to reduce image size
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

COPY --from=validator-builder --chmod=0555 /build/target/release/streamforge-validate /usr/local/bin/streamforge-validate
RUN /usr/local/bin/streamforge-validate --help >/dev/null

USER nextjs

EXPOSE 3001

ENV PORT=3001
ENV HOSTNAME="0.0.0.0"
ENV STREAMFORGE_VALIDATOR_PATH="/usr/local/bin/streamforge-validate"

CMD ["node", "server.js"]
