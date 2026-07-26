use crate::wasm::config::WasmModuleConfig;
use crate::wasm::error::{error, WasmError, WasmErrorKind};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

pub(crate) fn canonical_module_root(path: &Path) -> Result<std::path::PathBuf, WasmError> {
    let root = fs::canonicalize(path).map_err(|failure| {
        error(
            WasmErrorKind::Io,
            None,
            format!("cannot resolve wasm.module_root {path:?}: {failure}"),
        )
    })?;
    if !root.is_dir() {
        return Err(error(
            WasmErrorKind::Configuration,
            None,
            format!("wasm.module_root {root:?} is not a directory"),
        ));
    }
    Ok(root)
}

pub(crate) fn read_verified_module(
    root: &Path,
    module: &WasmModuleConfig,
    max_module_bytes: usize,
) -> Result<Vec<u8>, WasmError> {
    let requested = if module.path.is_absolute() {
        module.path.clone()
    } else {
        root.join(&module.path)
    };
    let canonical = fs::canonicalize(&requested).map_err(|failure| {
        error(
            WasmErrorKind::Io,
            Some(&module.name),
            format!("cannot resolve module path {requested:?}: {failure}"),
        )
    })?;
    if !canonical.starts_with(root) || canonical == root {
        return Err(error(
            WasmErrorKind::Configuration,
            Some(&module.name),
            format!("module path {canonical:?} escapes module_root {root:?}"),
        ));
    }
    let symlink_metadata = fs::symlink_metadata(&requested).map_err(|failure| {
        error(
            WasmErrorKind::Io,
            Some(&module.name),
            format!("cannot inspect module path {requested:?}: {failure}"),
        )
    })?;
    if symlink_metadata.file_type().is_symlink() {
        return Err(error(
            WasmErrorKind::Configuration,
            Some(&module.name),
            "module file must not be a symbolic link",
        ));
    }

    let mut file = File::open(&canonical).map_err(|failure| {
        error(
            WasmErrorKind::Io,
            Some(&module.name),
            format!("cannot open component {canonical:?}: {failure}"),
        )
    })?;
    let metadata = file.metadata().map_err(|failure| {
        error(
            WasmErrorKind::Io,
            Some(&module.name),
            format!("cannot inspect open component {canonical:?}: {failure}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(error(
            WasmErrorKind::Configuration,
            Some(&module.name),
            "module path must identify a regular file",
        ));
    }
    if metadata.len() > max_module_bytes as u64 {
        return Err(module_size_error(
            &module.name,
            metadata.len(),
            max_module_bytes,
        ));
    }

    let capacity = usize::try_from(metadata.len())
        .unwrap_or(max_module_bytes)
        .min(max_module_bytes);
    let mut bytes = Vec::with_capacity(capacity);
    file.by_ref()
        .take(max_module_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|failure| io_error(&module.name, &canonical, failure))?;
    if bytes.len() > max_module_bytes {
        return Err(module_size_error(
            &module.name,
            bytes.len() as u64,
            max_module_bytes,
        ));
    }

    let actual = Sha256::digest(&bytes);
    let expected = hex::decode(&module.sha256).map_err(|failure| {
        error(
            WasmErrorKind::Configuration,
            Some(&module.name),
            format!("configured SHA-256 digest is invalid: {failure}"),
        )
    })?;
    let mut difference = actual.len() ^ expected.len();
    for (left, right) in actual.iter().zip(expected.iter()) {
        difference |= usize::from(left ^ right);
    }
    if difference != 0 {
        return Err(error(
            WasmErrorKind::Integrity,
            Some(&module.name),
            format!(
                "SHA-256 mismatch: expected {}, got {}",
                module.sha256.to_ascii_lowercase(),
                hex::encode(actual)
            ),
        ));
    }
    Ok(bytes)
}

fn module_size_error(module: &str, actual: u64, maximum: usize) -> WasmError {
    error(
        WasmErrorKind::ResourceLimit,
        Some(module),
        format!("module is {actual} bytes, exceeding configured limit {maximum}"),
    )
}

fn io_error(module: &str, path: &Path, failure: io::Error) -> WasmError {
    error(
        WasmErrorKind::Io,
        Some(module),
        format!("cannot read component {path:?}: {failure}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::config::{WasmAbiVersion, WasmWorld};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_digest_mismatch_and_oversize() {
        let root = create_test_directory();
        let module_path = root.join("guest.wasm");
        fs::write(&module_path, b"guest").unwrap();
        let module = test_module("guest.wasm", "00".repeat(32));
        assert_eq!(
            read_verified_module(&root, &module, 1024)
                .unwrap_err()
                .kind(),
            WasmErrorKind::Integrity
        );
        assert_eq!(
            read_verified_module(&root, &module, 1).unwrap_err().kind(),
            WasmErrorKind::ResourceLimit
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_parent_escape() {
        let root = create_test_directory();
        let outside = root.with_extension("wasm");
        fs::write(&outside, b"guest").unwrap();
        let module = test_module(
            PathBuf::from("..").join(outside.file_name().unwrap()),
            hex::encode(Sha256::digest(b"guest")),
        );
        assert_eq!(
            read_verified_module(&root, &module, 1024)
                .unwrap_err()
                .kind(),
            WasmErrorKind::Configuration
        );
        fs::remove_dir_all(root).unwrap();
        fs::remove_file(outside).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_even_when_target_is_inside_root() {
        use std::os::unix::fs::symlink;

        let root = create_test_directory();
        fs::write(root.join("real.wasm"), b"guest").unwrap();
        symlink(root.join("real.wasm"), root.join("link.wasm")).unwrap();
        let module = test_module("link.wasm", hex::encode(Sha256::digest(b"guest")));
        assert_eq!(
            read_verified_module(&root, &module, 1024)
                .unwrap_err()
                .kind(),
            WasmErrorKind::Configuration
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn test_module(path: impl Into<PathBuf>, sha256: String) -> WasmModuleConfig {
        WasmModuleConfig {
            name: "guest".into(),
            path: path.into(),
            sha256,
            world: WasmWorld::Filter,
            abi: WasmAbiVersion::V1,
        }
    }

    fn create_test_directory() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "streamforge-wasm-loader-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        fs::canonicalize(path).unwrap()
    }
}
