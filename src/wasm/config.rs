use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const MAX_MODULES: usize = 32;

/// Top-level registry of stateless WebAssembly UDF components.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Trusted root containing all configured module files.
    pub module_root: PathBuf,

    /// Runtime-wide resource and interruption limits.
    #[serde(default)]
    pub runtime: WasmRuntimeConfig,

    /// Named component modules available to destinations.
    #[serde(default)]
    pub modules: Vec<WasmModuleConfig>,
}

/// Runtime-wide limits for WebAssembly UDFs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmRuntimeConfig {
    #[serde(default = "default_max_module_bytes")]
    pub max_module_bytes: usize,
    #[serde(default = "default_max_input_bytes")]
    pub max_input_bytes: usize,
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    #[serde(default = "default_max_memory_bytes")]
    pub max_memory_bytes: usize,
    #[serde(default = "default_max_table_elements")]
    pub max_table_elements: usize,
    #[serde(default = "default_max_execution_ms")]
    pub max_execution_ms: u64,
    #[serde(default = "default_epoch_tick_ms")]
    pub epoch_tick_ms: u64,
    #[serde(default = "default_max_stack_bytes")]
    pub max_wasm_stack_bytes: usize,
    #[serde(default = "default_max_concurrent_instances")]
    pub max_concurrent_instances: u32,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            max_module_bytes: default_max_module_bytes(),
            max_input_bytes: default_max_input_bytes(),
            max_output_bytes: default_max_output_bytes(),
            max_memory_bytes: default_max_memory_bytes(),
            max_table_elements: default_max_table_elements(),
            max_execution_ms: default_max_execution_ms(),
            epoch_tick_ms: default_epoch_tick_ms(),
            max_wasm_stack_bytes: default_max_stack_bytes(),
            max_concurrent_instances: default_max_concurrent_instances(),
        }
    }
}

/// A digest-pinned WebAssembly component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmModuleConfig {
    /// Stable registry name used by destination references and metric labels.
    pub name: String,
    /// Relative path below `module_root`, or an absolute path inside it.
    pub path: PathBuf,
    /// SHA-256 digest encoded as exactly 64 hexadecimal characters.
    pub sha256: String,
    /// WIT world exported by this component.
    pub world: WasmWorld,
    /// Versioned StreamForge component ABI.
    pub abi: WasmAbiVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WasmWorld {
    Filter,
    ValueTransform,
    EnvelopeTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WasmAbiVersion {
    V1,
}

/// UDF module references attached to one destination.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationUdfConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_transform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub envelope_transform: Option<String>,
}

impl DestinationUdfConfig {
    pub fn is_empty(&self) -> bool {
        self.filter.is_none() && self.value_transform.is_none() && self.envelope_transform.is_none()
    }
}

impl WasmRuntimeConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        validate_range(
            "wasm.runtime.max_module_bytes",
            self.max_module_bytes,
            1,
            256 * 1024 * 1024,
        )?;
        validate_range(
            "wasm.runtime.max_input_bytes",
            self.max_input_bytes,
            1,
            64 * 1024 * 1024,
        )?;
        validate_range(
            "wasm.runtime.max_output_bytes",
            self.max_output_bytes,
            1,
            64 * 1024 * 1024,
        )?;
        validate_range(
            "wasm.runtime.max_memory_bytes",
            self.max_memory_bytes,
            64 * 1024,
            512 * 1024 * 1024,
        )?;
        if self.max_memory_bytes % (64 * 1024) != 0 {
            return Err(
                "wasm.runtime.max_memory_bytes must be a multiple of the WebAssembly page size (65536)"
                    .into(),
            );
        }
        validate_range(
            "wasm.runtime.max_table_elements",
            self.max_table_elements,
            1,
            1_000_000,
        )?;
        validate_range(
            "wasm.runtime.max_execution_ms",
            self.max_execution_ms,
            1,
            60_000,
        )?;
        validate_range("wasm.runtime.epoch_tick_ms", self.epoch_tick_ms, 1, 1_000)?;
        if self.epoch_tick_ms > self.max_execution_ms {
            return Err("wasm.runtime.epoch_tick_ms must be <= max_execution_ms".into());
        }
        validate_range(
            "wasm.runtime.max_wasm_stack_bytes",
            self.max_wasm_stack_bytes,
            64 * 1024,
            16 * 1024 * 1024,
        )?;
        validate_range(
            "wasm.runtime.max_concurrent_instances",
            self.max_concurrent_instances,
            1,
            1_024,
        )?;
        Ok(())
    }
}

impl WasmConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.module_root.as_os_str().is_empty() {
            return Err("wasm.module_root must not be empty".into());
        }
        self.runtime.validate()?;
        if self.modules.is_empty() || self.modules.len() > MAX_MODULES {
            return Err(format!(
                "wasm.modules must contain between 1 and {MAX_MODULES} modules"
            ));
        }

        let mut names = HashSet::with_capacity(self.modules.len());
        let mut digest_paths = HashMap::with_capacity(self.modules.len());
        for module in &self.modules {
            if !valid_module_name(&module.name) {
                return Err(format!(
                    "wasm module name {:?} must be 1-128 ASCII letters, digits, '.', '_' or '-' and start with a letter or digit",
                    module.name
                ));
            }
            if !names.insert(module.name.as_str()) {
                return Err(format!("duplicate wasm module name {:?}", module.name));
            }
            if module.path.as_os_str().is_empty() {
                return Err(format!(
                    "wasm module {:?} path must not be empty",
                    module.name
                ));
            }
            if module.sha256.len() != 64
                || !module.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(format!(
                    "wasm module {:?} sha256 must contain exactly 64 hexadecimal characters",
                    module.name
                ));
            }
            let digest = module.sha256.to_ascii_lowercase();
            if let Some(existing_path) = digest_paths.insert(digest, &module.path) {
                if existing_path != &module.path {
                    return Err(format!(
                        "wasm modules sharing a SHA-256 digest must use the same path; found {:?} and {:?}",
                        existing_path, module.path
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_refs(
        &self,
        destination: &str,
        refs: &DestinationUdfConfig,
    ) -> Result<(), String> {
        self.validate_ref(
            destination,
            "filter",
            refs.filter.as_deref(),
            WasmWorld::Filter,
        )?;
        self.validate_ref(
            destination,
            "value_transform",
            refs.value_transform.as_deref(),
            WasmWorld::ValueTransform,
        )?;
        self.validate_ref(
            destination,
            "envelope_transform",
            refs.envelope_transform.as_deref(),
            WasmWorld::EnvelopeTransform,
        )
    }

    fn validate_ref(
        &self,
        destination: &str,
        slot: &str,
        name: Option<&str>,
        expected_world: WasmWorld,
    ) -> Result<(), String> {
        let Some(name) = name else {
            return Ok(());
        };
        let module = self
            .modules
            .iter()
            .find(|module| module.name == name)
            .ok_or_else(|| {
                format!("{destination} UDF slot {slot:?} references unknown wasm module {name:?}")
            })?;
        if module.world != expected_world {
            return Err(format!(
                "{destination} UDF slot {slot:?} requires world {expected_world:?}, but module {name:?} declares {:?}",
                module.world
            ));
        }
        Ok(())
    }
}

fn validate_range<T>(field: &str, value: T, minimum: T, maximum: T) -> Result<(), String>
where
    T: Copy + Ord + std::fmt::Display,
{
    if value < minimum || value > maximum {
        return Err(format!(
            "{field} must be between {minimum} and {maximum}, got {value}"
        ));
    }
    Ok(())
}

fn valid_module_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 128 {
        return false;
    }
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

const fn default_max_module_bytes() -> usize {
    16 * 1024 * 1024
}

const fn default_max_input_bytes() -> usize {
    1024 * 1024
}

const fn default_max_output_bytes() -> usize {
    1024 * 1024
}

const fn default_max_memory_bytes() -> usize {
    64 * 1024 * 1024
}

const fn default_max_table_elements() -> usize {
    10_000
}

const fn default_max_execution_ms() -> u64 {
    5
}

const fn default_epoch_tick_ms() -> u64 {
    1
}

const fn default_max_stack_bytes() -> usize {
    2 * 1024 * 1024
}

const fn default_max_concurrent_instances() -> u32 {
    256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_bounded_and_nonzero() {
        let runtime = WasmRuntimeConfig::default();
        assert!(runtime.validate().is_ok());
        assert_eq!(runtime.max_memory_bytes, 64 * 1024 * 1024);
        assert_eq!(runtime.max_execution_ms, 5);
    }

    #[test]
    fn validates_reference_worlds() {
        let config = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules: vec![WasmModuleConfig {
                name: "filter".into(),
                path: "filter.wasm".into(),
                sha256: "00".repeat(32),
                world: WasmWorld::Filter,
                abi: WasmAbiVersion::V1,
            }],
        };
        assert!(config
            .validate_refs(
                "destination \"out\"",
                &DestinationUdfConfig {
                    filter: Some("filter".into()),
                    ..DestinationUdfConfig::default()
                },
            )
            .is_ok());
        assert!(config
            .validate_refs(
                "destination \"out\"",
                &DestinationUdfConfig {
                    value_transform: Some("filter".into()),
                    ..DestinationUdfConfig::default()
                },
            )
            .unwrap_err()
            .contains("requires world ValueTransform"));
    }

    #[test]
    fn rejects_duplicate_names_and_malformed_digests() {
        let module = WasmModuleConfig {
            name: "same".into(),
            path: "module.wasm".into(),
            sha256: "not-a-digest".into(),
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        };
        let malformed = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules: vec![module.clone()],
        };
        assert!(malformed.validate().unwrap_err().contains("sha256"));

        let duplicate = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules: vec![
                WasmModuleConfig {
                    sha256: "00".repeat(32),
                    ..module.clone()
                },
                WasmModuleConfig {
                    sha256: "11".repeat(32),
                    ..module
                },
            ],
        };
        assert!(duplicate.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn rejects_empty_and_oversized_registries() {
        let empty = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules: vec![],
        };
        assert!(empty.validate().unwrap_err().contains("between 1 and 32"));

        let modules = (0..=MAX_MODULES)
            .map(|index| WasmModuleConfig {
                name: format!("module-{index}"),
                path: "module.wasm".into(),
                sha256: format!("{index:064x}"),
                world: WasmWorld::Filter,
                abi: WasmAbiVersion::V1,
            })
            .collect();
        let oversized = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules,
        };
        assert!(oversized
            .validate()
            .unwrap_err()
            .contains("between 1 and 32"));
    }

    #[test]
    fn rejects_pathological_runtime_limits() {
        let mut runtime = WasmRuntimeConfig::default();
        runtime.epoch_tick_ms = runtime.max_execution_ms + 1;
        assert!(runtime.validate().unwrap_err().contains("epoch_tick_ms"));

        let runtime = WasmRuntimeConfig {
            max_concurrent_instances: 1_025,
            ..WasmRuntimeConfig::default()
        };
        assert!(runtime
            .validate()
            .unwrap_err()
            .contains("max_concurrent_instances"));

        let mut runtime = WasmRuntimeConfig::default();
        runtime.max_memory_bytes += 1;
        assert!(runtime.validate().unwrap_err().contains("multiple"));
    }

    #[test]
    fn duplicate_digest_aliases_must_share_one_path() {
        let first = WasmModuleConfig {
            name: "first".into(),
            path: "shared.wasm".into(),
            sha256: "aa".repeat(32),
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        };
        let valid = WasmConfig {
            module_root: "/udfs".into(),
            runtime: WasmRuntimeConfig::default(),
            modules: vec![
                first.clone(),
                WasmModuleConfig {
                    name: "alias".into(),
                    world: WasmWorld::ValueTransform,
                    ..first.clone()
                },
            ],
        };
        assert!(valid.validate().is_ok());

        let invalid = WasmConfig {
            modules: vec![
                first.clone(),
                WasmModuleConfig {
                    name: "alias".into(),
                    path: "other.wasm".into(),
                    ..first
                },
            ],
            ..valid
        };
        assert!(invalid.validate().unwrap_err().contains("same path"));
    }
}
