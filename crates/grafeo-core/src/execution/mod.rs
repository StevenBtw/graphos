//! Vectorized query execution engine.
//!
//! Grafeo uses vectorized processing - instead of one row at a time, we process
//! batches of ~1024 rows. This unlocks SIMD and keeps the CPU busy.
//!
//! | Module | Purpose |
//! | ------ | ------- |
//! | [`chunk`] | Batched rows (DataChunk = multiple columns) |
//! | [`vector`] | Single column of values |
//! | [`selection`] | Bitmap for filtering without copying |
//! | [`operators`] | Physical operators (scan, filter, join, etc.) |
//! | [`pipeline`] | Push-based execution (data flows through operators) |
//! | [`parallel`] | Morsel-driven parallelism |
//! | [`spill`] | Disk spilling when memory is tight |
//! | [`adaptive`] | Adaptive execution with runtime cardinality feedback |
//!
//! The execution model is push-based: sources push data through a pipeline of
//! operators until it reaches a sink.

pub mod adaptive;
pub mod chunk;
pub mod memory;
pub mod operators;
pub mod parallel;
pub mod pipeline;
pub mod selection;
pub mod sink;
pub mod source;
pub mod spill;
pub mod vector;

pub use adaptive::{
    AdaptiveCheckpoint, AdaptiveContext, AdaptiveEvent, AdaptiveExecutionConfig,
    AdaptiveExecutionResult, AdaptivePipelineBuilder, AdaptivePipelineConfig,
    AdaptivePipelineExecutor, AdaptiveSummary, CardinalityCheckpoint, CardinalityFeedback,
    CardinalityTrackingOperator, CardinalityTrackingSink, CardinalityTrackingWrapper,
    ReoptimizationDecision, SharedAdaptiveContext, evaluate_reoptimization, execute_adaptive,
};
pub use chunk::DataChunk;
pub use memory::{ExecutionMemoryContext, ExecutionMemoryContextBuilder};
pub use parallel::{
    CloneableOperatorFactory, MorselScheduler, ParallelPipeline, ParallelPipelineConfig,
    ParallelSource, RangeSource,
};
pub use pipeline::{ChunkCollector, ChunkSizeHint, Pipeline, PushOperator, Sink, Source};
pub use selection::SelectionVector;
pub use sink::{CollectorSink, CountingSink, LimitingSink, MaterializingSink, NullSink};
pub use source::{ChunkSource, EmptySource, GeneratorSource, OperatorSource, VectorSource};
pub use spill::{SpillFile, SpillFileReader, SpillManager};
pub use vector::ValueVector;
