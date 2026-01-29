//! Python bindings for Grafeo.
//!
//! This crate wraps the Rust graph database for Python users. You get the
//! same performance as the Rust API, with a Pythonic interface.
//!
//! ## Quick Start (Python)
//!
//! ```python
//! from grafeo import GrafeoDB
//!
//! # Create an in-memory database
//! db = GrafeoDB.new_in_memory()
//!
//! # Add some data
//! db.execute("INSERT (:Person {name: 'Alice', age: 30})")
//!
//! # Query it
//! result = db.execute("MATCH (p:Person) RETURN p.name")
//! for row in result:
//!     print(row)
//! ```
//!
//! ## Interop
//!
//! Grafeo plays nicely with the Python data science ecosystem:
//! - Convert to NetworkX for graph algorithms
//! - Export to pandas DataFrames
//! - Use with OR-Tools for optimization

#![warn(missing_docs)]

use pyo3::prelude::*;

mod bridges;
mod database;
mod error;
mod graph;
mod query;
mod types;

use bridges::{PyAlgorithms, PyNetworkXAdapter, PySolvORAdapter};
use database::{AsyncQueryResult, AsyncQueryResultIter, PyGrafeoDB};
use graph::{PyEdge, PyNode};
use query::PyQueryResult;
use types::PyValue;

/// Grafeo Python module.
#[pymodule]
fn grafeo(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGrafeoDB>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyEdge>()?;
    m.add_class::<PyQueryResult>()?;
    m.add_class::<AsyncQueryResult>()?;
    m.add_class::<AsyncQueryResultIter>()?;
    m.add_class::<PyValue>()?;
    m.add_class::<PyAlgorithms>()?;
    m.add_class::<PyNetworkXAdapter>()?;
    m.add_class::<PySolvORAdapter>()?;

    // Add version info
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}
