use crate::error::{MirrorMakerError, Result};
use digest::Digest;
use md5::Md5;
use murmur3::murmur3_x64_128;
use serde_json::Value;
use sha2::{Sha256, Sha512};
use std::io::Cursor;

/// Hash algorithm types
#[derive(Debug, Clone, Copy)]
pub enum HashAlgorithm {
    Md5,
    Sha256,
    Sha512,
    Murmur64,
    Murmur128,
}

impl HashAlgorithm {
    /// Parse hash algorithm from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "md5" => Ok(HashAlgorithm::Md5),
            "sha256" | "sha-256" => Ok(HashAlgorithm::Sha256),
            "sha512" | "sha-512" => Ok(HashAlgorithm::Sha512),
            "murmur64" | "murmur-64" => Ok(HashAlgorithm::Murmur64),
            "murmur128" | "murmur-128" => Ok(HashAlgorithm::Murmur128),
            _ => Err(MirrorMakerError::Config(format!(
                "Unknown hash algorithm: {}",
                s
            ))),
        }
    }
}

/// Hash a byte slice using the specified algorithm
pub fn hash_bytes(data: &[u8], algorithm: HashAlgorithm) -> Result<String> {
    match algorithm {
        HashAlgorithm::Md5 => {
            let mut hasher = Md5::new();
            hasher.update(data);
            Ok(hex::encode(hasher.finalize()))
        }
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(data);
            Ok(hex::encode(hasher.finalize()))
        }
        HashAlgorithm::Sha512 => {
            let mut hasher = Sha512::new();
            hasher.update(data);
            Ok(hex::encode(hasher.finalize()))
        }
        HashAlgorithm::Murmur64 => {
            // For Murmur64, we use the lower 64 bits of Murmur128
            let hash = murmur3_x64_128(&mut Cursor::new(data), 0)
                .map_err(|e| MirrorMakerError::Processing(format!("Murmur hash error: {}", e)))?;
            Ok(format!("{:016x}", hash as u64))
        }
        HashAlgorithm::Murmur128 => {
            let hash = murmur3_x64_128(&mut Cursor::new(data), 0)
                .map_err(|e| MirrorMakerError::Processing(format!("Murmur hash error: {}", e)))?;
            Ok(format!("{:032x}", hash))
        }
    }
}

/// Hash a JSON value (converts to string first)
pub fn hash_value(value: &Value, algorithm: HashAlgorithm) -> Result<String> {
    let data = match value {
        Value::String(s) => s.as_bytes().to_vec(),
        Value::Number(n) => n.to_string().as_bytes().to_vec(),
        Value::Bool(b) => b.to_string().as_bytes().to_vec(),
        Value::Null => b"null".to_vec(),
        // For objects and arrays, serialize to JSON string
        Value::Object(_) | Value::Array(_) => {
            serde_json::to_string(value)
                .map_err(|e| MirrorMakerError::Processing(format!("JSON serialization error: {}", e)))?
                .as_bytes()
                .to_vec()
        }
    };

    hash_bytes(&data, algorithm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_md5_hash() {
        let result = hash_bytes(b"hello world", HashAlgorithm::Md5).unwrap();
        assert_eq!(result, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_sha256_hash() {
        let result = hash_bytes(b"hello world", HashAlgorithm::Sha256).unwrap();
        assert_eq!(
            result,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_sha512_hash() {
        let result = hash_bytes(b"hello world", HashAlgorithm::Sha512).unwrap();
        assert_eq!(
            result,
            "309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f"
        );
    }

    #[test]
    fn test_murmur64_hash() {
        let result = hash_bytes(b"hello world", HashAlgorithm::Murmur64).unwrap();
        // Murmur hash is deterministic
        assert_eq!(result.len(), 16); // 64 bits = 16 hex chars
    }

    #[test]
    fn test_murmur128_hash() {
        let result = hash_bytes(b"hello world", HashAlgorithm::Murmur128).unwrap();
        // Murmur128 hash
        assert_eq!(result.len(), 32); // 128 bits = 32 hex chars
    }

    #[test]
    fn test_hash_value_string() {
        let value = json!("hello world");
        let result = hash_value(&value, HashAlgorithm::Md5).unwrap();
        assert_eq!(result, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn test_hash_value_number() {
        let value = json!(12345);
        let result = hash_value(&value, HashAlgorithm::Md5).unwrap();
        // Hash of "12345"
        assert_eq!(result, "827ccb0eea8a706c4c34a16891f84e7b");
    }

    #[test]
    fn test_hash_value_object() {
        let value = json!({"name": "test", "id": 123});
        let result = hash_value(&value, HashAlgorithm::Md5).unwrap();
        // Should hash the JSON string representation
        assert!(!result.is_empty());
        assert_eq!(result.len(), 32); // MD5 is 32 hex chars
    }

    #[test]
    fn test_parse_algorithm() {
        assert!(matches!(
            HashAlgorithm::from_str("md5").unwrap(),
            HashAlgorithm::Md5
        ));
        assert!(matches!(
            HashAlgorithm::from_str("SHA256").unwrap(),
            HashAlgorithm::Sha256
        ));
        assert!(matches!(
            HashAlgorithm::from_str("sha-512").unwrap(),
            HashAlgorithm::Sha512
        ));
        assert!(matches!(
            HashAlgorithm::from_str("murmur64").unwrap(),
            HashAlgorithm::Murmur64
        ));
        assert!(matches!(
            HashAlgorithm::from_str("murmur-128").unwrap(),
            HashAlgorithm::Murmur128
        ));
    }

    #[test]
    fn test_parse_algorithm_invalid() {
        assert!(HashAlgorithm::from_str("invalid").is_err());
    }

    #[test]
    fn test_hash_consistency() {
        // Same input should produce same hash
        let data = b"test data";
        let hash1 = hash_bytes(data, HashAlgorithm::Sha256).unwrap();
        let hash2 = hash_bytes(data, HashAlgorithm::Sha256).unwrap();
        assert_eq!(hash1, hash2);
    }
}
