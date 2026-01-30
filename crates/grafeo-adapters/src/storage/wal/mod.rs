//! Write-Ahead Log - your safety net for crashes.
//!
//! Every mutation goes to the WAL before being applied to the main store.
//! If you crash mid-transaction, [`WalRecovery`] replays the log to restore
//! a consistent state. No committed data is lost.
//!
//! | Durability mode | What it does | When to use |
//! | --------------- | ------------ | ----------- |
//! | [`Sync`](DurabilityMode::Sync) | fsync after every commit | Can't lose any data |
//! | [`Batch`](DurabilityMode::Batch) | Periodic fsync | Balance of safety and speed |
//! | [`NoSync`](DurabilityMode::NoSync) | Let OS decide | Testing, when speed matters most |
//!
//! Choose [`WalManager`] for sync code, [`AsyncWalManager`] for async.

mod async_log;
mod log;
mod record;
mod recovery;

pub use async_log::AsyncWalManager;
pub use log::{CheckpointMetadata, DurabilityMode, WalConfig, WalManager};
pub use record::WalRecord;
pub use recovery::WalRecovery;
