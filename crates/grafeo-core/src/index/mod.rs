//! Index structures that make queries fast.
//!
//! Different indexes for different access patterns:
//!
//! - [`adjacency`] - Traversing neighbors (the bread and butter of graph queries)
//! - [`hash`] - Point lookups by ID or property value (O(1) average)
//! - [`btree`] - Range queries like "age > 30" (O(log n))
//! - [`trie`] - Multi-way joins for complex patterns (worst-case optimal)
//! - [`zone_map`] - Skip entire chunks when filtering (great for large scans)

pub mod adjacency;
pub mod btree;
pub mod hash;
pub mod trie;
pub mod zone_map;

pub use adjacency::ChunkedAdjacency;
pub use btree::BTreeIndex;
pub use hash::HashIndex;
pub use zone_map::{BloomFilter, ZoneMapBuilder, ZoneMapEntry, ZoneMapIndex};
