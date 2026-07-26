#[path = "fixtures/wasm/support.rs"]
mod support;

use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use streamforge::{
    MessageEnvelope, WasmAbiVersion, WasmConfig, WasmErrorKind, WasmModuleConfig, WasmRegistry,
    WasmRuntimeConfig, WasmWorld,
};

#[test]
fn checked_in_fixture_digests_match_the_manifest() {
    let manifest = fs::read_to_string(support::fixture_path("SHA256SUMS")).unwrap();
    let mut checked = 0;
    for line in manifest.lines() {
        let (expected, file) = line.split_once("  ").unwrap();
        assert_eq!(support::digest(&support::fixture_path(file)), expected);
        checked += 1;
    }
    assert_eq!(checked, 4);
}

#[test]
fn rejects_parent_directory_escape() {
    let temporary = support::TestDirectory::new("traversal");
    let root = temporary.path().join("root");
    fs::create_dir(&root).unwrap();
    let outside = temporary.path().join("outside.wasm");
    fs::copy(support::fixture_path("filter-v1.wasm"), &outside).unwrap();
    let config = WasmConfig {
        module_root: root,
        runtime: WasmRuntimeConfig::default(),
        modules: vec![WasmModuleConfig {
            name: "escape".to_string(),
            path: PathBuf::from("../outside.wasm"),
            sha256: support::digest(&outside),
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        }],
    };

    let error = support::load_error(&config);
    assert_eq!(error.kind(), WasmErrorKind::Configuration);
    assert!(error.message().contains("escapes module_root"));
}

#[cfg(unix)]
#[test]
fn rejects_symbolic_link_escape() {
    use std::os::unix::fs::symlink;

    let temporary = support::TestDirectory::new("symlink");
    let root = temporary.path().join("root");
    fs::create_dir(&root).unwrap();
    let outside = temporary.path().join("outside.wasm");
    fs::copy(support::fixture_path("filter-v1.wasm"), &outside).unwrap();
    symlink(&outside, root.join("link.wasm")).unwrap();
    let config = WasmConfig {
        module_root: root,
        runtime: WasmRuntimeConfig::default(),
        modules: vec![WasmModuleConfig {
            name: "symlink".to_string(),
            path: PathBuf::from("link.wasm"),
            sha256: support::digest(&outside),
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        }],
    };

    let error = support::load_error(&config);
    assert_eq!(error.kind(), WasmErrorKind::Configuration);
}

#[test]
fn rejects_digest_mismatch_before_compilation() {
    let mut module = support::module("filter", "filter-v1.wasm", WasmWorld::Filter);
    module.sha256 = "00".repeat(32);

    let error = support::load_error(&support::default_config(vec![module]));
    assert_eq!(error.kind(), WasmErrorKind::Integrity);
}

#[test]
fn loaded_registry_uses_verified_bytes_after_artifact_mutation() {
    let temporary = support::TestDirectory::new("mutation");
    let component = temporary.path().join("filter.wasm");
    fs::copy(support::fixture_path("filter-v1.wasm"), &component).unwrap();
    let config = WasmConfig {
        module_root: temporary.path().to_path_buf(),
        runtime: WasmRuntimeConfig::default(),
        modules: vec![WasmModuleConfig {
            name: "filter".to_string(),
            path: PathBuf::from("filter.wasm"),
            sha256: hex::encode(Sha256::digest(fs::read(&component).unwrap())),
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        }],
    };
    let registry = WasmRegistry::load(&config).unwrap();
    fs::write(&component, b"mutated after verified load").unwrap();

    assert!(registry
        .invoke_filter(
            "filter",
            &MessageEnvelope::new(serde_json::json!({"allow": true}))
        )
        .unwrap());
}
