#!/usr/bin/env bash
set -euo pipefail

sdk_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${sdk_root}/../.." && pwd)"
fixture_root="${repo_root}/tests/fixtures/wasm"
generated_root="${fixture_root}/generated"
target_root="${sdk_root}/target"

required_wasm_tools_version="1.236.1"
actual_wasm_tools_version="$(wasm-tools --version | awk '{print $2}')"
if [[ "${actual_wasm_tools_version}" != "${required_wasm_tools_version}" ]]; then
    echo "expected wasm-tools ${required_wasm_tools_version}, got ${actual_wasm_tools_version}" >&2
    exit 1
fi

rustup target list --installed | grep -qx "wasm32-unknown-unknown"

cargo build \
    --manifest-path "${sdk_root}/Cargo.toml" \
    --locked \
    --release \
    --target wasm32-unknown-unknown

mkdir -p "${generated_root}"

build_component() {
    local crate_name="$1"
    local fixture_name="$2"
    local core_module="${target_root}/wasm32-unknown-unknown/release/${crate_name}.wasm"
    local component="${generated_root}/${fixture_name}.wasm"

    wasm-tools component new "${core_module}" -o "${component}"
    wasm-tools validate --features component-model "${component}"
    if wasm-tools component wit "${component}" | grep '^  import ' >/dev/null; then
        echo "${fixture_name}.wasm unexpectedly imports host functionality" >&2
        exit 1
    fi
}

build_component "streamforge_envelope_transform_guest" "envelope-transform-v1"
build_component "streamforge_filter_guest" "filter-v1"
build_component "streamforge_value_transform_guest" "value-transform-v1"

wasm-tools parse \
    "${fixture_root}/source/forbidden-import.wat" \
    -o "${generated_root}/forbidden-import.wasm"
wasm-tools validate --features component-model \
    "${generated_root}/forbidden-import.wasm"

(
    cd "${generated_root}"
    shasum -a 256 \
        envelope-transform-v1.wasm \
        filter-v1.wasm \
        forbidden-import.wasm \
        value-transform-v1.wasm \
        > SHA256SUMS
)
