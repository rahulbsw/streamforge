# StreamForge Rust UDF SDK

This directory contains minimal Rust guests for every StreamForge v1 WIT
world. They are examples and executable fixture sources, not a separate host
API.

## Requirements

- the repository's pinned stable Rust toolchain;
- the `wasm32-unknown-unknown` Rust target;
- `wasm-tools` 1.236.1.

Install the tools once:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-tools --version 1.236.1 --locked \
  --no-default-features --features component,parse,print,validate
```

Build and verify all fixtures:

```bash
./udf-sdk/rust/build-fixtures.sh
```

The script compiles the three Rust guests, wraps their embedded WIT metadata
as components, rejects imports in the production examples, builds the
intentional forbidden-import negative fixture, validates every component, and
updates `tests/fixtures/wasm/generated/SHA256SUMS`.

## Contract

The source WIT is `wit/streamforge-udf-v1/streamforge-udf.wit`.

- `filter` receives the full immutable source envelope and returns a decision.
- `value-transform` receives and returns JSON bytes.
- `envelope-transform` may replace key, value, headers, and timestamp, but
  cannot replace source topic, partition, or offset.

Guests have no WASI or application imports. Do not add filesystem, network,
clock, random, environment, or logging imports. A guest must return structured
`udf-error` values for expected failures. Traps are reserved for unexpected
failures and are bounded by the host runtime.

The example guests include deterministic test switches:

- filter JSON containing `"guest_error":true` returns a guest error;
- filter JSON containing `"loop":true` loops until the host deadline;
- filter JSON containing `"stack":true` exhausts the bounded guest stack;
- filter JSON containing `"state_probe":true` proves each call starts from
  fresh guest state;
- value JSON containing `"large":true` emits an oversized result.

These switches exist only to exercise the host security contract.
