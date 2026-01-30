//! Common utilities used throughout Grafeo.
//!
//! - [`error`] - Error types like [`Error`] and [`QueryError`](error::QueryError)
//! - [`hash`] - Fast hashing with FxHash (non-cryptographic)

pub mod error;
pub mod hash;

pub use error::{Error, Result};
pub use hash::FxHasher;
