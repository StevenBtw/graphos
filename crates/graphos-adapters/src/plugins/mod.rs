//! Plugin system for Graphos.
//!
//! This module provides the plugin infrastructure and bridges to
//! external libraries.
//!
//! ## Modules
//!
//! - [`algorithms`] - Graph algorithms (BFS, DFS, components, centrality, etc.)

pub mod algorithms;
mod registry;
mod traits;

pub use registry::PluginRegistry;
pub use traits::{Algorithm, AlgorithmResult, ParameterDef, ParameterType, Parameters, Plugin};
