//! Storage utilities for graph data.
//!
//! This module provides compression and encoding utilities:
//!
//! - [`dictionary`] - Dictionary encoding for strings with low cardinality
//! - [`delta`] - Delta encoding for sorted integer sequences
//! - [`bitpack`] - Bit-packing for small integers
//! - [`bitvec`] - Bit vector for boolean compression
//! - [`codec`] - Unified compression codec interface
//!
//! # Compression Strategies
//!
//! | Data Type | Recommended Codec | Compression Ratio |
//! |-----------|-------------------|-------------------|
//! | Sorted integers | DeltaBitPacked | 5-20x |
//! | Small integers | BitPacked | 2-16x |
//! | Strings (low cardinality) | Dictionary | 2-50x |
//! | Booleans | BitVector | 8x |
//!
//! # Example
//!
//! ```ignore
//! use graphos_core::storage::{TypeSpecificCompressor, CodecSelector};
//!
//! // Compress sorted integers
//! let values: Vec<u64> = (100..200).collect();
//! let compressed = TypeSpecificCompressor::compress_integers(&values);
//! println!("Compression ratio: {:.1}x", compressed.compression_ratio());
//!
//! // Compress booleans
//! let bools = vec![true, false, true, true, false];
//! let compressed = TypeSpecificCompressor::compress_booleans(&bools);
//! ```

pub mod bitpack;
pub mod bitvec;
pub mod codec;
pub mod delta;
pub mod dictionary;

// Re-export commonly used types
pub use bitpack::{BitPackedInts, DeltaBitPacked};
pub use bitvec::BitVector;
pub use codec::{CodecSelector, CompressedData, CompressionCodec, CompressionMetadata, TypeSpecificCompressor};
pub use delta::{zigzag_decode, zigzag_encode, DeltaEncoding};
pub use dictionary::{DictionaryBuilder, DictionaryEncoding};
