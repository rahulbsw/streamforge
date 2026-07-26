# WebAssembly UDF fixtures

`generated/` contains binary components consumed by integration tests.
They are not opaque artifacts:

- the three valid components are built from `udf-sdk/rust/examples/`;
- the negative import component is built from
  `source/forbidden-import.wat`;
- `udf-sdk/rust/build-fixtures.sh` rebuilds and validates all artifacts;
- `generated/SHA256SUMS` pins every checked-in binary.

The valid examples target `wasm32-unknown-unknown`; the build script rejects
any host import before updating the digest manifest.
