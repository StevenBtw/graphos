//! Physical operators that actually execute queries.
//!
//! These are the building blocks of query execution. The optimizer picks which
//! operators to use and how to wire them together.
//!
//! **Graph operators:**
//! - [`ScanOperator`] - Read nodes/edges from storage
//! - [`ExpandOperator`] - Traverse edges (the core of graph queries)
//! - [`VariableLengthExpandOperator`] - Paths of variable length
//! - [`ShortestPathOperator`] - Find shortest paths
//!
//! **Relational operators:**
//! - [`FilterOperator`] - Apply predicates
//! - [`ProjectOperator`] - Select/transform columns
//! - [`HashJoinOperator`] - Efficient equi-joins
//! - [`HashAggregateOperator`] - Group by with aggregation
//! - [`SortOperator`] - Order results
//! - [`LimitOperator`] - SKIP and LIMIT
//!
//! The [`push`] submodule has push-based variants for pipeline execution.

mod aggregate;
mod distinct;
mod expand;
mod filter;
mod join;
mod limit;
mod merge;
mod mutation;
mod project;
pub mod push;
mod scan;
mod shortest_path;
pub mod single_row;
mod sort;
mod union;
mod unwind;
mod variable_length_expand;

pub use aggregate::{
    AggregateExpr, AggregateFunction, HashAggregateOperator, SimpleAggregateOperator,
};
pub use distinct::DistinctOperator;
pub use expand::ExpandOperator;
pub use filter::{
    BinaryFilterOp, ExpressionPredicate, FilterExpression, FilterOperator, Predicate, UnaryFilterOp,
};
pub use join::{
    EqualityCondition, HashJoinOperator, HashKey, JoinCondition, JoinType, NestedLoopJoinOperator,
};
pub use limit::{LimitOperator, LimitSkipOperator, SkipOperator};
pub use merge::MergeOperator;
pub use mutation::{
    AddLabelOperator, CreateEdgeOperator, CreateNodeOperator, DeleteEdgeOperator,
    DeleteNodeOperator, PropertySource, RemoveLabelOperator, SetPropertyOperator,
};
pub use project::{ProjectExpr, ProjectOperator};
pub use push::{
    AggregatePushOperator, DistinctMaterializingOperator, DistinctPushOperator, FilterPushOperator,
    LimitPushOperator, ProjectPushOperator, SkipLimitPushOperator, SkipPushOperator,
    SortPushOperator, SpillableAggregatePushOperator, SpillableSortPushOperator,
};
pub use scan::ScanOperator;
pub use shortest_path::ShortestPathOperator;
pub use sort::{NullOrder, SortDirection, SortKey, SortOperator};
pub use union::UnionOperator;
pub use unwind::UnwindOperator;
pub use variable_length_expand::VariableLengthExpandOperator;

use thiserror::Error;

use super::DataChunk;

/// Result of executing an operator.
pub type OperatorResult = Result<Option<DataChunk>, OperatorError>;

/// Error during operator execution.
#[derive(Error, Debug, Clone)]
pub enum OperatorError {
    /// Type mismatch during execution.
    #[error("type mismatch: expected {expected}, found {found}")]
    TypeMismatch {
        /// Expected type name.
        expected: String,
        /// Found type name.
        found: String,
    },
    /// Column not found.
    #[error("column not found: {0}")]
    ColumnNotFound(String),
    /// Execution error.
    #[error("execution error: {0}")]
    Execution(String),
}

/// The core trait for pull-based operators.
///
/// Call [`next()`](Self::next) repeatedly until it returns `None`. Each call
/// returns a batch of rows (a DataChunk) or an error.
pub trait Operator: Send + Sync {
    /// Pulls the next batch of data. Returns `None` when exhausted.
    fn next(&mut self) -> OperatorResult;

    /// Resets to initial state so you can iterate again.
    fn reset(&mut self);

    /// Returns a name for debugging/explain output.
    fn name(&self) -> &'static str;
}
