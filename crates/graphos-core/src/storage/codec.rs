//! Unified compression codec enumeration.
//!
//! Provides a unified interface for selecting and using compression codecs
//! based on data type and characteristics.
//!
//! # Supported Codecs
//!
//! | Codec | Best For | Compression |
//! |-------|----------|-------------|
//! | None | Small data, random access | 1x |
//! | Delta | Sorted integers | 2-10x |
//! | DeltaBitPacked | Sorted integers with small deltas | 5-20x |
//! | BitPacked | Small integers | 2-16x |
//! | Dictionary | Strings with low cardinality | 2-50x |
//! | BitVector | Booleans | 8x |
//! | RunLength | Highly repetitive data | 2-100x |

use std::io;

use super::bitpack::{BitPackedInts, DeltaBitPacked};
use super::bitvec::BitVector;

/// Compression codec identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompressionCodec {
    /// No compression (raw values).
    None,

    /// Delta encoding for integers.
    Delta,

    /// Bit packing for small integers.
    BitPacked {
        /// Number of bits per value.
        bits: u8,
    },

    /// Delta + bit packing for sorted integers.
    DeltaBitPacked {
        /// Number of bits per delta.
        bits: u8,
    },

    /// Dictionary encoding for strings.
    Dictionary,

    /// Bit vector for booleans.
    BitVector,

    /// Run-length encoding for repeated values.
    RunLength,
}

impl CompressionCodec {
    /// Returns a human-readable name for the codec.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Delta => "Delta",
            Self::BitPacked { .. } => "BitPacked",
            Self::DeltaBitPacked { .. } => "DeltaBitPacked",
            Self::Dictionary => "Dictionary",
            Self::BitVector => "BitVector",
            Self::RunLength => "RunLength",
        }
    }

    /// Returns whether this codec is lossless.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        // All our codecs are lossless
        true
    }
}

/// Compressed data container.
///
/// Holds compressed data along with metadata needed for decompression.
#[derive(Debug, Clone)]
pub struct CompressedData {
    /// The codec used for compression.
    pub codec: CompressionCodec,
    /// Size of the original uncompressed data in bytes.
    pub uncompressed_size: usize,
    /// Compressed data bytes.
    pub data: Vec<u8>,
    /// Additional metadata for decompression.
    pub metadata: CompressionMetadata,
}

/// Metadata needed for decompression.
#[derive(Debug, Clone)]
pub enum CompressionMetadata {
    /// No metadata needed.
    None,
    /// Delta encoding metadata.
    Delta {
        /// Base value (first value in sequence).
        base: i64,
    },
    /// Bit-packing metadata.
    BitPacked {
        /// Number of values.
        count: usize,
    },
    /// Delta + bit-packing metadata.
    DeltaBitPacked {
        /// Base value.
        base: i64,
        /// Number of values.
        count: usize,
    },
    /// Dictionary metadata.
    Dictionary {
        /// Dictionary identifier (for shared dictionaries).
        dict_id: u32,
    },
    /// Run-length metadata.
    RunLength {
        /// Number of runs.
        run_count: usize,
    },
}

impl CompressedData {
    /// Creates uncompressed data (no compression).
    pub fn uncompressed(data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            codec: CompressionCodec::None,
            uncompressed_size: size,
            data,
            metadata: CompressionMetadata::None,
        }
    }

    /// Returns the compression ratio (original / compressed).
    #[must_use]
    pub fn compression_ratio(&self) -> f64 {
        if self.data.is_empty() {
            return 1.0;
        }
        self.uncompressed_size as f64 / self.data.len() as f64
    }

    /// Returns whether the data is compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        !matches!(self.codec, CompressionCodec::None)
    }
}

/// Automatic codec selection based on data characteristics.
pub struct CodecSelector;

impl CodecSelector {
    /// Selects the best codec for a slice of u64 values.
    #[must_use]
    pub fn select_for_integers(values: &[u64]) -> CompressionCodec {
        if values.is_empty() {
            return CompressionCodec::None;
        }

        if values.len() < 8 {
            // Not worth compressing very small arrays
            return CompressionCodec::None;
        }

        // Check if sorted (ascending)
        let is_sorted = values.windows(2).all(|w| w[0] <= w[1]);

        if is_sorted {
            // Calculate deltas
            let deltas: Vec<u64> = values.windows(2).map(|w| w[1] - w[0]).collect();
            let max_delta = deltas.iter().copied().max().unwrap_or(0);
            let bits_needed = BitPackedInts::bits_needed(max_delta);

            return CompressionCodec::DeltaBitPacked { bits: bits_needed };
        }

        // Not sorted - try simple bit-packing
        let max_value = values.iter().copied().max().unwrap_or(0);
        let bits_needed = BitPackedInts::bits_needed(max_value);

        if bits_needed < 32 {
            CompressionCodec::BitPacked { bits: bits_needed }
        } else {
            CompressionCodec::None
        }
    }

    /// Selects the best codec for a slice of strings.
    #[must_use]
    pub fn select_for_strings(values: &[&str]) -> CompressionCodec {
        if values.is_empty() || values.len() < 4 {
            return CompressionCodec::None;
        }

        // Count unique values
        let unique: std::collections::HashSet<_> = values.iter().collect();
        let cardinality_ratio = unique.len() as f64 / values.len() as f64;

        // Dictionary is effective when cardinality is low
        if cardinality_ratio < 0.5 {
            CompressionCodec::Dictionary
        } else {
            CompressionCodec::None
        }
    }

    /// Selects the best codec for boolean values.
    #[must_use]
    pub fn select_for_booleans(_values: &[bool]) -> CompressionCodec {
        // BitVector is always the best choice for booleans
        CompressionCodec::BitVector
    }
}

/// Compressor that handles all supported data types.
pub struct TypeSpecificCompressor;

impl TypeSpecificCompressor {
    /// Compresses u64 values using the optimal codec.
    pub fn compress_integers(values: &[u64]) -> CompressedData {
        let codec = CodecSelector::select_for_integers(values);

        match codec {
            CompressionCodec::None => {
                let mut data = Vec::with_capacity(values.len() * 8);
                for &v in values {
                    data.extend_from_slice(&v.to_le_bytes());
                }
                CompressedData {
                    codec,
                    uncompressed_size: values.len() * 8,
                    data,
                    metadata: CompressionMetadata::None,
                }
            }
            CompressionCodec::DeltaBitPacked { bits } => {
                let encoded = DeltaBitPacked::encode(values);
                CompressedData {
                    codec: CompressionCodec::DeltaBitPacked { bits },
                    uncompressed_size: values.len() * 8,
                    data: encoded.to_bytes(),
                    metadata: CompressionMetadata::DeltaBitPacked {
                        base: encoded.base() as i64,
                        count: values.len(),
                    },
                }
            }
            CompressionCodec::BitPacked { bits } => {
                let packed = BitPackedInts::pack(values);
                CompressedData {
                    codec: CompressionCodec::BitPacked { bits },
                    uncompressed_size: values.len() * 8,
                    data: packed.to_bytes(),
                    metadata: CompressionMetadata::BitPacked {
                        count: values.len(),
                    },
                }
            }
            _ => unreachable!("Unexpected codec for integers"),
        }
    }

    /// Compresses i64 values using the optimal codec.
    pub fn compress_signed_integers(values: &[i64]) -> CompressedData {
        // Convert to u64 using zig-zag encoding
        let zigzag: Vec<u64> = values
            .iter()
            .map(|&v| super::delta::zigzag_encode(v))
            .collect();
        Self::compress_integers(&zigzag)
    }

    /// Compresses boolean values.
    pub fn compress_booleans(values: &[bool]) -> CompressedData {
        let bitvec = BitVector::from_bools(values);
        CompressedData {
            codec: CompressionCodec::BitVector,
            uncompressed_size: values.len(),
            data: bitvec.to_bytes(),
            metadata: CompressionMetadata::BitPacked {
                count: values.len(),
            },
        }
    }

    /// Decompresses u64 values.
    pub fn decompress_integers(data: &CompressedData) -> io::Result<Vec<u64>> {
        match data.codec {
            CompressionCodec::None => {
                let mut values = Vec::with_capacity(data.data.len() / 8);
                for chunk in data.data.chunks_exact(8) {
                    values.push(u64::from_le_bytes(chunk.try_into().unwrap()));
                }
                Ok(values)
            }
            CompressionCodec::DeltaBitPacked { .. } => {
                let encoded = DeltaBitPacked::from_bytes(&data.data)?;
                Ok(encoded.decode())
            }
            CompressionCodec::BitPacked { .. } => {
                let packed = BitPackedInts::from_bytes(&data.data)?;
                Ok(packed.unpack())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid codec for integer decompression",
            )),
        }
    }

    /// Decompresses boolean values.
    pub fn decompress_booleans(data: &CompressedData) -> io::Result<Vec<bool>> {
        match data.codec {
            CompressionCodec::BitVector => {
                let bitvec = BitVector::from_bytes(&data.data)?;
                Ok(bitvec.to_bools())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid codec for boolean decompression",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_selection_sorted_integers() {
        let sorted: Vec<u64> = (0..100).collect();
        let codec = CodecSelector::select_for_integers(&sorted);
        assert!(matches!(codec, CompressionCodec::DeltaBitPacked { .. }));
    }

    #[test]
    fn test_codec_selection_small_integers() {
        let small: Vec<u64> = vec![1, 5, 3, 7, 2, 4, 6, 8];
        let codec = CodecSelector::select_for_integers(&small);
        assert!(matches!(codec, CompressionCodec::BitPacked { .. }));
    }

    #[test]
    fn test_codec_selection_strings() {
        let repeated = vec!["a", "b", "a", "a", "b", "a", "c", "a"];
        let codec = CodecSelector::select_for_strings(&repeated);
        assert_eq!(codec, CompressionCodec::Dictionary);

        let unique = vec!["a", "b", "c", "d", "e", "f", "g", "h"];
        let codec = CodecSelector::select_for_strings(&unique);
        assert_eq!(codec, CompressionCodec::None);
    }

    #[test]
    fn test_codec_selection_booleans() {
        let bools = vec![true, false, true];
        let codec = CodecSelector::select_for_booleans(&bools);
        assert_eq!(codec, CompressionCodec::BitVector);
    }

    #[test]
    fn test_compress_decompress_sorted_integers() {
        let values: Vec<u64> = (100..200).collect();
        let compressed = TypeSpecificCompressor::compress_integers(&values);

        assert!(matches!(
            compressed.codec,
            CompressionCodec::DeltaBitPacked { .. }
        ));
        assert!(compressed.compression_ratio() > 1.0);

        let decompressed = TypeSpecificCompressor::decompress_integers(&compressed).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_compress_decompress_small_integers() {
        let values: Vec<u64> = vec![5, 2, 7, 1, 9, 3, 8, 4, 6, 0];
        let compressed = TypeSpecificCompressor::compress_integers(&values);

        let decompressed = TypeSpecificCompressor::decompress_integers(&compressed).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_compress_decompress_booleans() {
        let values = vec![true, false, true, true, false, false, true, false];
        let compressed = TypeSpecificCompressor::compress_booleans(&values);

        assert_eq!(compressed.codec, CompressionCodec::BitVector);

        let decompressed = TypeSpecificCompressor::decompress_booleans(&compressed).unwrap();
        assert_eq!(values, decompressed);
    }

    #[test]
    fn test_compression_ratio() {
        // 100 sequential values should compress well
        let values: Vec<u64> = (1000..1100).collect();
        let compressed = TypeSpecificCompressor::compress_integers(&values);

        let ratio = compressed.compression_ratio();
        assert!(ratio > 5.0, "Expected ratio > 5, got {}", ratio);
    }

    #[test]
    fn test_codec_names() {
        assert_eq!(CompressionCodec::None.name(), "None");
        assert_eq!(CompressionCodec::Delta.name(), "Delta");
        assert_eq!(CompressionCodec::BitPacked { bits: 4 }.name(), "BitPacked");
        assert_eq!(
            CompressionCodec::DeltaBitPacked { bits: 4 }.name(),
            "DeltaBitPacked"
        );
        assert_eq!(CompressionCodec::Dictionary.name(), "Dictionary");
        assert_eq!(CompressionCodec::BitVector.name(), "BitVector");
        assert_eq!(CompressionCodec::RunLength.name(), "RunLength");
    }
}
