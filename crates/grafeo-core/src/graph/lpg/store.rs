//! The in-memory LPG graph store.
//!
//! This is where nodes and edges actually live. Most users interact with
//! this through [`GrafeoDB`](grafeo_engine::GrafeoDB), but if you need
//! direct access for algorithms, here it is.

use super::property::CompareOp;
use super::{Edge, EdgeRecord, Node, NodeRecord, PropertyStorage};
use crate::graph::Direction;
use crate::index::adjacency::ChunkedAdjacency;
use crate::index::zone_map::ZoneMapEntry;
use crate::statistics::{EdgeTypeStatistics, LabelStatistics, Statistics};
use grafeo_common::mvcc::VersionChain;
use grafeo_common::types::{EdgeId, EpochId, NodeId, PropertyKey, TxId, Value};
use grafeo_common::utils::hash::{FxHashMap, FxHashSet};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Configuration for the LPG store.
#[derive(Debug, Clone)]
pub struct LpgStoreConfig {
    /// Whether to maintain backward adjacency lists.
    pub backward_edges: bool,
    /// Initial capacity for nodes.
    pub initial_node_capacity: usize,
    /// Initial capacity for edges.
    pub initial_edge_capacity: usize,
}

impl Default for LpgStoreConfig {
    fn default() -> Self {
        Self {
            backward_edges: true,
            initial_node_capacity: 1024,
            initial_edge_capacity: 4096,
        }
    }
}

/// The core LPG graph storage.
///
/// Holds all nodes, edges, properties, and indexes in memory. Designed for
/// concurrent reads with MVCC versioning so queries don't block each other.
///
/// For most use cases, interact through [`GrafeoDB`](grafeo_engine::GrafeoDB).
/// Direct access is useful for algorithm implementations that need raw
/// adjacency traversal.
pub struct LpgStore {
    /// Configuration.
    #[allow(dead_code)]
    config: LpgStoreConfig,

    /// Node records indexed by NodeId, with version chains for MVCC.
    nodes: RwLock<FxHashMap<NodeId, VersionChain<NodeRecord>>>,

    /// Edge records indexed by EdgeId, with version chains for MVCC.
    edges: RwLock<FxHashMap<EdgeId, VersionChain<EdgeRecord>>>,

    /// Property storage for nodes.
    node_properties: PropertyStorage<NodeId>,

    /// Property storage for edges.
    edge_properties: PropertyStorage<EdgeId>,

    /// Label name to ID mapping.
    label_to_id: RwLock<FxHashMap<Arc<str>, u32>>,

    /// Label ID to name mapping.
    id_to_label: RwLock<Vec<Arc<str>>>,

    /// Edge type name to ID mapping.
    edge_type_to_id: RwLock<FxHashMap<Arc<str>, u32>>,

    /// Edge type ID to name mapping.
    id_to_edge_type: RwLock<Vec<Arc<str>>>,

    /// Forward adjacency lists (outgoing edges).
    forward_adj: ChunkedAdjacency,

    /// Backward adjacency lists (incoming edges).
    /// Only populated if config.backward_edges is true.
    backward_adj: Option<ChunkedAdjacency>,

    /// Label index: label_id -> set of node IDs.
    label_index: RwLock<Vec<FxHashMap<NodeId, ()>>>,

    /// Node labels: node_id -> set of label IDs.
    /// Reverse mapping to efficiently get labels for a node.
    node_labels: RwLock<FxHashMap<NodeId, FxHashSet<u32>>>,

    /// Next node ID.
    next_node_id: AtomicU64,

    /// Next edge ID.
    next_edge_id: AtomicU64,

    /// Current epoch.
    current_epoch: AtomicU64,

    /// Statistics for cost-based optimization.
    statistics: RwLock<Statistics>,
}

impl LpgStore {
    /// Creates a new LPG store with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(LpgStoreConfig::default())
    }

    /// Creates a new LPG store with custom configuration.
    #[must_use]
    pub fn with_config(config: LpgStoreConfig) -> Self {
        let backward_adj = if config.backward_edges {
            Some(ChunkedAdjacency::new())
        } else {
            None
        };

        Self {
            nodes: RwLock::new(FxHashMap::default()),
            edges: RwLock::new(FxHashMap::default()),
            node_properties: PropertyStorage::new(),
            edge_properties: PropertyStorage::new(),
            label_to_id: RwLock::new(FxHashMap::default()),
            id_to_label: RwLock::new(Vec::new()),
            edge_type_to_id: RwLock::new(FxHashMap::default()),
            id_to_edge_type: RwLock::new(Vec::new()),
            forward_adj: ChunkedAdjacency::new(),
            backward_adj,
            label_index: RwLock::new(Vec::new()),
            node_labels: RwLock::new(FxHashMap::default()),
            next_node_id: AtomicU64::new(0),
            next_edge_id: AtomicU64::new(0),
            current_epoch: AtomicU64::new(0),
            statistics: RwLock::new(Statistics::new()),
            config,
        }
    }

    /// Returns the current epoch.
    #[must_use]
    pub fn current_epoch(&self) -> EpochId {
        EpochId::new(self.current_epoch.load(Ordering::Acquire))
    }

    /// Creates a new epoch.
    pub fn new_epoch(&self) -> EpochId {
        let id = self.current_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        EpochId::new(id)
    }

    // === Node Operations ===

    /// Creates a new node with the given labels.
    ///
    /// Uses the system transaction for non-transactional operations.
    pub fn create_node(&self, labels: &[&str]) -> NodeId {
        self.create_node_versioned(labels, self.current_epoch(), TxId::SYSTEM)
    }

    /// Creates a new node with the given labels within a transaction context.
    pub fn create_node_versioned(&self, labels: &[&str], epoch: EpochId, tx_id: TxId) -> NodeId {
        let id = NodeId::new(self.next_node_id.fetch_add(1, Ordering::Relaxed));

        let mut record = NodeRecord::new(id, epoch);
        record.set_label_count(labels.len() as u16);

        // Store labels in node_labels map and label_index
        let mut node_label_set = FxHashSet::default();
        for label in labels {
            let label_id = self.get_or_create_label_id(*label);
            node_label_set.insert(label_id);

            // Update label index
            let mut index = self.label_index.write();
            while index.len() <= label_id as usize {
                index.push(FxHashMap::default());
            }
            index[label_id as usize].insert(id, ());
        }

        // Store node's labels
        self.node_labels.write().insert(id, node_label_set);

        // Create version chain with initial version
        let chain = VersionChain::with_initial(record, epoch, tx_id);
        self.nodes.write().insert(id, chain);
        id
    }

    /// Creates a new node with labels and properties.
    pub fn create_node_with_props(
        &self,
        labels: &[&str],
        properties: impl IntoIterator<Item = (impl Into<PropertyKey>, impl Into<Value>)>,
    ) -> NodeId {
        self.create_node_with_props_versioned(
            labels,
            properties,
            self.current_epoch(),
            TxId::SYSTEM,
        )
    }

    /// Creates a new node with labels and properties within a transaction context.
    pub fn create_node_with_props_versioned(
        &self,
        labels: &[&str],
        properties: impl IntoIterator<Item = (impl Into<PropertyKey>, impl Into<Value>)>,
        epoch: EpochId,
        tx_id: TxId,
    ) -> NodeId {
        let id = self.create_node_versioned(labels, epoch, tx_id);

        for (key, value) in properties {
            self.node_properties.set(id, key.into(), value.into());
        }

        // Update props_count in record
        let count = self.node_properties.get_all(id).len() as u16;
        if let Some(chain) = self.nodes.write().get_mut(&id) {
            if let Some(record) = chain.latest_mut() {
                record.props_count = count;
            }
        }

        id
    }

    /// Gets a node by ID (latest visible version).
    #[must_use]
    pub fn get_node(&self, id: NodeId) -> Option<Node> {
        self.get_node_at_epoch(id, self.current_epoch())
    }

    /// Gets a node by ID at a specific epoch.
    #[must_use]
    pub fn get_node_at_epoch(&self, id: NodeId, epoch: EpochId) -> Option<Node> {
        let nodes = self.nodes.read();
        let chain = nodes.get(&id)?;
        let record = chain.visible_at(epoch)?;

        if record.is_deleted() {
            return None;
        }

        let mut node = Node::new(id);

        // Get labels from node_labels map
        let id_to_label = self.id_to_label.read();
        let node_labels = self.node_labels.read();
        if let Some(label_ids) = node_labels.get(&id) {
            for &label_id in label_ids {
                if let Some(label) = id_to_label.get(label_id as usize) {
                    node.labels.push(label.clone());
                }
            }
        }

        // Get properties
        node.properties = self.node_properties.get_all(id).into_iter().collect();

        Some(node)
    }

    /// Gets a node visible to a specific transaction.
    #[must_use]
    pub fn get_node_versioned(&self, id: NodeId, epoch: EpochId, tx_id: TxId) -> Option<Node> {
        let nodes = self.nodes.read();
        let chain = nodes.get(&id)?;
        let record = chain.visible_to(epoch, tx_id)?;

        if record.is_deleted() {
            return None;
        }

        let mut node = Node::new(id);

        // Get labels from node_labels map
        let id_to_label = self.id_to_label.read();
        let node_labels = self.node_labels.read();
        if let Some(label_ids) = node_labels.get(&id) {
            for &label_id in label_ids {
                if let Some(label) = id_to_label.get(label_id as usize) {
                    node.labels.push(label.clone());
                }
            }
        }

        // Get properties
        node.properties = self.node_properties.get_all(id).into_iter().collect();

        Some(node)
    }

    /// Deletes a node and all its edges (using latest epoch).
    pub fn delete_node(&self, id: NodeId) -> bool {
        self.delete_node_at_epoch(id, self.current_epoch())
    }

    /// Deletes a node at a specific epoch.
    pub fn delete_node_at_epoch(&self, id: NodeId, epoch: EpochId) -> bool {
        let mut nodes = self.nodes.write();
        if let Some(chain) = nodes.get_mut(&id) {
            // Check if visible at this epoch (not already deleted)
            if let Some(record) = chain.visible_at(epoch) {
                if record.is_deleted() {
                    return false;
                }
            } else {
                // Not visible at this epoch (already deleted or doesn't exist)
                return false;
            }

            // Mark the version chain as deleted at this epoch
            chain.mark_deleted(epoch);

            // Remove from label index using node_labels map
            let mut index = self.label_index.write();
            let mut node_labels = self.node_labels.write();
            if let Some(label_ids) = node_labels.remove(&id) {
                for label_id in label_ids {
                    if let Some(set) = index.get_mut(label_id as usize) {
                        set.remove(&id);
                    }
                }
            }

            // Remove properties
            drop(nodes); // Release lock before removing properties
            drop(index);
            drop(node_labels);
            self.node_properties.remove_all(id);

            // Note: Caller should use delete_node_edges() first if detach is needed

            true
        } else {
            false
        }
    }

    /// Deletes all edges connected to a node (for DETACH DELETE).
    pub fn delete_node_edges(&self, node_id: NodeId) {
        // Get outgoing edges
        let outgoing: Vec<EdgeId> = self
            .forward_adj
            .edges_from(node_id)
            .into_iter()
            .map(|(_, edge_id)| edge_id)
            .collect();

        // Get incoming edges
        let incoming: Vec<EdgeId> = if let Some(ref backward) = self.backward_adj {
            backward
                .edges_from(node_id)
                .into_iter()
                .map(|(_, edge_id)| edge_id)
                .collect()
        } else {
            // No backward adjacency - scan all edges
            let epoch = self.current_epoch();
            self.edges
                .read()
                .iter()
                .filter_map(|(id, chain)| {
                    chain.visible_at(epoch).and_then(|r| {
                        if !r.is_deleted() && r.dst == node_id {
                            Some(*id)
                        } else {
                            None
                        }
                    })
                })
                .collect()
        };

        // Delete all edges
        for edge_id in outgoing.into_iter().chain(incoming) {
            self.delete_edge(edge_id);
        }
    }

    /// Sets a property on a node.
    pub fn set_node_property(&self, id: NodeId, key: &str, value: Value) {
        self.node_properties.set(id, key.into(), value);

        // Update props_count in record
        let count = self.node_properties.get_all(id).len() as u16;
        if let Some(chain) = self.nodes.write().get_mut(&id) {
            if let Some(record) = chain.latest_mut() {
                record.props_count = count;
            }
        }
    }

    /// Sets a property on an edge.
    pub fn set_edge_property(&self, id: EdgeId, key: &str, value: Value) {
        self.edge_properties.set(id, key.into(), value);
    }

    /// Removes a property from a node.
    ///
    /// Returns the previous value if it existed, or None if the property didn't exist.
    pub fn remove_node_property(&self, id: NodeId, key: &str) -> Option<Value> {
        let result = self.node_properties.remove(id, &key.into());

        // Update props_count in record
        let count = self.node_properties.get_all(id).len() as u16;
        if let Some(chain) = self.nodes.write().get_mut(&id) {
            if let Some(record) = chain.latest_mut() {
                record.props_count = count;
            }
        }

        result
    }

    /// Removes a property from an edge.
    ///
    /// Returns the previous value if it existed, or None if the property didn't exist.
    pub fn remove_edge_property(&self, id: EdgeId, key: &str) -> Option<Value> {
        self.edge_properties.remove(id, &key.into())
    }

    /// Adds a label to a node.
    ///
    /// Returns true if the label was added, false if the node doesn't exist
    /// or already has the label.
    pub fn add_label(&self, node_id: NodeId, label: &str) -> bool {
        let epoch = self.current_epoch();

        // Check if node exists
        let nodes = self.nodes.read();
        if let Some(chain) = nodes.get(&node_id) {
            if chain.visible_at(epoch).map_or(true, |r| r.is_deleted()) {
                return false;
            }
        } else {
            return false;
        }
        drop(nodes);

        // Get or create label ID
        let label_id = self.get_or_create_label_id(label);

        // Add to node_labels map
        let mut node_labels = self.node_labels.write();
        let label_set = node_labels
            .entry(node_id)
            .or_insert_with(FxHashSet::default);

        if label_set.contains(&label_id) {
            return false; // Already has this label
        }

        label_set.insert(label_id);
        drop(node_labels);

        // Add to label_index
        let mut index = self.label_index.write();
        if (label_id as usize) >= index.len() {
            index.resize(label_id as usize + 1, FxHashMap::default());
        }
        index[label_id as usize].insert(node_id, ());

        // Update label count in node record
        if let Some(chain) = self.nodes.write().get_mut(&node_id) {
            if let Some(record) = chain.latest_mut() {
                let count = self.node_labels.read().get(&node_id).map_or(0, |s| s.len());
                record.set_label_count(count as u16);
            }
        }

        true
    }

    /// Removes a label from a node.
    ///
    /// Returns true if the label was removed, false if the node doesn't exist
    /// or doesn't have the label.
    pub fn remove_label(&self, node_id: NodeId, label: &str) -> bool {
        let epoch = self.current_epoch();

        // Check if node exists
        let nodes = self.nodes.read();
        if let Some(chain) = nodes.get(&node_id) {
            if chain.visible_at(epoch).map_or(true, |r| r.is_deleted()) {
                return false;
            }
        } else {
            return false;
        }
        drop(nodes);

        // Get label ID
        let label_id = {
            let label_ids = self.label_to_id.read();
            match label_ids.get(label) {
                Some(&id) => id,
                None => return false, // Label doesn't exist
            }
        };

        // Remove from node_labels map
        let mut node_labels = self.node_labels.write();
        if let Some(label_set) = node_labels.get_mut(&node_id) {
            if !label_set.remove(&label_id) {
                return false; // Node doesn't have this label
            }
        } else {
            return false;
        }
        drop(node_labels);

        // Remove from label_index
        let mut index = self.label_index.write();
        if (label_id as usize) < index.len() {
            index[label_id as usize].remove(&node_id);
        }

        // Update label count in node record
        if let Some(chain) = self.nodes.write().get_mut(&node_id) {
            if let Some(record) = chain.latest_mut() {
                let count = self.node_labels.read().get(&node_id).map_or(0, |s| s.len());
                record.set_label_count(count as u16);
            }
        }

        true
    }

    /// Returns the number of nodes (non-deleted at current epoch).
    #[must_use]
    pub fn node_count(&self) -> usize {
        let epoch = self.current_epoch();
        self.nodes
            .read()
            .values()
            .filter_map(|chain| chain.visible_at(epoch))
            .filter(|r| !r.is_deleted())
            .count()
    }

    /// Returns all node IDs in the store.
    ///
    /// This returns a snapshot of current node IDs. The returned vector
    /// excludes deleted nodes.
    #[must_use]
    pub fn node_ids(&self) -> Vec<NodeId> {
        let epoch = self.current_epoch();
        self.nodes
            .read()
            .iter()
            .filter_map(|(id, chain)| {
                chain
                    .visible_at(epoch)
                    .and_then(|r| if !r.is_deleted() { Some(*id) } else { None })
            })
            .collect()
    }

    // === Edge Operations ===

    /// Creates a new edge.
    pub fn create_edge(&self, src: NodeId, dst: NodeId, edge_type: &str) -> EdgeId {
        self.create_edge_versioned(src, dst, edge_type, self.current_epoch(), TxId::SYSTEM)
    }

    /// Creates a new edge within a transaction context.
    pub fn create_edge_versioned(
        &self,
        src: NodeId,
        dst: NodeId,
        edge_type: &str,
        epoch: EpochId,
        tx_id: TxId,
    ) -> EdgeId {
        let id = EdgeId::new(self.next_edge_id.fetch_add(1, Ordering::Relaxed));
        let type_id = self.get_or_create_edge_type_id(edge_type);

        let record = EdgeRecord::new(id, src, dst, type_id, epoch);
        let chain = VersionChain::with_initial(record, epoch, tx_id);
        self.edges.write().insert(id, chain);

        // Update adjacency
        self.forward_adj.add_edge(src, dst, id);
        if let Some(ref backward) = self.backward_adj {
            backward.add_edge(dst, src, id);
        }

        id
    }

    /// Creates a new edge with properties.
    pub fn create_edge_with_props(
        &self,
        src: NodeId,
        dst: NodeId,
        edge_type: &str,
        properties: impl IntoIterator<Item = (impl Into<PropertyKey>, impl Into<Value>)>,
    ) -> EdgeId {
        let id = self.create_edge(src, dst, edge_type);

        for (key, value) in properties {
            self.edge_properties.set(id, key.into(), value.into());
        }

        id
    }

    /// Gets an edge by ID (latest visible version).
    #[must_use]
    pub fn get_edge(&self, id: EdgeId) -> Option<Edge> {
        self.get_edge_at_epoch(id, self.current_epoch())
    }

    /// Gets an edge by ID at a specific epoch.
    #[must_use]
    pub fn get_edge_at_epoch(&self, id: EdgeId, epoch: EpochId) -> Option<Edge> {
        let edges = self.edges.read();
        let chain = edges.get(&id)?;
        let record = chain.visible_at(epoch)?;

        if record.is_deleted() {
            return None;
        }

        let edge_type = {
            let id_to_type = self.id_to_edge_type.read();
            id_to_type.get(record.type_id as usize)?.clone()
        };

        let mut edge = Edge::new(id, record.src, record.dst, edge_type);

        // Get properties
        edge.properties = self.edge_properties.get_all(id).into_iter().collect();

        Some(edge)
    }

    /// Gets an edge visible to a specific transaction.
    #[must_use]
    pub fn get_edge_versioned(&self, id: EdgeId, epoch: EpochId, tx_id: TxId) -> Option<Edge> {
        let edges = self.edges.read();
        let chain = edges.get(&id)?;
        let record = chain.visible_to(epoch, tx_id)?;

        if record.is_deleted() {
            return None;
        }

        let edge_type = {
            let id_to_type = self.id_to_edge_type.read();
            id_to_type.get(record.type_id as usize)?.clone()
        };

        let mut edge = Edge::new(id, record.src, record.dst, edge_type);

        // Get properties
        edge.properties = self.edge_properties.get_all(id).into_iter().collect();

        Some(edge)
    }

    /// Deletes an edge (using latest epoch).
    pub fn delete_edge(&self, id: EdgeId) -> bool {
        self.delete_edge_at_epoch(id, self.current_epoch())
    }

    /// Deletes an edge at a specific epoch.
    pub fn delete_edge_at_epoch(&self, id: EdgeId, epoch: EpochId) -> bool {
        let mut edges = self.edges.write();
        if let Some(chain) = edges.get_mut(&id) {
            // Get the visible record to check if deleted and get src/dst
            let (src, dst) = {
                match chain.visible_at(epoch) {
                    Some(record) => {
                        if record.is_deleted() {
                            return false;
                        }
                        (record.src, record.dst)
                    }
                    None => return false, // Not visible at this epoch (already deleted)
                }
            };

            // Mark the version chain as deleted
            chain.mark_deleted(epoch);

            drop(edges); // Release lock

            // Mark as deleted in adjacency (soft delete)
            self.forward_adj.mark_deleted(src, id);
            if let Some(ref backward) = self.backward_adj {
                backward.mark_deleted(dst, id);
            }

            // Remove properties
            self.edge_properties.remove_all(id);

            true
        } else {
            false
        }
    }

    /// Returns the number of edges (non-deleted at current epoch).
    #[must_use]
    pub fn edge_count(&self) -> usize {
        let epoch = self.current_epoch();
        self.edges
            .read()
            .values()
            .filter_map(|chain| chain.visible_at(epoch))
            .filter(|r| !r.is_deleted())
            .count()
    }

    /// Discards all uncommitted versions created by a transaction.
    ///
    /// This is called during transaction rollback to clean up uncommitted changes.
    /// The method removes version chain entries created by the specified transaction.
    pub fn discard_uncommitted_versions(&self, tx_id: TxId) {
        // Remove uncommitted node versions
        {
            let mut nodes = self.nodes.write();
            for chain in nodes.values_mut() {
                chain.remove_versions_by(tx_id);
            }
            // Remove completely empty chains (no versions left)
            nodes.retain(|_, chain| !chain.is_empty());
        }

        // Remove uncommitted edge versions
        {
            let mut edges = self.edges.write();
            for chain in edges.values_mut() {
                chain.remove_versions_by(tx_id);
            }
            // Remove completely empty chains (no versions left)
            edges.retain(|_, chain| !chain.is_empty());
        }
    }

    /// Returns the number of distinct labels in the store.
    #[must_use]
    pub fn label_count(&self) -> usize {
        self.id_to_label.read().len()
    }

    /// Returns the number of distinct property keys in the store.
    ///
    /// This counts unique property keys across both nodes and edges.
    #[must_use]
    pub fn property_key_count(&self) -> usize {
        let node_keys = self.node_properties.column_count();
        let edge_keys = self.edge_properties.column_count();
        // Note: This may count some keys twice if the same key is used
        // for both nodes and edges. A more precise count would require
        // tracking unique keys across both storages.
        node_keys + edge_keys
    }

    /// Returns the number of distinct edge types in the store.
    #[must_use]
    pub fn edge_type_count(&self) -> usize {
        self.id_to_edge_type.read().len()
    }

    // === Traversal ===

    /// Returns an iterator over neighbors of a node.
    pub fn neighbors(
        &self,
        node: NodeId,
        direction: Direction,
    ) -> impl Iterator<Item = NodeId> + '_ {
        let forward: Box<dyn Iterator<Item = NodeId>> = match direction {
            Direction::Outgoing | Direction::Both => {
                Box::new(self.forward_adj.neighbors(node).into_iter())
            }
            Direction::Incoming => Box::new(std::iter::empty()),
        };

        let backward: Box<dyn Iterator<Item = NodeId>> = match direction {
            Direction::Incoming | Direction::Both => {
                if let Some(ref adj) = self.backward_adj {
                    Box::new(adj.neighbors(node).into_iter())
                } else {
                    Box::new(std::iter::empty())
                }
            }
            Direction::Outgoing => Box::new(std::iter::empty()),
        };

        forward.chain(backward)
    }

    /// Returns edges from a node with their targets.
    ///
    /// Returns an iterator of (target_node, edge_id) pairs.
    pub fn edges_from(
        &self,
        node: NodeId,
        direction: Direction,
    ) -> impl Iterator<Item = (NodeId, EdgeId)> + '_ {
        let forward: Box<dyn Iterator<Item = (NodeId, EdgeId)>> = match direction {
            Direction::Outgoing | Direction::Both => {
                Box::new(self.forward_adj.edges_from(node).into_iter())
            }
            Direction::Incoming => Box::new(std::iter::empty()),
        };

        let backward: Box<dyn Iterator<Item = (NodeId, EdgeId)>> = match direction {
            Direction::Incoming | Direction::Both => {
                if let Some(ref adj) = self.backward_adj {
                    Box::new(adj.edges_from(node).into_iter())
                } else {
                    Box::new(std::iter::empty())
                }
            }
            Direction::Outgoing => Box::new(std::iter::empty()),
        };

        forward.chain(backward)
    }

    /// Gets the type of an edge by ID.
    #[must_use]
    pub fn edge_type(&self, id: EdgeId) -> Option<Arc<str>> {
        let edges = self.edges.read();
        let chain = edges.get(&id)?;
        let epoch = self.current_epoch();
        let record = chain.visible_at(epoch)?;
        let id_to_type = self.id_to_edge_type.read();
        id_to_type.get(record.type_id as usize).cloned()
    }

    /// Returns nodes with a specific label.
    pub fn nodes_by_label(&self, label: &str) -> Vec<NodeId> {
        let label_to_id = self.label_to_id.read();
        if let Some(&label_id) = label_to_id.get(label) {
            let index = self.label_index.read();
            if let Some(set) = index.get(label_id as usize) {
                return set.keys().copied().collect();
            }
        }
        Vec::new()
    }

    // === Zone Map Support ===

    /// Checks if a node property predicate might match any nodes.
    ///
    /// Uses zone maps for early filtering. Returns `true` if there might be
    /// matching nodes, `false` if there definitely aren't.
    #[must_use]
    pub fn node_property_might_match(
        &self,
        property: &PropertyKey,
        op: CompareOp,
        value: &Value,
    ) -> bool {
        self.node_properties.might_match(property, op, value)
    }

    /// Checks if an edge property predicate might match any edges.
    #[must_use]
    pub fn edge_property_might_match(
        &self,
        property: &PropertyKey,
        op: CompareOp,
        value: &Value,
    ) -> bool {
        self.edge_properties.might_match(property, op, value)
    }

    /// Gets the zone map for a node property.
    #[must_use]
    pub fn node_property_zone_map(&self, property: &PropertyKey) -> Option<ZoneMapEntry> {
        self.node_properties.zone_map(property)
    }

    /// Gets the zone map for an edge property.
    #[must_use]
    pub fn edge_property_zone_map(&self, property: &PropertyKey) -> Option<ZoneMapEntry> {
        self.edge_properties.zone_map(property)
    }

    /// Rebuilds zone maps for all properties.
    pub fn rebuild_zone_maps(&self) {
        self.node_properties.rebuild_zone_maps();
        self.edge_properties.rebuild_zone_maps();
    }

    // === Statistics ===

    /// Returns the current statistics.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.statistics.read().clone()
    }

    /// Updates statistics from current data.
    ///
    /// This scans all labels and edge types to compute cardinality statistics.
    pub fn compute_statistics(&self) {
        let mut stats = Statistics::new();

        // Compute total counts
        stats.total_nodes = self.node_count() as u64;
        stats.total_edges = self.edge_count() as u64;

        // Compute per-label statistics
        let id_to_label = self.id_to_label.read();
        let label_index = self.label_index.read();

        for (label_id, label_name) in id_to_label.iter().enumerate() {
            let node_count = label_index
                .get(label_id)
                .map(|set| set.len() as u64)
                .unwrap_or(0);

            if node_count > 0 {
                // Estimate average degree
                let avg_out_degree = if stats.total_nodes > 0 {
                    stats.total_edges as f64 / stats.total_nodes as f64
                } else {
                    0.0
                };

                let label_stats =
                    LabelStatistics::new(node_count).with_degrees(avg_out_degree, avg_out_degree);

                stats.update_label(label_name.as_ref(), label_stats);
            }
        }

        // Compute per-edge-type statistics
        let id_to_edge_type = self.id_to_edge_type.read();
        let edges = self.edges.read();
        let epoch = self.current_epoch();

        let mut edge_type_counts: FxHashMap<u32, u64> = FxHashMap::default();
        for chain in edges.values() {
            if let Some(record) = chain.visible_at(epoch) {
                if !record.is_deleted() {
                    *edge_type_counts.entry(record.type_id).or_default() += 1;
                }
            }
        }

        for (type_id, count) in edge_type_counts {
            if let Some(type_name) = id_to_edge_type.get(type_id as usize) {
                let avg_degree = if stats.total_nodes > 0 {
                    count as f64 / stats.total_nodes as f64
                } else {
                    0.0
                };

                let edge_stats = EdgeTypeStatistics::new(count, avg_degree, avg_degree);
                stats.update_edge_type(type_name.as_ref(), edge_stats);
            }
        }

        *self.statistics.write() = stats;
    }

    /// Estimates cardinality for a label scan.
    #[must_use]
    pub fn estimate_label_cardinality(&self, label: &str) -> f64 {
        self.statistics.read().estimate_label_cardinality(label)
    }

    /// Estimates average degree for an edge type.
    #[must_use]
    pub fn estimate_avg_degree(&self, edge_type: &str, outgoing: bool) -> f64 {
        self.statistics
            .read()
            .estimate_avg_degree(edge_type, outgoing)
    }

    // === Internal Helpers ===

    fn get_or_create_label_id(&self, label: &str) -> u32 {
        {
            let label_to_id = self.label_to_id.read();
            if let Some(&id) = label_to_id.get(label) {
                return id;
            }
        }

        let mut label_to_id = self.label_to_id.write();
        let mut id_to_label = self.id_to_label.write();

        // Double-check after acquiring write lock
        if let Some(&id) = label_to_id.get(label) {
            return id;
        }

        let id = id_to_label.len() as u32;

        let label: Arc<str> = label.into();
        label_to_id.insert(label.clone(), id);
        id_to_label.push(label);

        id
    }

    fn get_or_create_edge_type_id(&self, edge_type: &str) -> u32 {
        {
            let type_to_id = self.edge_type_to_id.read();
            if let Some(&id) = type_to_id.get(edge_type) {
                return id;
            }
        }

        let mut type_to_id = self.edge_type_to_id.write();
        let mut id_to_type = self.id_to_edge_type.write();

        // Double-check
        if let Some(&id) = type_to_id.get(edge_type) {
            return id;
        }

        let id = id_to_type.len() as u32;
        let edge_type: Arc<str> = edge_type.into();
        type_to_id.insert(edge_type.clone(), id);
        id_to_type.push(edge_type);

        id
    }

    // === Recovery Support ===

    /// Creates a node with a specific ID during recovery.
    ///
    /// This is used for WAL recovery to restore nodes with their original IDs.
    /// The caller must ensure IDs don't conflict with existing nodes.
    pub fn create_node_with_id(&self, id: NodeId, labels: &[&str]) {
        let epoch = self.current_epoch();
        let mut record = NodeRecord::new(id, epoch);
        record.set_label_count(labels.len() as u16);

        // Store labels in node_labels map and label_index
        let mut node_label_set = FxHashSet::default();
        for label in labels {
            let label_id = self.get_or_create_label_id(*label);
            node_label_set.insert(label_id);

            // Update label index
            let mut index = self.label_index.write();
            while index.len() <= label_id as usize {
                index.push(FxHashMap::default());
            }
            index[label_id as usize].insert(id, ());
        }

        // Store node's labels
        self.node_labels.write().insert(id, node_label_set);

        // Create version chain with initial version (using SYSTEM tx for recovery)
        let chain = VersionChain::with_initial(record, epoch, TxId::SYSTEM);
        self.nodes.write().insert(id, chain);

        // Update next_node_id if necessary to avoid future collisions
        let id_val = id.as_u64();
        let _ = self
            .next_node_id
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if id_val >= current {
                    Some(id_val + 1)
                } else {
                    None
                }
            });
    }

    /// Creates an edge with a specific ID during recovery.
    ///
    /// This is used for WAL recovery to restore edges with their original IDs.
    pub fn create_edge_with_id(&self, id: EdgeId, src: NodeId, dst: NodeId, edge_type: &str) {
        let epoch = self.current_epoch();
        let type_id = self.get_or_create_edge_type_id(edge_type);

        let record = EdgeRecord::new(id, src, dst, type_id, epoch);
        let chain = VersionChain::with_initial(record, epoch, TxId::SYSTEM);
        self.edges.write().insert(id, chain);

        // Update adjacency
        self.forward_adj.add_edge(src, dst, id);
        if let Some(ref backward) = self.backward_adj {
            backward.add_edge(dst, src, id);
        }

        // Update next_edge_id if necessary
        let id_val = id.as_u64();
        let _ = self
            .next_edge_id
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                if id_val >= current {
                    Some(id_val + 1)
                } else {
                    None
                }
            });
    }

    /// Sets the current epoch during recovery.
    pub fn set_epoch(&self, epoch: EpochId) {
        self.current_epoch.store(epoch.as_u64(), Ordering::SeqCst);
    }
}

impl Default for LpgStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node() {
        let store = LpgStore::new();

        let id = store.create_node(&["Person"]);
        assert!(id.is_valid());

        let node = store.get_node(id).unwrap();
        assert!(node.has_label("Person"));
        assert!(!node.has_label("Animal"));
    }

    #[test]
    fn test_create_node_with_props() {
        let store = LpgStore::new();

        let id = store.create_node_with_props(
            &["Person"],
            [("name", Value::from("Alice")), ("age", Value::from(30i64))],
        );

        let node = store.get_node(id).unwrap();
        assert_eq!(
            node.get_property("name").and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            node.get_property("age").and_then(|v| v.as_int64()),
            Some(30)
        );
    }

    #[test]
    fn test_delete_node() {
        let store = LpgStore::new();

        let id = store.create_node(&["Person"]);
        assert_eq!(store.node_count(), 1);

        assert!(store.delete_node(id));
        assert_eq!(store.node_count(), 0);
        assert!(store.get_node(id).is_none());

        // Double delete should return false
        assert!(!store.delete_node(id));
    }

    #[test]
    fn test_create_edge() {
        let store = LpgStore::new();

        let alice = store.create_node(&["Person"]);
        let bob = store.create_node(&["Person"]);

        let edge_id = store.create_edge(alice, bob, "KNOWS");
        assert!(edge_id.is_valid());

        let edge = store.get_edge(edge_id).unwrap();
        assert_eq!(edge.src, alice);
        assert_eq!(edge.dst, bob);
        assert_eq!(edge.edge_type.as_ref(), "KNOWS");
    }

    #[test]
    fn test_neighbors() {
        let store = LpgStore::new();

        let a = store.create_node(&["Person"]);
        let b = store.create_node(&["Person"]);
        let c = store.create_node(&["Person"]);

        store.create_edge(a, b, "KNOWS");
        store.create_edge(a, c, "KNOWS");

        let outgoing: Vec<_> = store.neighbors(a, Direction::Outgoing).collect();
        assert_eq!(outgoing.len(), 2);
        assert!(outgoing.contains(&b));
        assert!(outgoing.contains(&c));

        let incoming: Vec<_> = store.neighbors(b, Direction::Incoming).collect();
        assert_eq!(incoming.len(), 1);
        assert!(incoming.contains(&a));
    }

    #[test]
    fn test_nodes_by_label() {
        let store = LpgStore::new();

        let p1 = store.create_node(&["Person"]);
        let p2 = store.create_node(&["Person"]);
        let _a = store.create_node(&["Animal"]);

        let persons = store.nodes_by_label("Person");
        assert_eq!(persons.len(), 2);
        assert!(persons.contains(&p1));
        assert!(persons.contains(&p2));

        let animals = store.nodes_by_label("Animal");
        assert_eq!(animals.len(), 1);
    }

    #[test]
    fn test_delete_edge() {
        let store = LpgStore::new();

        let a = store.create_node(&["Person"]);
        let b = store.create_node(&["Person"]);
        let edge_id = store.create_edge(a, b, "KNOWS");

        assert_eq!(store.edge_count(), 1);

        assert!(store.delete_edge(edge_id));
        assert_eq!(store.edge_count(), 0);
        assert!(store.get_edge(edge_id).is_none());
    }
}
