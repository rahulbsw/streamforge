use crate::config::{CompressionAlgo, CompressionType};
use crate::error::{MirrorMakerError, Result};
use flate2::write::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::Write;

/// Compression utilities for message payloads
pub struct Compressor {
    pub compression_type: CompressionType,
    pub algo: CompressionAlgo,
}

impl Compressor {
    pub fn new(compression_type: CompressionType, algo: CompressionAlgo) -> Self {
        Self {
            compression_type,
            algo,
        }
    }

    /// Compress data using configured algorithm
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        if self.compression_type == CompressionType::None {
            return Ok(data.to_vec());
        }

        match self.algo {
            CompressionAlgo::Gzip => self.compress_gzip(data),
            CompressionAlgo::Snappy => self.compress_snappy(data),
            CompressionAlgo::Zstd => self.compress_zstd(data),
            CompressionAlgo::Lz4 => self.compress_lz4(data),
        }
    }

    /// Decompress data
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        if self.compression_type == CompressionType::None {
            return Ok(data.to_vec());
        }

        match self.algo {
            CompressionAlgo::Gzip => self.decompress_gzip(data),
            CompressionAlgo::Snappy => self.decompress_snappy(data),
            CompressionAlgo::Zstd => self.decompress_zstd(data),
            CompressionAlgo::Lz4 => self.decompress_lz4(data),
        }
    }

    fn compress_gzip(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        encoder
            .finish()
            .map_err(|e| MirrorMakerError::Compression(e.to_string()))
    }

    fn decompress_gzip(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(Vec::new());
        decoder.write_all(data)?;
        decoder
            .finish()
            .map_err(|e| MirrorMakerError::Compression(e.to_string()))
    }

    fn compress_snappy(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder = snap::write::FrameEncoder::new(Vec::new());
        encoder.write_all(data)?;
        encoder
            .into_inner()
            .map_err(|e| MirrorMakerError::Compression(e.to_string()))
    }

    fn decompress_snappy(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = snap::read::FrameDecoder::new(data);
        let mut output = Vec::new();
        std::io::copy(&mut decoder, &mut output)?;
        Ok(output)
    }

    fn compress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(data, 3).map_err(|e| MirrorMakerError::Compression(e.to_string()))
    }

    fn decompress_zstd(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(data).map_err(|e| MirrorMakerError::Compression(e.to_string()))
    }

    fn compress_lz4(&self, _data: &[u8]) -> Result<Vec<u8>> {
        // Note: LZ4 support requires additional crate
        Err(MirrorMakerError::Compression(
            "LZ4 not yet implemented".to_string(),
        ))
    }

    fn decompress_lz4(&self, _data: &[u8]) -> Result<Vec<u8>> {
        Err(MirrorMakerError::Compression(
            "LZ4 not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gzip_roundtrip() {
        let compressor = Compressor::new(CompressionType::Raw, CompressionAlgo::Gzip);
        let data = b"Hello, World! This is a test message.";

        let compressed = compressor.compress(data).unwrap();
        // Note: Small data may not compress smaller due to compression overhead
        assert!(!compressed.is_empty());

        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_snappy_roundtrip() {
        let compressor = Compressor::new(CompressionType::Raw, CompressionAlgo::Snappy);
        let data = b"Hello, World! This is a test message.";

        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }

    #[test]
    fn test_zstd_roundtrip() {
        let compressor = Compressor::new(CompressionType::Raw, CompressionAlgo::Zstd);
        let data = b"Hello, World! This is a test message.";

        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(data, decompressed.as_slice());
    }
}
