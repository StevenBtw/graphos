//! Write-Ahead Log (WAL) for durability.
//!
//! This module provides both synchronous and asynchronous WAL implementations:
//!
//! - [`WalManager`] - Synchronous WAL with blocking I/O (suitable for sync contexts)
//! - [`AsyncWalManager`] - Asynchronous WAL with tokio (suitable for async contexts)
//!
//! Both implementations support the same durability modes:
//!
//! - [`DurabilityMode::Sync`] - fsync after every commit (safest, slowest)
//! - [`DurabilityMode::Batch`] - periodic fsync based on time/records (balanced)
//! - [`DurabilityMode::NoSync`] - no fsync, rely on OS (fastest, least safe)

mod async_log;
mod log;
mod record;
mod recovery;

pub use async_log::AsyncWalManager;
pub use log::{CheckpointMetadata, DurabilityMode, WalConfig, WalManager};
pub use record::WalRecord;
pub use recovery::WalRecovery;
