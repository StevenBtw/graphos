//! Core type definitions for Graphos.
//!
//! This module contains all fundamental types used throughout the graph database:
//! - Identifier types ([`NodeId`], [`EdgeId`], [`TxId`], [`EpochId`])
//! - Property types ([`Value`], [`PropertyKey`], [`LogicalType`])
//! - Temporal types ([`Timestamp`])

mod id;
mod logical_type;
mod timestamp;
mod value;

pub use id::{EdgeId, EdgeTypeId, EpochId, IndexId, LabelId, NodeId, PropertyKeyId, TxId};
pub use logical_type::LogicalType;
pub use timestamp::Timestamp;
pub use value::{PropertyKey, Value};
