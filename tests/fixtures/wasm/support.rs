#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use streamforge::{
    WasmAbiVersion, WasmConfig, WasmError, WasmModuleConfig, WasmRegistry, WasmRuntimeConfig,
    WasmWorld,
};

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wasm/generated")
}

pub fn fixture_path(name: &str) -> PathBuf {
    fixture_root().join(name)
}

pub fn digest(path: &Path) -> String {
    hex::encode(Sha256::digest(fs::read(path).unwrap()))
}

pub fn module(name: &str, file: &str, world: WasmWorld) -> WasmModuleConfig {
    WasmModuleConfig {
        name: name.to_string(),
        path: PathBuf::from(file),
        sha256: digest(&fixture_path(file)),
        world,
        abi: WasmAbiVersion::V1,
    }
}

pub fn config(modules: Vec<WasmModuleConfig>, runtime: WasmRuntimeConfig) -> WasmConfig {
    WasmConfig {
        module_root: fixture_root(),
        runtime,
        modules,
    }
}

pub fn default_config(modules: Vec<WasmModuleConfig>) -> WasmConfig {
    config(
        modules,
        WasmRuntimeConfig {
            // Correctness/security tests run concurrently and should not
            // accidentally exercise the production deadline. Timeout tests
            // provide their own deliberately strict runtime configuration.
            max_execution_ms: 1_000,
            ..WasmRuntimeConfig::default()
        },
    )
}

pub fn load_error(config: &WasmConfig) -> WasmError {
    match WasmRegistry::load(config) {
        Ok(_) => panic!("registry load unexpectedly succeeded"),
        Err(error) => error,
    }
}

pub struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    pub fn new(label: &str) -> Self {
        let sequence = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "streamforge-wasm-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
