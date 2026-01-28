//! Transparent spilling for out-of-core query processing.
//!
//! This module provides infrastructure for spilling operator state to disk
//! when memory pressure is high, enabling queries to complete even when
//! intermediate results exceed available memory.
//!
//! # Architecture
//!
//! - [`SpillManager`] - Manages spill file lifecycle with automatic cleanup (sync)
//! - [`AsyncSpillManager`] - Async version using tokio for non-blocking I/O
//! - [`SpillFile`] - Read/write abstraction for individual spill files (sync)
//! - [`AsyncSpillFile`] - Async version using tokio
//! - Serializer functions for binary Value encoding (no serde overhead)
//! - [`ExternalSort`] - External merge sort for out-of-core sorting
//! - [`PartitionedState`] - Hash partitioning for spillable aggregation

mod async_file;
mod async_manager;
mod external_sort;
mod file;
mod manager;
mod partition;
mod serializer;

pub use async_file::{AsyncSpillFile, AsyncSpillFileReader};
pub use async_manager::AsyncSpillManager;
pub use external_sort::{ExternalSort, NullOrder, SortDirection, SortKey};
pub use file::{SpillFile, SpillFileReader};
pub use manager::SpillManager;
pub use partition::{DEFAULT_NUM_PARTITIONS, PartitionedState};
pub use serializer::{deserialize_row, deserialize_value, serialize_row, serialize_value};
