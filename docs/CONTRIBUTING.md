---
title: Contributing
nav_order: 9
---

# Contributing

StreamForge welcomes code, documentation, tests, bug reports, feature proposals,
and community support. All participation is governed by the
[Code of Conduct](https://github.com/rahulbsw/streamforge/blob/main/CODE_OF_CONDUCT.md),
and all contributions are licensed under the
[Apache License 2.0](https://github.com/rahulbsw/streamforge/blob/main/LICENSE).

## Set up

Required: Git and the stable Rust toolchain. The UI additionally uses Node.js
20 and npm. Container, Kubernetes, and Helm work requires the corresponding
tools. The Linux packages used by CI are authoritative in
[the CI workflow](https://github.com/rahulbsw/streamforge/blob/main/.github/workflows/ci.yml).

```bash
git clone https://github.com/rahulbsw/streamforge.git
cd streamforge
rustup component add clippy rustfmt
cargo build --locked
cargo test --all --all-features
```

Run the engine with a YAML or JSON configuration through `CONFIG_FILE`:

```bash
CONFIG_FILE=examples/redpanda/selective-replication.yaml \
  cargo run --release --bin streamforge
```

Validate configuration without connecting to Kafka:

```bash
cargo run --quiet --bin streamforge-validate -- \
  examples/configs/config.example.yaml --fail-on-warnings
```

For a working local broker journey, use the [Quick Start](QUICKSTART.md).

## Choose the right source of truth

- Product scope: [PROJECT_SPEC.md](https://github.com/rahulbsw/streamforge/blob/main/PROJECT_SPEC.md)
- Architecture: [ARCHITECTURE.md](https://github.com/rahulbsw/streamforge/blob/main/ARCHITECTURE.md)
- Planned work: [ROADMAP.md](https://github.com/rahulbsw/streamforge/blob/main/ROADMAP.md)
- Verified implementation state:
  [IMPLEMENTATION_STATUS.md](https://github.com/rahulbsw/streamforge/blob/main/docs/IMPLEMENTATION_STATUS.md)
- Configuration: [YAML_CONFIGURATION.md](YAML_CONFIGURATION.md)
- Filters and transforms: [ADVANCED_DSL_GUIDE.md](ADVANCED_DSL_GUIDE.md)
- Delivery behavior: [DELIVERY_GUARANTEES.md](DELIVERY_GUARANTEES.md)
- Performance measurement: [PERFORMANCE.md](PERFORMANCE.md)

Update the relevant source of truth with behavior changes. Do not create a
second description of the same contract.

## Make a change

1. Fork the repository and branch from `main`; use a focused name such as
   `feature/...`, `bugfix/...`, or `docs/...`.
2. Make the smallest complete change. Add or update tests for changed behavior.
3. Update the canonical documentation and `CHANGELOG.md` when applicable.
4. Run the checks below.
5. Open a focused pull request and respond to review feedback.

Use conventional commit and PR-title types: `feat`, `fix`, `docs`, `style`,
`refactor`, `perf`, `test`, `build`, `ci`, `chore`, or `revert`. PR titles must
start their subject with an uppercase letter, for example
`fix(kafka): Preserve keyed partition ordering`.

New filters or transforms belong in the existing filter/transform and parser
flow, with parser and evaluation tests. New dependencies need a demonstrated
need and must pass dependency and security review.

## Verify

The following root checks mirror the current CI workflow:

```bash
cargo machete
cargo test --all --verbose
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo bench --no-run
```

CI also compiles the Rust WebAssembly examples, verifies their checked-in
digests, and validates promoted configurations:

```bash
rustup target add wasm32-unknown-unknown
cargo check --manifest-path udf-sdk/rust/Cargo.toml \
  --workspace --target wasm32-unknown-unknown --locked
(cd tests/fixtures/wasm/generated && sha256sum -c SHA256SUMS)

for config in \
  examples/configs/config.example.yaml \
  examples/redpanda/selective-replication.yaml \
  examples/production/pii-redaction.yaml \
  examples/production/cdc-to-datalake.yaml \
  tests/fixtures/wasm/release-smoke.yaml
do
  cargo run --quiet --bin streamforge-validate -- "$config"
done
```

Install `cargo-machete` or `cargo-audit` before running them if absent:

```bash
cargo install cargo-machete --locked
cargo install cargo-audit --locked
cargo audit
```

For operator changes:

```bash
cd operator
cargo test --verbose
cargo clippy --all-targets -- -D warnings
cargo fmt -- --check
```

For UI changes:

```bash
cd ui
npm ci
npm test
npm run type-check
npm run lint
npm run unused-deps
npm audit --omit=dev --audit-level=high
npm run build
```

For Helm changes:

```bash
helm lint helm/streamforge-operator
helm template streamforge helm/streamforge-operator \
  --set operator.image.repository=test \
  --set operator.image.tag=test
```

Run only the checks relevant to a documentation-only change, including the
site’s internal-link check after generating `_site`:

```bash
python3 scripts/docs/check_internal_links.py _site --baseurl /streamforge
```

Use the maintained performance procedure in [PERFORMANCE.md](PERFORMANCE.md)
for performance claims; a local `cargo bench` result alone is not publication
evidence.

## Pull requests

A pull request must:

- explain what changed and why, and link related issues;
- include tests or explain why behavior is unchanged;
- pass required CI and resolve all review conversations;
- carry at least one of `bug`, `enhancement`, `documentation`, `maintenance`,
  or `dependencies`;
- add the `security` label when touching security-sensitive paths;
- document breaking changes and provide migration guidance; and
- update documentation and the changelog, or use `skip-changelog` when a
  changelog update is intentionally unnecessary.

Small, reviewable commits are preferred. Do not include credentials, generated
build output, or unrelated cleanup. Security vulnerabilities must not be filed
publicly; follow the [Security Policy](https://github.com/rahulbsw/streamforge/blob/main/SECURITY.md).

See [Governance](https://github.com/rahulbsw/streamforge/blob/main/GOVERNANCE.md)
for decision authority and review expectations, and
[Support](https://github.com/rahulbsw/streamforge/blob/main/SUPPORT.md) for help.
