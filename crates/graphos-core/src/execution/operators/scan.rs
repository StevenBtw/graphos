//! Scan operator for reading data from storage.

use super::{Operator, OperatorResult};
use crate::execution::DataChunk;
use crate::graph::lpg::LpgStore;
use graphos_common::types::{EpochId, LogicalType, NodeId, TxId};
use std::sync::Arc;

/// A scan operator that reads nodes from storage.
pub struct ScanOperator {
    /// The store to scan from.
    store: Arc<LpgStore>,
    /// Label filter (None = all nodes).
    label: Option<String>,
    /// Current position in the scan.
    position: usize,
    /// Batch of node IDs to scan.
    batch: Vec<NodeId>,
    /// Whether the scan is exhausted.
    exhausted: bool,
    /// Chunk capacity.
    chunk_capacity: usize,
    /// Transaction ID for MVCC visibility (None = use current epoch).
    tx_id: Option<TxId>,
    /// Epoch for version visibility.
    viewing_epoch: Option<EpochId>,
}

impl ScanOperator {
    /// Creates a new scan operator for all nodes.
    pub fn new(store: Arc<LpgStore>) -> Self {
        Self {
            store,
            label: None,
            position: 0,
            batch: Vec::new(),
            exhausted: false,
            chunk_capacity: 2048,
            tx_id: None,
            viewing_epoch: None,
        }
    }

    /// Creates a new scan operator for nodes with a specific label.
    pub fn with_label(store: Arc<LpgStore>, label: impl Into<String>) -> Self {
        Self {
            store,
            label: Some(label.into()),
            position: 0,
            batch: Vec::new(),
            exhausted: false,
            chunk_capacity: 2048,
            tx_id: None,
            viewing_epoch: None,
        }
    }

    /// Sets the chunk capacity.
    pub fn with_chunk_capacity(mut self, capacity: usize) -> Self {
        self.chunk_capacity = capacity;
        self
    }

    /// Sets the transaction context for MVCC visibility.
    ///
    /// When set, the scan will only return nodes visible to this transaction.
    pub fn with_tx_context(mut self, epoch: EpochId, tx_id: Option<TxId>) -> Self {
        self.viewing_epoch = Some(epoch);
        self.tx_id = tx_id;
        self
    }

    fn load_batch(&mut self) {
        if !self.batch.is_empty() || self.exhausted {
            return;
        }

        // Get nodes, using versioned method if tx context is set
        let all_ids = match &self.label {
            Some(label) => self.store.nodes_by_label(label),
            None => self.store.node_ids(),
        };

        // Filter by visibility if we have tx context
        self.batch = if let Some(epoch) = self.viewing_epoch {
            let tx = self.tx_id.unwrap_or(TxId::SYSTEM);
            all_ids
                .into_iter()
                .filter(|id| self.store.get_node_versioned(*id, epoch, tx).is_some())
                .collect()
        } else {
            all_ids
        };

        if self.batch.is_empty() {
            self.exhausted = true;
        }
    }
}

impl Operator for ScanOperator {
    fn next(&mut self) -> OperatorResult {
        self.load_batch();

        if self.exhausted || self.position >= self.batch.len() {
            return Ok(None);
        }

        // Create output chunk with node IDs
        let schema = [LogicalType::Node];
        let mut chunk = DataChunk::with_capacity(&schema, self.chunk_capacity);

        let end = (self.position + self.chunk_capacity).min(self.batch.len());
        let count = end - self.position;

        {
            // Column 0 guaranteed to exist: chunk created with single-column schema above
            let col = chunk
                .column_mut(0)
                .expect("column 0 exists: chunk created with single-column schema");
            for i in self.position..end {
                col.push_node_id(self.batch[i]);
            }
        }

        chunk.set_count(count);
        self.position = end;

        Ok(Some(chunk))
    }

    fn reset(&mut self) {
        self.position = 0;
        self.batch.clear();
        self.exhausted = false;
    }

    fn name(&self) -> &'static str {
        "Scan"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_by_label() {
        let store = Arc::new(LpgStore::new());

        store.create_node(&["Person"]);
        store.create_node(&["Person"]);
        store.create_node(&["Animal"]);

        let mut scan = ScanOperator::with_label(Arc::clone(&store), "Person");

        let chunk = scan.next().unwrap().unwrap();
        assert_eq!(chunk.row_count(), 2);

        // Should be exhausted
        let next = scan.next().unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn test_scan_reset() {
        let store = Arc::new(LpgStore::new());
        store.create_node(&["Person"]);

        let mut scan = ScanOperator::with_label(Arc::clone(&store), "Person");

        // First scan
        let chunk1 = scan.next().unwrap().unwrap();
        assert_eq!(chunk1.row_count(), 1);

        // Reset
        scan.reset();

        // Second scan should work
        let chunk2 = scan.next().unwrap().unwrap();
        assert_eq!(chunk2.row_count(), 1);
    }

    #[test]
    fn test_full_scan() {
        let store = Arc::new(LpgStore::new());

        // Create nodes with different labels
        store.create_node(&["Person"]);
        store.create_node(&["Person"]);
        store.create_node(&["Animal"]);
        store.create_node(&["Place"]);

        // Full scan (no label filter) should return all nodes
        let mut scan = ScanOperator::new(Arc::clone(&store));

        let chunk = scan.next().unwrap().unwrap();
        assert_eq!(chunk.row_count(), 4, "Full scan should return all 4 nodes");

        // Should be exhausted
        let next = scan.next().unwrap();
        assert!(next.is_none());
    }

    #[test]
    fn test_scan_with_mvcc_context() {
        let store = Arc::new(LpgStore::new());

        // Create nodes at epoch 1
        let epoch1 = EpochId::new(1);
        let tx1 = TxId::new(1);
        store.create_node_versioned(&["Person"], epoch1, tx1);
        store.create_node_versioned(&["Person"], epoch1, tx1);

        // Create a node at epoch 5
        let epoch5 = EpochId::new(5);
        let tx2 = TxId::new(2);
        store.create_node_versioned(&["Person"], epoch5, tx2);

        // Scan at epoch 3 should see only the first 2 nodes (created at epoch 1)
        let mut scan = ScanOperator::with_label(Arc::clone(&store), "Person")
            .with_tx_context(EpochId::new(3), None);

        let chunk = scan.next().unwrap().unwrap();
        assert_eq!(chunk.row_count(), 2, "Should see 2 nodes at epoch 3");

        // Scan at epoch 5 should see all 3 nodes
        let mut scan_all = ScanOperator::with_label(Arc::clone(&store), "Person")
            .with_tx_context(EpochId::new(5), None);

        let chunk_all = scan_all.next().unwrap().unwrap();
        assert_eq!(chunk_all.row_count(), 3, "Should see 3 nodes at epoch 5");
    }
}
