# WebAssembly UDFs

This document defines the StreamForge v1 WebAssembly UDF contract. The feature
is opt-in: configurations without a `wasm` block retain the native processing
path and do not initialize Wasmtime.

## Scope

The v1 host supports stateless, digest-pinned components for three destination
stages:

- envelope-aware filters;
- JSON value transforms;
- full-envelope transforms of key, value, headers, and timestamp.

The native DSL remains the default filter and transform language. UDF module
references are typed configuration fields rather than dynamically parsed DSL
expressions.

The following are deliberately outside this contract:

- WASI, filesystem, socket, environment, clock, random, or logging imports;
- persistent or fault-tolerant guest state;
- network download, OCI distribution, or mutable module reload;
- guest mutation of source topic, partition, or offset;
- multi-language guest SDKs.

Changing a component or its digest requires a configuration rollout and process
restart.

## Configuration

```yaml
wasm:
  module_root: /etc/streamforge/udfs
  runtime:
    max_module_bytes: 16777216
    max_input_bytes: 1048576
    max_output_bytes: 1048576
    max_memory_bytes: 67108864
    max_table_elements: 10000
    max_execution_ms: 5
    epoch_tick_ms: 1
    max_wasm_stack_bytes: 2097152
    max_concurrent_instances: 256
  modules:
    - name: redact
      path: redact.wasm
      sha256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
      world: value_transform
      abi: v1

routing:
  routing_type: filter
  destinations:
    - output: sanitized-events
      udfs:
        value_transform: redact
```

Single-destination mode uses the same reference shape at the root:

```yaml
output: sanitized-events
udfs:
  filter: allow-event
  value_transform: redact
  envelope_transform: rewrite-metadata
```

Module names are stable metric labels and must be unique; a registry contains
between 1 and 32 modules. Each destination reference must resolve to a module
whose configured world matches the stage. Unknown references, duplicate
modules, invalid digests, world mismatches, and UDF references without a
registry are startup errors.

## ABI

The canonical interface is the versioned WIT package
`streamforge:udf@1.0.0` under `wit/streamforge-udf-v1/`.

The host exposes three distinct worlds:

- `filter-v1` accepts the envelope input and returns a boolean decision;
- `value-transform-v1` accepts JSON bytes and returns JSON bytes;
- `envelope-transform-v1` accepts the envelope and returns only mutable fields.

Each world also exports the shared `types` interface. This preserves WIT's
nominal type identity while allowing normal `wit-bindgen` Rust guests to remain
zero-import components; it does not expose a host capability.

JSON crosses the boundary as `list<u8>` to avoid redundant host string
construction. Returned JSON must be valid UTF-8 JSON within the configured
output limit. Header values remain bytes. Source coordinates appear only in
input types, so a guest cannot rewrite provenance used by logging, DLQ, or
delivery coordination.

A guest error is structured and bounded. The host retains the stable error code
but deliberately discards the guest-controlled message, so a guest cannot echo
record payloads into logs or DLQ metadata. Guest-provided values are never
metric labels.

## Module loading

Startup performs the complete trust-boundary validation before Kafka
consumption:

1. canonicalize the configured artifact root;
2. resolve the module path beneath that root and reject traversal or symlink
   escape;
3. reject non-regular and oversized artifacts;
4. read the component once into a bounded buffer;
5. verify the configured SHA-256 digest over those exact bytes;
6. compile each unique digest once;
7. reject every unresolved import, including all WASI interfaces;
8. bind the component to its configured WIT world;
9. instantiate a startup probe to catch initialization traps.

The verified bytes, not a subsequently reopened path, are compiled. Unsafe
deserialization of precompiled native code is not part of v1.

## Runtime isolation and limits

StreamForge uses one process-wide Wasmtime engine with:

- Cranelift compilation;
- the component model;
- epoch interruption driven by a dedicated native ticker thread;
- Wasmtime's pooling allocator;
- explicit component, core instance, table, memory, and stack limits.

The default correctness mode creates a fresh store and component instance from
the pre-linked component for each invocation. Wasmtime may recycle cleared VM
resources internally, but guest linear memory and globals are not shared across
records. A reusable-instance optimization may replace this only after
state-reset, cross-record isolation, trap recovery, concurrency, and performance
tests prove the same contract.

UDF execution is synchronous and inline with the existing native stage. No
ambient host call can block. The independent epoch thread ensures an infinite
guest loop is interrupted even if Tokio workers are occupied. Fuel-based
instruction accounting is not the default because its steady-state overhead is
higher; it may be added only as an explicitly benchmarked deterministic mode.

The host validates input size before lowering and validates output size and
shape before applying a mutation. Full-envelope changes are transactional:
invalid output does not partially modify the original record.

When native and UDF stages are both configured, the native stage runs first.
The destination order is native filter, UDF filter, native value transform, UDF
value transform, native envelope mutations, then UDF envelope mutation. This
ensures key, header, timestamp, and full-envelope logic observes the final
destination payload. A false native filter short-circuits the UDF filter; a
failed native value transform does not invoke the UDF transform or any envelope
mutation.

## Error policy

UDF guest errors, traps, timeouts, resource-limit failures, and invalid outputs
are deterministic and are not retried. The configured destination policy is
authoritative:

| Policy | Filter failure | Transform failure |
|---|---|---|
| `fail` | halt without DLQ | halt without DLQ |
| `dlq` | emit one contextual DLQ record | emit one contextual DLQ record |
| `skip_and_log` | skip this destination | skip this destination |
| `continue` | treat the filter as passing | send the original unchanged envelope |

Multi-destination mode reads this policy from each destination. The existing
single-destination configuration has no per-destination policy field: it uses
`dlq` when the global DLQ is enabled and `fail` when it is disabled.

DLQ records identify the destination, stage, and configured module without
including payload data in log fields. Successful destinations are not rerun
when another destination fails.

## Observability

WASM metrics use only startup-known module and world labels plus bounded status
or cause values:

- `streamforge_wasm_invocations_total`;
- `streamforge_wasm_duration_seconds`;
- `streamforge_wasm_input_bytes`;
- `streamforge_wasm_output_bytes`;
- `streamforge_wasm_active_invocations`;
- `streamforge_wasm_compilations_total`;
- `streamforge_wasm_compilation_duration_seconds`.

Allowed invocation status values are `ok`, `guest_error`, `trap`, `timeout`,
`resource_limit`, and `invalid_output`. Payloads, paths, full digests, topics,
partitions, offsets, and guest text are forbidden as metric labels.

## Kubernetes artifacts

The operator supports read-only ConfigMap keys for small components and
read-only PVC paths for larger components. It renders deterministic paths below
the module root and includes both artifact digests and a SHA-256 of the rendered
core configuration in pod-template annotations, so artifact, binding, or
runtime-limit changes cause a rolling restart.

The operator rejects mixed destination broker values until the core supports a
broker per destination. It renders the complete routing destination list; it
must never silently select only the first destination.

UDF containers run as non-root, disallow privilege escalation, drop Linux
capabilities, and use the runtime-default seccomp profile. No `hostPath`,
network fetcher, or writable artifact mount is permitted.

## Performance contract

Every release candidate must measure:

- no-UDF native paths before and after the change;
- startup digest verification, compilation, linking, and probe instantiation;
- filter, value, and envelope invocation at 256 B, 4 KiB, and 64 KiB;
- one and four destinations;
- concurrency through the configured worker count;
- success, guest error, trap, timeout, and output-limit paths;
- long-running memory stability and scheduler liveness.

The no-UDF Criterion path may not regress by more than 3% with statistical
significance. Shared CI runs functional smoke benchmarks; release performance
gates use same-revision A/B runs on controlled native x86-64 and arm64 hardware.

The host must not compile, hash, resolve exports, construct linkers, or open
module paths in the record path. Boundary serialization, canonical ABI copying,
and JSON decoding are profiled separately. If they account for at least 30% of
UDF CPU, a shared raw/canonical envelope representation becomes release-blocking
rather than accepting the avoidable copy.

## Verification

Required automated coverage includes:

- configuration and reference validation;
- path traversal, symlink escape, digest mismatch, oversize, and mutation-after-
  load tests;
- wrong ABI/world and forbidden-import tests;
- valid filter/value/envelope components;
- malformed JSON, header, guest error, and provenance tests;
- trap, timeout, stack, memory, table, and output-limit tests;
- cross-record state isolation and concurrent input-tag isolation;
- policy tests for all four policies with and without retries;
- multi-destination partial success and contextual DLQ tests;
- operator schema, rendering, mount, checksum, and security-context tests;
- runnable smoke components on each supported release target.
