//! GraphosDB main database struct.

use std::path::Path;
use std::sync::Arc;

use parking_lot::RwLock;

use graphos_adapters::storage::wal::{WalConfig, WalManager, WalRecord, WalRecovery};
use graphos_common::memory::buffer::{BufferManager, BufferManagerConfig};
use graphos_common::utils::error::Result;
use graphos_core::graph::lpg::LpgStore;
#[cfg(feature = "rdf")]
use graphos_core::graph::rdf::RdfStore;

use crate::config::Config;
use crate::session::Session;
use crate::transaction::TransactionManager;

/// The main Graphos database.
pub struct GraphosDB {
    /// Database configuration.
    config: Config,
    /// The underlying graph store.
    store: Arc<LpgStore>,
    /// RDF triple store (if RDF feature is enabled).
    #[cfg(feature = "rdf")]
    rdf_store: Arc<RdfStore>,
    /// Transaction manager.
    tx_manager: Arc<TransactionManager>,
    /// Unified buffer manager.
    buffer_manager: Arc<BufferManager>,
    /// Write-ahead log manager (if durability is enabled).
    wal: Option<Arc<WalManager>>,
    /// Whether the database is open.
    is_open: RwLock<bool>,
}

impl GraphosDB {
    /// Creates a new in-memory database.
    ///
    /// # Examples
    ///
    /// ```
    /// use graphos_engine::GraphosDB;
    ///
    /// let db = GraphosDB::new_in_memory();
    /// let session = db.session();
    /// ```
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self::with_config(Config::in_memory()).expect("In-memory database creation should not fail")
    }

    /// Opens or creates a database at the given path.
    ///
    /// If the database exists, it will be recovered from the WAL.
    /// If the database does not exist, a new one will be created.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or created.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use graphos_engine::GraphosDB;
    ///
    /// let db = GraphosDB::open("./my_database").expect("Failed to open database");
    /// ```
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::with_config(Config::persistent(path.as_ref()))
    }

    /// Creates a database with the given configuration.
    ///
    /// If WAL is enabled and a database exists at the configured path,
    /// the database will be recovered from the WAL.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created or recovery fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use graphos_engine::{GraphosDB, Config};
    ///
    /// let config = Config::in_memory()
    ///     .with_memory_limit(512 * 1024 * 1024); // 512MB
    ///
    /// let db = GraphosDB::with_config(config).unwrap();
    /// ```
    pub fn with_config(config: Config) -> Result<Self> {
        let store = Arc::new(LpgStore::new());
        #[cfg(feature = "rdf")]
        let rdf_store = Arc::new(RdfStore::new());
        let tx_manager = Arc::new(TransactionManager::new());

        // Create buffer manager with configured limits
        let buffer_config = BufferManagerConfig {
            budget: config.memory_limit.unwrap_or_else(|| {
                (BufferManagerConfig::detect_system_memory() as f64 * 0.75) as usize
            }),
            spill_path: config
                .spill_path
                .clone()
                .or_else(|| config.path.as_ref().map(|p| p.join("spill"))),
            ..BufferManagerConfig::default()
        };
        let buffer_manager = BufferManager::new(buffer_config);

        // Initialize WAL if persistence is enabled
        let wal = if config.wal_enabled {
            if let Some(ref db_path) = config.path {
                // Create database directory if it doesn't exist
                std::fs::create_dir_all(db_path)?;

                let wal_path = db_path.join("wal");

                // Check if WAL exists and recover if needed
                if wal_path.exists() {
                    let recovery = WalRecovery::new(&wal_path);
                    let records = recovery.recover()?;
                    Self::apply_wal_records(&store, &records)?;
                }

                // Open/create WAL manager
                let wal_config = WalConfig::default();
                let wal_manager = WalManager::with_config(&wal_path, wal_config)?;
                Some(Arc::new(wal_manager))
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            store,
            #[cfg(feature = "rdf")]
            rdf_store,
            tx_manager,
            buffer_manager,
            wal,
            is_open: RwLock::new(true),
        })
    }

    /// Applies WAL records to restore the database state.
    fn apply_wal_records(store: &LpgStore, records: &[WalRecord]) -> Result<()> {
        for record in records {
            match record {
                WalRecord::CreateNode { id, labels } => {
                    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                    store.create_node_with_id(*id, &label_refs);
                }
                WalRecord::DeleteNode { id } => {
                    store.delete_node(*id);
                }
                WalRecord::CreateEdge {
                    id,
                    src,
                    dst,
                    edge_type,
                } => {
                    store.create_edge_with_id(*id, *src, *dst, edge_type);
                }
                WalRecord::DeleteEdge { id } => {
                    store.delete_edge(*id);
                }
                WalRecord::SetNodeProperty { id, key, value } => {
                    store.set_node_property(*id, key, value.clone());
                }
                WalRecord::SetEdgeProperty { id, key, value } => {
                    store.set_edge_property(*id, key, value.clone());
                }
                WalRecord::TxCommit { .. }
                | WalRecord::TxAbort { .. }
                | WalRecord::Checkpoint { .. } => {
                    // Transaction control records don't need replay action
                    // (recovery already filtered to only committed transactions)
                }
            }
        }
        Ok(())
    }

    /// Creates a new session for interacting with the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use graphos_engine::GraphosDB;
    ///
    /// let db = GraphosDB::new_in_memory();
    /// let session = db.session();
    /// // Use session for queries and transactions
    /// ```
    #[must_use]
    pub fn session(&self) -> Session {
        Session::new(Arc::clone(&self.store), Arc::clone(&self.tx_manager))
    }

    /// Executes a query and returns the result.
    ///
    /// This is a convenience method that creates a session, executes the query,
    /// and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn execute(&self, query: &str) -> Result<QueryResult> {
        let session = self.session();
        session.execute(query)
    }

    /// Executes a query with parameters and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn execute_with_params(
        &self,
        query: &str,
        params: std::collections::HashMap<String, graphos_common::types::Value>,
    ) -> Result<QueryResult> {
        let session = self.session();
        session.execute_with_params(query, params)
    }

    /// Executes a Gremlin query and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[cfg(feature = "gremlin")]
    pub fn execute_gremlin(&self, query: &str) -> Result<QueryResult> {
        let session = self.session();
        session.execute_gremlin(query)
    }

    /// Executes a Gremlin query with parameters and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[cfg(feature = "gremlin")]
    pub fn execute_gremlin_with_params(
        &self,
        query: &str,
        params: std::collections::HashMap<String, graphos_common::types::Value>,
    ) -> Result<QueryResult> {
        let session = self.session();
        session.execute_gremlin_with_params(query, params)
    }

    /// Executes a GraphQL query and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[cfg(feature = "graphql")]
    pub fn execute_graphql(&self, query: &str) -> Result<QueryResult> {
        let session = self.session();
        session.execute_graphql(query)
    }

    /// Executes a GraphQL query with parameters and returns the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    #[cfg(feature = "graphql")]
    pub fn execute_graphql_with_params(
        &self,
        query: &str,
        params: std::collections::HashMap<String, graphos_common::types::Value>,
    ) -> Result<QueryResult> {
        let session = self.session();
        session.execute_graphql_with_params(query, params)
    }

    /// Executes a SPARQL query and returns the result.
    ///
    /// SPARQL queries operate on the RDF triple store.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use graphos_engine::GraphosDB;
    ///
    /// let db = GraphosDB::new_in_memory();
    /// let result = db.execute_sparql("SELECT ?s ?p ?o WHERE { ?s ?p ?o }")?;
    /// ```
    #[cfg(all(feature = "sparql", feature = "rdf"))]
    pub fn execute_sparql(&self, query: &str) -> Result<QueryResult> {
        use crate::query::{
            Executor, optimizer::Optimizer, planner_rdf::RdfPlanner, sparql_translator,
        };

        // Parse and translate the SPARQL query to a logical plan
        let logical_plan = sparql_translator::translate(query)?;

        // Optimize the plan
        let optimizer = Optimizer::new();
        let optimized_plan = optimizer.optimize(logical_plan)?;

        // Convert to physical plan using RDF planner
        let planner = RdfPlanner::new(Arc::clone(&self.rdf_store));
        let mut physical_plan = planner.plan(&optimized_plan)?;

        // Execute the plan
        let executor = Executor::with_columns(physical_plan.columns.clone());
        executor.execute(physical_plan.operator.as_mut())
    }

    /// Returns the RDF store.
    ///
    /// This provides direct access to the RDF store for triple operations.
    #[cfg(feature = "rdf")]
    #[must_use]
    pub fn rdf_store(&self) -> &Arc<RdfStore> {
        &self.rdf_store
    }

    /// Executes a query and returns a single scalar value.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or doesn't return exactly one row.
    pub fn query_scalar<T: FromValue>(&self, query: &str) -> Result<T> {
        let result = self.execute(query)?;
        result.scalar()
    }

    /// Returns the configuration.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Returns the underlying store.
    ///
    /// This provides direct access to the LPG store for algorithm implementations.
    #[must_use]
    pub fn store(&self) -> &Arc<LpgStore> {
        &self.store
    }

    /// Returns the buffer manager for memory-aware operations.
    #[must_use]
    pub fn buffer_manager(&self) -> &Arc<BufferManager> {
        &self.buffer_manager
    }

    /// Closes the database.
    ///
    /// This will:
    /// - Commit any pending WAL records
    /// - Create a checkpoint
    /// - Sync the WAL to disk
    ///
    /// # Errors
    ///
    /// Returns an error if the WAL cannot be flushed.
    pub fn close(&self) -> Result<()> {
        let mut is_open = self.is_open.write();
        if !*is_open {
            return Ok(());
        }

        // Commit and checkpoint WAL
        if let Some(ref wal) = self.wal {
            let epoch = self.store.current_epoch();

            // Use the last assigned transaction ID, or create a checkpoint-only tx
            let checkpoint_tx = self.tx_manager.last_assigned_tx_id().unwrap_or_else(|| {
                // No transactions have been started; begin one for checkpoint
                self.tx_manager.begin()
            });

            // Log a TxCommit to mark all pending records as committed
            wal.log(&WalRecord::TxCommit {
                tx_id: checkpoint_tx,
            })?;

            // Then checkpoint
            wal.checkpoint(checkpoint_tx, epoch)?;
            wal.sync()?;
        }

        *is_open = false;
        Ok(())
    }

    /// Returns the WAL manager if available.
    #[must_use]
    pub fn wal(&self) -> Option<&Arc<WalManager>> {
        self.wal.as_ref()
    }

    /// Logs a WAL record if WAL is enabled.
    fn log_wal(&self, record: &WalRecord) -> Result<()> {
        if let Some(ref wal) = self.wal {
            wal.log(record)?;
        }
        Ok(())
    }

    /// Returns the number of nodes in the database.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.store.node_count()
    }

    /// Returns the number of edges in the database.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.store.edge_count()
    }

    /// Returns the number of distinct labels in the database.
    #[must_use]
    pub fn label_count(&self) -> usize {
        self.store.label_count()
    }

    /// Returns the number of distinct property keys in the database.
    #[must_use]
    pub fn property_key_count(&self) -> usize {
        self.store.property_key_count()
    }

    /// Returns the number of distinct edge types in the database.
    #[must_use]
    pub fn edge_type_count(&self) -> usize {
        self.store.edge_type_count()
    }

    // === Node Operations ===

    /// Creates a new node with the given labels.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn create_node(&self, labels: &[&str]) -> graphos_common::types::NodeId {
        let id = self.store.create_node(labels);

        // Log to WAL if enabled
        if let Err(e) = self.log_wal(&WalRecord::CreateNode {
            id,
            labels: labels.iter().map(|s| s.to_string()).collect(),
        }) {
            tracing::warn!("Failed to log CreateNode to WAL: {}", e);
        }

        id
    }

    /// Creates a new node with labels and properties.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn create_node_with_props(
        &self,
        labels: &[&str],
        properties: impl IntoIterator<
            Item = (
                impl Into<graphos_common::types::PropertyKey>,
                impl Into<graphos_common::types::Value>,
            ),
        >,
    ) -> graphos_common::types::NodeId {
        // Collect properties first so we can log them to WAL
        let props: Vec<(
            graphos_common::types::PropertyKey,
            graphos_common::types::Value,
        )> = properties
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        let id = self
            .store
            .create_node_with_props(labels, props.iter().map(|(k, v)| (k.clone(), v.clone())));

        // Log node creation to WAL
        if let Err(e) = self.log_wal(&WalRecord::CreateNode {
            id,
            labels: labels.iter().map(|s| s.to_string()).collect(),
        }) {
            tracing::warn!("Failed to log CreateNode to WAL: {}", e);
        }

        // Log each property to WAL for full durability
        for (key, value) in props {
            if let Err(e) = self.log_wal(&WalRecord::SetNodeProperty {
                id,
                key: key.to_string(),
                value,
            }) {
                tracing::warn!("Failed to log SetNodeProperty to WAL: {}", e);
            }
        }

        id
    }

    /// Gets a node by ID.
    #[must_use]
    pub fn get_node(
        &self,
        id: graphos_common::types::NodeId,
    ) -> Option<graphos_core::graph::lpg::Node> {
        self.store.get_node(id)
    }

    /// Deletes a node and all its edges.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn delete_node(&self, id: graphos_common::types::NodeId) -> bool {
        let result = self.store.delete_node(id);

        if result {
            if let Err(e) = self.log_wal(&WalRecord::DeleteNode { id }) {
                tracing::warn!("Failed to log DeleteNode to WAL: {}", e);
            }
        }

        result
    }

    /// Sets a property on a node.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn set_node_property(
        &self,
        id: graphos_common::types::NodeId,
        key: &str,
        value: graphos_common::types::Value,
    ) {
        // Log to WAL first
        if let Err(e) = self.log_wal(&WalRecord::SetNodeProperty {
            id,
            key: key.to_string(),
            value: value.clone(),
        }) {
            tracing::warn!("Failed to log SetNodeProperty to WAL: {}", e);
        }

        self.store.set_node_property(id, key, value);
    }

    // === Edge Operations ===

    /// Creates a new edge between two nodes.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn create_edge(
        &self,
        src: graphos_common::types::NodeId,
        dst: graphos_common::types::NodeId,
        edge_type: &str,
    ) -> graphos_common::types::EdgeId {
        let id = self.store.create_edge(src, dst, edge_type);

        // Log to WAL if enabled
        if let Err(e) = self.log_wal(&WalRecord::CreateEdge {
            id,
            src,
            dst,
            edge_type: edge_type.to_string(),
        }) {
            tracing::warn!("Failed to log CreateEdge to WAL: {}", e);
        }

        id
    }

    /// Creates a new edge with properties.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn create_edge_with_props(
        &self,
        src: graphos_common::types::NodeId,
        dst: graphos_common::types::NodeId,
        edge_type: &str,
        properties: impl IntoIterator<
            Item = (
                impl Into<graphos_common::types::PropertyKey>,
                impl Into<graphos_common::types::Value>,
            ),
        >,
    ) -> graphos_common::types::EdgeId {
        // Collect properties first so we can log them to WAL
        let props: Vec<(
            graphos_common::types::PropertyKey,
            graphos_common::types::Value,
        )> = properties
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        let id = self.store.create_edge_with_props(
            src,
            dst,
            edge_type,
            props.iter().map(|(k, v)| (k.clone(), v.clone())),
        );

        // Log edge creation to WAL
        if let Err(e) = self.log_wal(&WalRecord::CreateEdge {
            id,
            src,
            dst,
            edge_type: edge_type.to_string(),
        }) {
            tracing::warn!("Failed to log CreateEdge to WAL: {}", e);
        }

        // Log each property to WAL for full durability
        for (key, value) in props {
            if let Err(e) = self.log_wal(&WalRecord::SetEdgeProperty {
                id,
                key: key.to_string(),
                value,
            }) {
                tracing::warn!("Failed to log SetEdgeProperty to WAL: {}", e);
            }
        }

        id
    }

    /// Gets an edge by ID.
    #[must_use]
    pub fn get_edge(
        &self,
        id: graphos_common::types::EdgeId,
    ) -> Option<graphos_core::graph::lpg::Edge> {
        self.store.get_edge(id)
    }

    /// Deletes an edge.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn delete_edge(&self, id: graphos_common::types::EdgeId) -> bool {
        let result = self.store.delete_edge(id);

        if result {
            if let Err(e) = self.log_wal(&WalRecord::DeleteEdge { id }) {
                tracing::warn!("Failed to log DeleteEdge to WAL: {}", e);
            }
        }

        result
    }

    /// Sets a property on an edge.
    ///
    /// If WAL is enabled, the operation is logged for durability.
    pub fn set_edge_property(
        &self,
        id: graphos_common::types::EdgeId,
        key: &str,
        value: graphos_common::types::Value,
    ) {
        // Log to WAL first
        if let Err(e) = self.log_wal(&WalRecord::SetEdgeProperty {
            id,
            key: key.to_string(),
            value: value.clone(),
        }) {
            tracing::warn!("Failed to log SetEdgeProperty to WAL: {}", e);
        }
        self.store.set_edge_property(id, key, value);
    }

    /// Removes a property from a node.
    ///
    /// Returns true if the property existed and was removed, false otherwise.
    pub fn remove_node_property(&self, id: graphos_common::types::NodeId, key: &str) -> bool {
        // Note: RemoveProperty WAL records not yet implemented, but operation works in memory
        self.store.remove_node_property(id, key).is_some()
    }

    /// Removes a property from an edge.
    ///
    /// Returns true if the property existed and was removed, false otherwise.
    pub fn remove_edge_property(&self, id: graphos_common::types::EdgeId, key: &str) -> bool {
        // Note: RemoveProperty WAL records not yet implemented, but operation works in memory
        self.store.remove_edge_property(id, key).is_some()
    }
}

impl Drop for GraphosDB {
    fn drop(&mut self) {
        if let Err(e) = self.close() {
            tracing::error!("Error closing database: {}", e);
        }
    }
}

/// Result of a query execution.
#[derive(Debug)]
pub struct QueryResult {
    /// Column names.
    pub columns: Vec<String>,
    /// Column types (used for distinguishing Node/Edge IDs from regular integers).
    pub column_types: Vec<graphos_common::types::LogicalType>,
    /// Result rows.
    pub rows: Vec<Vec<graphos_common::types::Value>>,
}

impl QueryResult {
    /// Creates a new empty query result.
    #[must_use]
    pub fn new(columns: Vec<String>) -> Self {
        let len = columns.len();
        Self {
            columns,
            column_types: vec![graphos_common::types::LogicalType::Any; len],
            rows: Vec::new(),
        }
    }

    /// Creates a new empty query result with column types.
    #[must_use]
    pub fn with_types(
        columns: Vec<String>,
        column_types: Vec<graphos_common::types::LogicalType>,
    ) -> Self {
        Self {
            columns,
            column_types,
            rows: Vec::new(),
        }
    }

    /// Returns the number of rows.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Returns the number of columns.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Returns true if the result is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Gets a single scalar value from the result.
    ///
    /// # Errors
    ///
    /// Returns an error if the result doesn't have exactly one row and one column.
    pub fn scalar<T: FromValue>(&self) -> Result<T> {
        if self.rows.len() != 1 || self.columns.len() != 1 {
            return Err(graphos_common::utils::error::Error::InvalidValue(
                "Expected single value".to_string(),
            ));
        }
        T::from_value(&self.rows[0][0])
    }

    /// Returns an iterator over the rows.
    pub fn iter(&self) -> impl Iterator<Item = &Vec<graphos_common::types::Value>> {
        self.rows.iter()
    }
}

/// Trait for converting from Value.
pub trait FromValue: Sized {
    /// Converts from a Value.
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion fails.
    fn from_value(value: &graphos_common::types::Value) -> Result<Self>;
}

impl FromValue for i64 {
    fn from_value(value: &graphos_common::types::Value) -> Result<Self> {
        value
            .as_int64()
            .ok_or_else(|| graphos_common::utils::error::Error::TypeMismatch {
                expected: "INT64".to_string(),
                found: value.type_name().to_string(),
            })
    }
}

impl FromValue for f64 {
    fn from_value(value: &graphos_common::types::Value) -> Result<Self> {
        value
            .as_float64()
            .ok_or_else(|| graphos_common::utils::error::Error::TypeMismatch {
                expected: "FLOAT64".to_string(),
                found: value.type_name().to_string(),
            })
    }
}

impl FromValue for String {
    fn from_value(value: &graphos_common::types::Value) -> Result<Self> {
        value.as_str().map(String::from).ok_or_else(|| {
            graphos_common::utils::error::Error::TypeMismatch {
                expected: "STRING".to_string(),
                found: value.type_name().to_string(),
            }
        })
    }
}

impl FromValue for bool {
    fn from_value(value: &graphos_common::types::Value) -> Result<Self> {
        value
            .as_bool()
            .ok_or_else(|| graphos_common::utils::error::Error::TypeMismatch {
                expected: "BOOL".to_string(),
                found: value.type_name().to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_in_memory_database() {
        let db = GraphosDB::new_in_memory();
        assert_eq!(db.node_count(), 0);
        assert_eq!(db.edge_count(), 0);
    }

    #[test]
    fn test_database_config() {
        let config = Config::in_memory().with_threads(4).with_query_logging();

        let db = GraphosDB::with_config(config).unwrap();
        assert_eq!(db.config().threads, 4);
        assert!(db.config().query_logging);
    }

    #[test]
    fn test_database_session() {
        let db = GraphosDB::new_in_memory();
        let _session = db.session();
        // Session should be created successfully
    }

    #[test]
    fn test_persistent_database_recovery() {
        use graphos_common::types::Value;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test_db");

        // Create database and add some data
        {
            let db = GraphosDB::open(&db_path).unwrap();

            let alice = db.create_node(&["Person"]);
            db.set_node_property(alice, "name", Value::from("Alice"));

            let bob = db.create_node(&["Person"]);
            db.set_node_property(bob, "name", Value::from("Bob"));

            let _edge = db.create_edge(alice, bob, "KNOWS");

            // Explicitly close to flush WAL
            db.close().unwrap();
        }

        // Reopen and verify data was recovered
        {
            let db = GraphosDB::open(&db_path).unwrap();

            assert_eq!(db.node_count(), 2);
            assert_eq!(db.edge_count(), 1);

            // Verify nodes exist
            let node0 = db.get_node(graphos_common::types::NodeId::new(0));
            assert!(node0.is_some());

            let node1 = db.get_node(graphos_common::types::NodeId::new(1));
            assert!(node1.is_some());
        }
    }

    #[test]
    fn test_wal_logging() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("wal_test_db");

        let db = GraphosDB::open(&db_path).unwrap();

        // Create some data
        let node = db.create_node(&["Test"]);
        db.delete_node(node);

        // WAL should have records
        if let Some(wal) = db.wal() {
            assert!(wal.record_count() > 0);
        }

        db.close().unwrap();
    }
}
