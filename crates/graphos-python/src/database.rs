//! Python database interface.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::RwLock;
use pyo3::prelude::*;
use pyo3_async_runtimes::tokio::future_into_py;

use graphos_common::types::{EdgeId, LogicalType, NodeId, Value};
use graphos_engine::config::Config;
use graphos_engine::database::{GraphosDB, QueryResult};

use crate::bridges::{PyAlgorithms, PyNetworkXAdapter, PySolvORAdapter};
use crate::error::PyGraphosError;
use crate::graph::{PyEdge, PyNode};
use crate::query::{PyQueryBuilder, PyQueryResult};
use crate::types::PyValue;

/// Result from async query execution.
///
/// This is a simpler result type used for async queries since we can't
/// safely extract nodes/edges from a non-Python context.
#[pyclass(name = "AsyncQueryResult")]
pub struct AsyncQueryResult {
    #[pyo3(get)]
    columns: Vec<String>,
    rows: Vec<Vec<Value>>,
    #[allow(dead_code)]
    column_types: Vec<LogicalType>,
}

#[pymethods]
impl AsyncQueryResult {
    /// Get all rows as a list of lists.
    fn rows(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let list = pyo3::types::PyList::empty(py);
        for row in &self.rows {
            let py_row = pyo3::types::PyList::empty(py);
            for val in row {
                let py_val = PyValue::to_py(val, py);
                py_row.append(py_val)?;
            }
            list.append(py_row)?;
        }
        Ok(list.into())
    }

    /// Get the number of rows.
    fn __len__(&self) -> usize {
        self.rows.len()
    }

    /// Iterate over rows.
    fn __iter__(slf: PyRef<'_, Self>) -> AsyncQueryResultIter {
        AsyncQueryResultIter {
            rows: slf.rows.clone(),
            index: 0,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "AsyncQueryResult(columns={:?}, rows={})",
            self.columns,
            self.rows.len()
        )
    }
}

/// Iterator for async query results.
#[pyclass]
pub struct AsyncQueryResultIter {
    rows: Vec<Vec<Value>>,
    index: usize,
}

#[pymethods]
impl AsyncQueryResultIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>, py: Python<'_>) -> Option<Py<PyAny>> {
        if slf.index >= slf.rows.len() {
            return None;
        }
        let row = slf.rows[slf.index].clone();
        slf.index += 1;

        let py_row = pyo3::types::PyList::empty(py);
        for val in &row {
            let py_val = PyValue::to_py(val, py);
            let _ = py_row.append(py_val);
        }
        Some(py_row.into())
    }
}

/// Python wrapper for GraphosDB.
#[pyclass(name = "GraphosDB")]
pub struct PyGraphosDB {
    inner: Arc<RwLock<GraphosDB>>,
}

#[pymethods]
impl PyGraphosDB {
    /// Create a new in-memory database.
    #[new]
    #[pyo3(signature = (path=None))]
    fn new(path: Option<String>) -> PyResult<Self> {
        let config = if let Some(p) = path {
            Config::persistent(p)
        } else {
            Config::in_memory()
        };

        let db = GraphosDB::with_config(config).map_err(PyGraphosError::from)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(db)),
        })
    }

    /// Open an existing database.
    #[staticmethod]
    fn open(path: String) -> PyResult<Self> {
        let config = Config::persistent(path);
        let db = GraphosDB::with_config(config).map_err(PyGraphosError::from)?;

        Ok(Self {
            inner: Arc::new(RwLock::new(db)),
        })
    }

    /// Execute a GQL query.
    #[pyo3(signature = (query, params=None))]
    fn execute(
        &self,
        query: &str,
        params: Option<&Bound<'_, pyo3::types::PyDict>>,
        _py: Python<'_>,
    ) -> PyResult<PyQueryResult> {
        let db = self.inner.read();

        let result = if let Some(p) = params {
            // Convert Python params to Rust HashMap
            let mut param_map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                param_map.insert(key_str, val);
            }
            db.execute_with_params(query, param_map).map_err(PyGraphosError::from)?
        } else {
            db.execute(query).map_err(PyGraphosError::from)?
        };

        // Extract nodes and edges based on column types
        let (nodes, edges) = extract_entities(&result, &db);

        Ok(PyQueryResult::new(
            result.columns,
            result.rows,
            nodes,
            edges,
        ))
    }

    /// Execute a query and return a query builder.
    fn query(&self, query: String) -> PyQueryBuilder {
        PyQueryBuilder::create(query)
    }

    /// Execute a GQL query asynchronously.
    ///
    /// This method returns a Python awaitable that can be used with asyncio.
    ///
    /// Example:
    /// ```python
    /// async def main():
    ///     db = GraphosDB()
    ///     result = await db.execute_async("MATCH (n:Person) RETURN n")
    ///     for row in result:
    ///         print(row)
    ///
    /// asyncio.run(main())
    /// ```
    #[pyo3(signature = (query, params=None))]
    fn execute_async<'py>(
        &self,
        py: Python<'py>,
        query: String,
        params: Option<&Bound<'py, pyo3::types::PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Convert params before the async block since they contain Python references
        let param_map: Option<HashMap<String, Value>> = if let Some(p) = params {
            let mut map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                map.insert(key_str, val);
            }
            Some(map)
        } else {
            None
        };

        let db = self.inner.clone();

        future_into_py(py, async move {
            // Perform the query execution in the async context
            // We use spawn_blocking since the actual db.execute is synchronous
            let result = tokio::task::spawn_blocking(move || {
                let db = db.read();
                if let Some(params) = param_map {
                    db.execute_with_params(&query, params)
                } else {
                    db.execute(&query)
                }
            })
            .await
            .map_err(|e| PyGraphosError::Database(e.to_string()))?
            .map_err(PyGraphosError::from)?;

            // Create PyQueryResult from the result
            // Note: We can't call extract_entities here because we don't have
            // Python references in the async context. We return raw data.
            Ok(AsyncQueryResult {
                columns: result.columns,
                rows: result.rows,
                column_types: result.column_types,
            })
        })
    }

    /// Execute a Gremlin query.
    #[cfg(feature = "gremlin")]
    #[pyo3(signature = (query, params=None))]
    fn execute_gremlin(
        &self,
        query: &str,
        params: Option<&Bound<'_, pyo3::types::PyDict>>,
        _py: Python<'_>,
    ) -> PyResult<PyQueryResult> {
        let db = self.inner.read();

        let result = if let Some(p) = params {
            // Convert Python params to Rust HashMap
            let mut param_map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                param_map.insert(key_str, val);
            }
            db.execute_gremlin_with_params(query, param_map).map_err(PyGraphosError::from)?
        } else {
            db.execute_gremlin(query).map_err(PyGraphosError::from)?
        };

        // Extract nodes and edges based on column types
        let (nodes, edges) = extract_entities(&result, &db);

        Ok(PyQueryResult::new(
            result.columns,
            result.rows,
            nodes,
            edges,
        ))
    }

    /// Execute a GraphQL query.
    #[cfg(feature = "graphql")]
    #[pyo3(signature = (query, params=None))]
    fn execute_graphql(
        &self,
        query: &str,
        params: Option<&Bound<'_, pyo3::types::PyDict>>,
        _py: Python<'_>,
    ) -> PyResult<PyQueryResult> {
        let db = self.inner.read();

        let result = if let Some(p) = params {
            // Convert Python params to Rust HashMap
            let mut param_map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                param_map.insert(key_str, val);
            }
            db.execute_graphql_with_params(query, param_map).map_err(PyGraphosError::from)?
        } else {
            db.execute_graphql(query).map_err(PyGraphosError::from)?
        };

        // Extract nodes and edges based on column types
        let (nodes, edges) = extract_entities(&result, &db);

        Ok(PyQueryResult::new(
            result.columns,
            result.rows,
            nodes,
            edges,
        ))
    }

    /// Execute a SPARQL query against the RDF triple store.
    ///
    /// SPARQL is the W3C standard query language for RDF data.
    ///
    /// Example:
    ///     result = db.execute_sparql("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")
    #[cfg(feature = "sparql")]
    #[pyo3(signature = (query, params=None))]
    fn execute_sparql(
        &self,
        query: &str,
        params: Option<&Bound<'_, pyo3::types::PyDict>>,
        _py: Python<'_>,
    ) -> PyResult<PyQueryResult> {
        let _params = if let Some(p) = params {
            let mut map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                map.insert(key_str, val);
            }
            map
        } else {
            HashMap::new()
        };

        let db = self.inner.read();
        let result = db.execute_sparql(query).map_err(PyGraphosError::from)?;

        // SPARQL results don't have LPG nodes/edges, so pass empty vectors
        Ok(PyQueryResult::new(
            result.columns,
            result.rows,
            Vec::new(),
            Vec::new(),
        ))
    }

    /// Create a node.
    #[pyo3(signature = (labels, properties=None))]
    fn create_node(
        &self,
        labels: Vec<String>,
        properties: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> PyResult<PyNode> {
        let db = self.inner.read();

        // Convert labels from Vec<String> to Vec<&str>
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();

        // Create node with or without properties
        let id = if let Some(p) = properties {
            // Convert properties
            let mut props: Vec<(
                graphos_common::types::PropertyKey,
                graphos_common::types::Value,
            )> = Vec::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                props.push((graphos_common::types::PropertyKey::new(key_str), val));
            }
            db.create_node_with_props(&label_refs, props)
        } else {
            db.create_node(&label_refs)
        };

        // Fetch the node back to get the full representation
        if let Some(node) = db.get_node(id) {
            let labels: Vec<String> = node.labels.iter().map(|s| s.to_string()).collect();
            let properties: HashMap<String, graphos_common::types::Value> = node
                .properties
                .into_iter()
                .map(|(k, v)| (k.as_str().to_string(), v))
                .collect();
            Ok(PyNode::new(id, labels, properties))
        } else {
            Err(PyGraphosError::Database("Failed to create node".into()).into())
        }
    }

    /// Create an edge between two nodes.
    #[pyo3(signature = (source_id, target_id, edge_type, properties=None))]
    fn create_edge(
        &self,
        source_id: u64,
        target_id: u64,
        edge_type: String,
        properties: Option<&Bound<'_, pyo3::types::PyDict>>,
    ) -> PyResult<PyEdge> {
        let db = self.inner.read();
        let src = NodeId(source_id);
        let dst = NodeId(target_id);

        // Create edge with or without properties
        let id = if let Some(p) = properties {
            // Convert properties
            let mut props: Vec<(
                graphos_common::types::PropertyKey,
                graphos_common::types::Value,
            )> = Vec::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                props.push((graphos_common::types::PropertyKey::new(key_str), val));
            }
            db.create_edge_with_props(src, dst, &edge_type, props)
        } else {
            db.create_edge(src, dst, &edge_type)
        };

        // Fetch the edge back to get the full representation
        if let Some(edge) = db.get_edge(id) {
            let properties: HashMap<String, graphos_common::types::Value> = edge
                .properties
                .into_iter()
                .map(|(k, v)| (k.as_str().to_string(), v))
                .collect();
            Ok(PyEdge::new(
                id,
                edge.edge_type.to_string(),
                edge.src,
                edge.dst,
                properties,
            ))
        } else {
            Err(PyGraphosError::Database("Failed to create edge".into()).into())
        }
    }

    /// Get a node by ID.
    fn get_node(&self, id: u64) -> PyResult<Option<PyNode>> {
        let db = self.inner.read();
        let node_id = NodeId(id);

        if let Some(node) = db.get_node(node_id) {
            let labels: Vec<String> = node.labels.iter().map(|s| s.to_string()).collect();
            let properties: HashMap<String, graphos_common::types::Value> = node
                .properties
                .into_iter()
                .map(|(k, v)| (k.as_str().to_string(), v))
                .collect();
            Ok(Some(PyNode::new(node_id, labels, properties)))
        } else {
            Ok(None)
        }
    }

    /// Get an edge by ID.
    fn get_edge(&self, id: u64) -> PyResult<Option<PyEdge>> {
        let db = self.inner.read();
        let edge_id = EdgeId(id);

        if let Some(edge) = db.get_edge(edge_id) {
            let properties: HashMap<String, graphos_common::types::Value> = edge
                .properties
                .into_iter()
                .map(|(k, v)| (k.as_str().to_string(), v))
                .collect();
            Ok(Some(PyEdge::new(
                edge_id,
                edge.edge_type.to_string(),
                edge.src,
                edge.dst,
                properties,
            )))
        } else {
            Ok(None)
        }
    }

    /// Delete a node by ID.
    fn delete_node(&self, id: u64) -> PyResult<bool> {
        let db = self.inner.read();
        Ok(db.delete_node(NodeId(id)))
    }

    /// Delete an edge by ID.
    fn delete_edge(&self, id: u64) -> PyResult<bool> {
        let db = self.inner.read();
        Ok(db.delete_edge(EdgeId(id)))
    }

    /// Set a property on a node.
    ///
    /// Example:
    /// ```python
    /// db.set_node_property(node_id, "name", "Alice")
    /// db.set_node_property(node_id, "age", 30)
    /// ```
    fn set_node_property(
        &self,
        node_id: u64,
        key: &str,
        value: &Bound<'_, pyo3::prelude::PyAny>,
    ) -> PyResult<()> {
        let db = self.inner.read();
        let val = PyValue::from_py(value).map_err(PyGraphosError::from)?;
        db.set_node_property(NodeId(node_id), key, val);
        Ok(())
    }

    /// Set a property on an edge.
    ///
    /// Example:
    /// ```python
    /// db.set_edge_property(edge_id, "weight", 1.5)
    /// db.set_edge_property(edge_id, "since", "2024-01-01")
    /// ```
    fn set_edge_property(
        &self,
        edge_id: u64,
        key: &str,
        value: &Bound<'_, pyo3::prelude::PyAny>,
    ) -> PyResult<()> {
        let db = self.inner.read();
        let val = PyValue::from_py(value).map_err(PyGraphosError::from)?;
        db.set_edge_property(EdgeId(edge_id), key, val);
        Ok(())
    }

    /// Remove a property from a node.
    ///
    /// Returns True if the property existed and was removed, False otherwise.
    ///
    /// Example:
    /// ```python
    /// if db.remove_node_property(node_id, "deprecated_field"):
    ///     print("Property removed")
    /// ```
    fn remove_node_property(&self, node_id: u64, key: &str) -> PyResult<bool> {
        let db = self.inner.read();
        Ok(db.remove_node_property(NodeId(node_id), key))
    }

    /// Remove a property from an edge.
    ///
    /// Returns True if the property existed and was removed, False otherwise.
    ///
    /// Example:
    /// ```python
    /// if db.remove_edge_property(edge_id, "temporary"):
    ///     print("Property removed")
    /// ```
    fn remove_edge_property(&self, edge_id: u64, key: &str) -> PyResult<bool> {
        let db = self.inner.read();
        Ok(db.remove_edge_property(EdgeId(edge_id), key))
    }

    /// Begin a transaction.
    ///
    /// Returns a Transaction object that can be used as a context manager.
    /// The transaction provides snapshot isolation - all queries within the
    /// transaction see a consistent view of the database.
    ///
    /// Example:
    /// ```python
    /// with db.begin_transaction() as tx:
    ///     tx.execute("CREATE (n:Person {name: 'Alice'})")
    ///     tx.execute("CREATE (n:Person {name: 'Bob'})")
    ///     tx.commit()  # Both nodes created atomically
    /// ```
    fn begin_transaction(&self) -> PyResult<PyTransaction> {
        PyTransaction::new(self.inner.clone())
    }

    /// Get database statistics.
    fn stats(&self) -> PyResult<PyDbStats> {
        let db = self.inner.read();
        Ok(PyDbStats {
            node_count: db.node_count() as u64,
            edge_count: db.edge_count() as u64,
            label_count: db.label_count() as u64,
            property_count: db.property_key_count() as u64,
        })
    }

    /// Close the database.
    fn close(&self) -> PyResult<()> {
        let db = self.inner.read();
        db.close().map_err(PyGraphosError::from)?;
        Ok(())
    }

    /// Get the algorithms interface.
    ///
    /// Returns an Algorithms object providing access to all graph algorithms.
    ///
    /// Example:
    ///     pr = db.algorithms.pagerank()
    ///     path = db.algorithms.dijkstra(1, 5)
    #[getter]
    fn algorithms(&self) -> PyAlgorithms {
        PyAlgorithms::new(self.inner.clone())
    }

    /// Get a NetworkX-compatible view of the graph.
    ///
    /// Args:
    ///     directed: Whether to treat as directed (default: True)
    ///
    /// Returns:
    ///     NetworkXAdapter that can be used with NetworkX algorithms
    ///     or converted to a NetworkX graph with to_networkx().
    ///
    /// Example:
    ///     nx_adapter = db.as_networkx()
    ///     G = nx_adapter.to_networkx()  # Convert to NetworkX graph
    ///     pr = nx_adapter.pagerank()    # Use native Graphos algorithms
    #[pyo3(signature = (directed=true))]
    fn as_networkx(&self, directed: bool) -> PyNetworkXAdapter {
        PyNetworkXAdapter::new(self.inner.clone(), directed)
    }

    /// Get a solvOR-compatible adapter for OR-style algorithms.
    ///
    /// Returns:
    ///     SolvORAdapter providing Operations Research style algorithms.
    ///
    /// Example:
    ///     solvor = db.as_solvor()
    ///     distance, path = solvor.shortest_path(1, 5)
    ///     result = solvor.max_flow(source=1, sink=10)
    fn as_solvor(&self) -> PySolvORAdapter {
        PySolvORAdapter::new(self.inner.clone())
    }

    /// Get number of nodes.
    #[getter]
    fn node_count(&self) -> usize {
        let db = self.inner.read();
        db.node_count()
    }

    /// Get number of edges.
    #[getter]
    fn edge_count(&self) -> usize {
        let db = self.inner.read();
        db.edge_count()
    }

    fn __repr__(&self) -> String {
        "GraphosDB()".to_string()
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        self.close()?;
        Ok(false)
    }
}

/// Transaction wrapper.
///
/// Use this as a context manager for transactional operations:
/// ```python
/// with db.begin_transaction() as tx:
///     tx.execute("CREATE (n:Person {name: 'Alice'})")
///     tx.commit()
/// ```
///
/// Changes are isolated until commit and automatically rolled back on exception.
#[pyclass(name = "Transaction")]
pub struct PyTransaction {
    db: Arc<RwLock<GraphosDB>>,
    session: parking_lot::Mutex<Option<graphos_engine::session::Session>>,
    committed: bool,
    rolled_back: bool,
}

impl PyTransaction {
    /// Create a new transaction, starting a Rust transaction internally.
    fn new(db: Arc<RwLock<GraphosDB>>) -> PyResult<Self> {
        // Create session from db, but drop the read guard before moving db
        let mut session = {
            let db_guard = db.read();
            db_guard.session()
        };

        // Begin the transaction in the Rust session
        session.begin_tx().map_err(PyGraphosError::from)?;

        Ok(Self {
            db,
            session: parking_lot::Mutex::new(Some(session)),
            committed: false,
            rolled_back: false,
        })
    }
}

#[pymethods]
impl PyTransaction {
    /// Commit the transaction.
    ///
    /// Makes all changes permanent. Raises an error if the transaction is
    /// already completed or if there's a write-write conflict.
    fn commit(&mut self) -> PyResult<()> {
        if self.committed || self.rolled_back {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Transaction already completed",
            ));
        }

        let mut session_guard = self.session.lock();
        if let Some(ref mut session) = *session_guard {
            session.commit().map_err(PyGraphosError::from)?;
        }
        *session_guard = None; // Drop the session
        self.committed = true;
        Ok(())
    }

    /// Rollback the transaction.
    ///
    /// Discards all changes made within this transaction.
    fn rollback(&mut self) -> PyResult<()> {
        if self.committed || self.rolled_back {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Transaction already completed",
            ));
        }

        let mut session_guard = self.session.lock();
        if let Some(ref mut session) = *session_guard {
            session.rollback().map_err(PyGraphosError::from)?;
        }
        *session_guard = None; // Drop the session
        self.rolled_back = true;
        Ok(())
    }

    /// Execute a query within this transaction.
    ///
    /// All queries executed through this method see the same snapshot
    /// and their changes are isolated until commit.
    #[pyo3(signature = (query, params=None))]
    fn execute(
        &self,
        query: &str,
        params: Option<&Bound<'_, pyo3::types::PyDict>>,
        _py: Python<'_>,
    ) -> PyResult<PyQueryResult> {
        if self.committed || self.rolled_back {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(
                "Cannot execute on completed transaction",
            ));
        }

        let db = self.db.read();
        let mut session_guard = self.session.lock();
        let session = session_guard.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("Transaction session not available")
        })?;

        let result = if let Some(p) = params {
            // Convert Python params to Rust HashMap
            let mut param_map = HashMap::new();
            for (key, value) in p.iter() {
                let key_str: String = key.extract()?;
                let val = PyValue::from_py(&value).map_err(PyGraphosError::from)?;
                param_map.insert(key_str, val);
            }
            session
                .execute_with_params(query, param_map)
                .map_err(PyGraphosError::from)?
        } else {
            session.execute(query).map_err(PyGraphosError::from)?
        };

        // Extract nodes and edges based on column types
        let (nodes, edges) = extract_entities(&result, &db);

        Ok(PyQueryResult::new(
            result.columns,
            result.rows,
            nodes,
            edges,
        ))
    }

    /// Check if transaction is active.
    #[getter]
    fn is_active(&self) -> bool {
        !self.committed && !self.rolled_back
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &mut self,
        exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        if !self.committed && !self.rolled_back {
            if exc_type.is_some() {
                self.rollback()?;
            } else {
                // Auto-commit on successful exit (no exception)
                self.commit()?;
            }
        }
        Ok(false)
    }

    fn __repr__(&self) -> String {
        let status = if self.committed {
            "committed"
        } else if self.rolled_back {
            "rolled_back"
        } else {
            "active"
        };
        format!("Transaction(status={})", status)
    }
}

/// Database statistics.
#[pyclass(name = "DbStats")]
pub struct PyDbStats {
    #[pyo3(get)]
    node_count: u64,
    #[pyo3(get)]
    edge_count: u64,
    #[pyo3(get)]
    label_count: u64,
    #[pyo3(get)]
    property_count: u64,
}

#[pymethods]
impl PyDbStats {
    fn __repr__(&self) -> String {
        format!(
            "DbStats(nodes={}, edges={}, labels={}, properties={})",
            self.node_count, self.edge_count, self.label_count, self.property_count
        )
    }
}

/// Extracts nodes and edges from a query result based on column types.
fn extract_entities(result: &QueryResult, db: &GraphosDB) -> (Vec<PyNode>, Vec<PyEdge>) {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut seen_node_ids = HashSet::new();
    let mut seen_edge_ids = HashSet::new();

    // Find which columns contain Node/Edge types
    let node_cols: Vec<usize> = result
        .column_types
        .iter()
        .enumerate()
        .filter_map(|(i, t)| if *t == LogicalType::Node { Some(i) } else { None })
        .collect();

    let edge_cols: Vec<usize> = result
        .column_types
        .iter()
        .enumerate()
        .filter_map(|(i, t)| if *t == LogicalType::Edge { Some(i) } else { None })
        .collect();

    // Extract unique nodes and edges from result rows
    for row in &result.rows {
        // Extract nodes
        for &col_idx in &node_cols {
            if let Some(Value::Int64(id)) = row.get(col_idx) {
                let node_id = NodeId(*id as u64);
                if !seen_node_ids.contains(&node_id) {
                    seen_node_ids.insert(node_id);
                    if let Some(node) = db.get_node(node_id) {
                        let labels: Vec<String> = node.labels.iter().map(|s| s.to_string()).collect();
                        let properties: HashMap<String, Value> = node
                            .properties
                            .into_iter()
                            .map(|(k, v)| (k.as_str().to_string(), v))
                            .collect();
                        nodes.push(PyNode::new(node_id, labels, properties));
                    }
                }
            }
        }

        // Extract edges
        for &col_idx in &edge_cols {
            if let Some(Value::Int64(id)) = row.get(col_idx) {
                let edge_id = EdgeId(*id as u64);
                if !seen_edge_ids.contains(&edge_id) {
                    seen_edge_ids.insert(edge_id);
                    if let Some(edge) = db.get_edge(edge_id) {
                        let properties: HashMap<String, Value> = edge
                            .properties
                            .into_iter()
                            .map(|(k, v)| (k.as_str().to_string(), v))
                            .collect();
                        edges.push(PyEdge::new(
                            edge_id,
                            edge.edge_type.to_string(),
                            edge.src,
                            edge.dst,
                            properties,
                        ));
                    }
                }
            }
        }
    }

    (nodes, edges)
}
