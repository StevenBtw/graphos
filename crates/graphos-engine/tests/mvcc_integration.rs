//! MVCC Integration Tests
//!
//! Tests for Multi-Version Concurrency Control (MVCC) functionality.
//! These tests verify snapshot isolation, write-write conflict detection,
//! rollback behavior, and version chain garbage collection.

use std::sync::Arc;

use graphos_common::types::{EpochId, Value};
use graphos_core::graph::lpg::LpgStore;
use graphos_engine::{
    transaction::{TransactionManager, TxState},
    GraphosDB,
};

/// Helper to create a test store with some initial data.
fn create_test_store() -> Arc<LpgStore> {
    let store = Arc::new(LpgStore::new());

    // Create some initial nodes
    let alice_id = store.create_node_with_props(
        &["Person"],
        [("name", Value::String("Alice".into())), ("age", Value::Int64(30))],
    );
    let bob_id = store.create_node_with_props(
        &["Person"],
        [("name", Value::String("Bob".into())), ("age", Value::Int64(25))],
    );

    // Create an edge
    store.create_edge(alice_id, bob_id, "KNOWS");

    store
}

// ============================================================================
// Snapshot Isolation Tests
// ============================================================================

#[test]
fn test_snapshot_isolation_reads_see_consistent_data() {
    // A transaction should see a consistent snapshot of data at its start time
    let store = create_test_store();
    let tx_manager = Arc::new(TransactionManager::new());

    // Start T1
    let tx1 = tx_manager.begin();
    let epoch1 = tx_manager.start_epoch(tx1).unwrap();

    // Create a new node (simulating T2 committing)
    let new_epoch = EpochId::new(epoch1.as_u64() + 10);
    let new_tx = tx_manager.begin();
    store.create_node_versioned(&["Person"], new_epoch, new_tx);
    tx_manager.commit(new_tx).unwrap();

    // T1 should not see the new node (created after T1's snapshot)
    let all_nodes = store.node_ids();
    let visible_at_epoch1: Vec<_> = all_nodes
        .iter()
        .filter(|id| store.get_node_versioned(**id, epoch1, tx1).is_some())
        .collect();

    // Should see initial 2 nodes, not the new one
    assert_eq!(visible_at_epoch1.len(), 2, "T1 should only see 2 initial nodes");
}

#[test]
fn test_transaction_sees_own_writes() {
    // A transaction should always see its own uncommitted writes
    let store = Arc::new(LpgStore::new());
    let tx_manager = Arc::new(TransactionManager::new());

    let tx1 = tx_manager.begin();
    let epoch = tx_manager.current_epoch();

    // Create a node within the transaction
    let node_id = store.create_node_versioned(&["TestNode"], epoch, tx1);

    // Transaction should see its own node
    let visible = store.get_node_versioned(node_id, epoch, tx1);
    assert!(visible.is_some(), "Transaction should see its own uncommitted writes");
}

#[test]
fn test_committed_writes_visible_to_new_transactions() {
    // After commit, writes should be visible to new transactions
    let store = Arc::new(LpgStore::new());
    let tx_manager = Arc::new(TransactionManager::new());

    // T1 creates and commits
    let tx1 = tx_manager.begin();
    let epoch1 = tx_manager.current_epoch();
    let node_id = store.create_node_versioned(&["Committed"], epoch1, tx1);
    tx_manager.commit(tx1).unwrap();

    // T2 starts after commit
    let tx2 = tx_manager.begin();
    let epoch2 = tx_manager.current_epoch();

    // T2 should see T1's committed node
    let visible = store.get_node_versioned(node_id, epoch2, tx2);
    assert!(visible.is_some(), "New transaction should see committed writes");
}

// ============================================================================
// Write-Write Conflict Detection Tests
// ============================================================================

#[test]
fn test_write_write_conflict_detection() {
    // Two transactions writing to the same entity should result in a conflict
    let store = create_test_store();
    let tx_manager = Arc::new(TransactionManager::new());

    let node_id = store.node_ids()[0]; // Get first node

    // T1 records a write
    let tx1 = tx_manager.begin();
    tx_manager.record_write(tx1, node_id).unwrap();

    // T2 also records a write to same entity
    let tx2 = tx_manager.begin();
    tx_manager.record_write(tx2, node_id).unwrap();

    // T1 commits first
    tx_manager.commit(tx1).unwrap();

    // T2 should fail with conflict
    let result = tx_manager.commit(tx2);
    assert!(result.is_err(), "Second transaction should fail with write-write conflict");

    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("conflict"),
        "Error should indicate write conflict: {}",
        err
    );
}

#[test]
fn test_non_overlapping_writes_succeed() {
    // Two transactions writing to different entities should both succeed
    let store = create_test_store();
    let tx_manager = Arc::new(TransactionManager::new());

    let nodes = store.node_ids();
    assert!(nodes.len() >= 2, "Need at least 2 nodes for this test");

    // T1 writes to node 0
    let tx1 = tx_manager.begin();
    tx_manager.record_write(tx1, nodes[0]).unwrap();

    // T2 writes to node 1
    let tx2 = tx_manager.begin();
    tx_manager.record_write(tx2, nodes[1]).unwrap();

    // Both should commit successfully
    assert!(tx_manager.commit(tx1).is_ok(), "T1 should commit");
    assert!(tx_manager.commit(tx2).is_ok(), "T2 should commit (no conflict)");
}

// ============================================================================
// Rollback Tests
// ============================================================================

#[test]
fn test_rollback_makes_writes_invisible() {
    // After rollback, a transaction's writes should not be visible
    let store = Arc::new(LpgStore::new());
    let tx_manager = Arc::new(TransactionManager::new());

    // T1 creates a node
    let tx1 = tx_manager.begin();
    let epoch = tx_manager.current_epoch();
    let node_id = store.create_node_versioned(&["Rollback"], epoch, tx1);
    tx_manager.record_write(tx1, node_id).unwrap();

    // Abort T1
    tx_manager.abort(tx1).unwrap();

    // Discard uncommitted versions for this transaction
    store.discard_uncommitted_versions(tx1);

    // New transaction should not see the aborted node
    let tx2 = tx_manager.begin();
    let epoch2 = tx_manager.current_epoch();
    let visible = store.get_node_versioned(node_id, epoch2, tx2);

    assert!(visible.is_none(), "Aborted transaction's writes should not be visible");
}

#[test]
fn test_abort_releases_write_locks() {
    // After abort, another transaction should be able to write to the same entity
    let store = create_test_store();
    let tx_manager = Arc::new(TransactionManager::new());

    let node_id = store.node_ids()[0];

    // T1 records a write
    let tx1 = tx_manager.begin();
    tx_manager.record_write(tx1, node_id).unwrap();

    // Abort T1
    tx_manager.abort(tx1).unwrap();

    // GC the aborted transaction
    tx_manager.gc();

    // T2 should be able to write to the same entity
    let tx2 = tx_manager.begin();
    tx_manager.record_write(tx2, node_id).unwrap();

    // T2 should commit successfully
    assert!(tx_manager.commit(tx2).is_ok(), "T2 should commit after T1 aborted");
}

// ============================================================================
// Version Chain GC Tests
// ============================================================================

#[test]
fn test_gc_cleans_up_completed_transactions() {
    // GC should clean up completed (committed/aborted) transactions
    let tx_manager = Arc::new(TransactionManager::new());

    // Create and complete several transactions
    for _ in 0..5 {
        let tx = tx_manager.begin();
        tx_manager.commit(tx).unwrap();
    }

    for _ in 0..3 {
        let tx = tx_manager.begin();
        tx_manager.abort(tx).unwrap();
    }

    // All transactions are completed
    assert_eq!(tx_manager.active_count(), 0, "No active transactions");

    // GC should clean them up
    let cleaned = tx_manager.gc();
    assert_eq!(cleaned, 8, "GC should clean up all 8 completed transactions");
}

// ============================================================================
// Session Integration Tests (using GraphosDB)
// ============================================================================

#[test]
fn test_session_transaction_isolation() {
    // Verify session-level transaction isolation
    let db = GraphosDB::new_in_memory();
    let mut session = db.session();

    // Begin transaction
    session.begin_tx().unwrap();

    // Create a node within transaction (using GQL INSERT syntax)
    session.execute("INSERT (:TestIsolation {value: 42})").unwrap();

    // Commit
    session.commit().unwrap();

    // Query should find the node
    let result = session.execute("MATCH (n:TestIsolation) RETURN n.value").unwrap();
    assert!(result.row_count() >= 1, "Node should be visible after commit");
}

#[test]
fn test_session_rollback_state() {
    // Verify session rollback properly releases transaction state
    // NOTE: Full SQL rollback (discarding INSERT/UPDATE/DELETE) requires
    // the query executor to use versioned storage, which is a separate enhancement.
    let db = GraphosDB::new_in_memory();
    let mut session = db.session();

    // Begin transaction
    session.begin_tx().unwrap();
    assert!(session.in_transaction(), "Should be in transaction after begin");

    // Rollback
    session.rollback().unwrap();
    assert!(!session.in_transaction(), "Should not be in transaction after rollback");

    // Should be able to begin a new transaction
    session.begin_tx().unwrap();
    assert!(session.in_transaction(), "Should be able to begin new transaction");
    session.commit().unwrap();
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_transaction_commits_successfully() {
    // A transaction that does nothing should commit without issues
    let tx_manager = Arc::new(TransactionManager::new());

    let tx = tx_manager.begin();
    let result = tx_manager.commit(tx);

    assert!(result.is_ok(), "Empty transaction should commit successfully");
}

#[test]
fn test_transaction_state_transitions() {
    // Verify proper state transitions
    let tx_manager = Arc::new(TransactionManager::new());

    let tx = tx_manager.begin();
    assert_eq!(tx_manager.state(tx), Some(TxState::Active));

    tx_manager.commit(tx).unwrap();
    assert_eq!(tx_manager.state(tx), Some(TxState::Committed));

    let tx2 = tx_manager.begin();
    tx_manager.abort(tx2).unwrap();
    assert_eq!(tx_manager.state(tx2), Some(TxState::Aborted));
}

#[test]
fn test_double_commit_fails() {
    // Committing an already committed transaction should fail
    let tx_manager = Arc::new(TransactionManager::new());

    let tx = tx_manager.begin();
    tx_manager.commit(tx).unwrap();

    let result = tx_manager.commit(tx);
    assert!(result.is_err(), "Double commit should fail");
}

#[test]
fn test_commit_after_abort_fails() {
    // Committing an aborted transaction should fail
    let tx_manager = Arc::new(TransactionManager::new());

    let tx = tx_manager.begin();
    tx_manager.abort(tx).unwrap();

    let result = tx_manager.commit(tx);
    assert!(result.is_err(), "Commit after abort should fail");
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_many_concurrent_transactions() {
    // Verify system handles many concurrent transactions
    let store = Arc::new(LpgStore::new());
    let tx_manager = Arc::new(TransactionManager::new());

    let mut transactions = Vec::new();

    // Start 100 transactions
    for _ in 0..100 {
        let tx = tx_manager.begin();
        let epoch = tx_manager.current_epoch();

        // Create a node in each transaction
        let node_id = store.create_node_versioned(&["Stress"], epoch, tx);
        tx_manager.record_write(tx, node_id).unwrap();

        transactions.push((tx, node_id));
    }

    assert_eq!(tx_manager.active_count(), 100, "Should have 100 active transactions");

    // Commit half, abort half
    for (i, (tx, _)) in transactions.iter().enumerate() {
        if i % 2 == 0 {
            tx_manager.commit(*tx).unwrap();
        } else {
            tx_manager.abort(*tx).unwrap();
        }
    }

    assert_eq!(tx_manager.active_count(), 0, "No active transactions after completion");

    // GC
    let cleaned = tx_manager.gc();
    assert_eq!(cleaned, 100, "GC should clean up all 100 transactions");
}

// ============================================================================
// Multi-Session Tests
// ============================================================================

#[test]
fn test_multiple_sessions_independent() {
    // Multiple sessions should operate independently
    let db = GraphosDB::new_in_memory();

    let session1 = db.session();
    let session2 = db.session();

    // Session 1 creates data
    session1.execute("INSERT (:Session1Node)").unwrap();

    // Session 2 creates different data
    session2.execute("INSERT (:Session2Node)").unwrap();

    // Both should see their own data
    let r1 = session1.execute("MATCH (n:Session1Node) RETURN n").unwrap();
    let r2 = session2.execute("MATCH (n:Session2Node) RETURN n").unwrap();

    assert!(r1.row_count() >= 1, "Session 1 should see its data");
    assert!(r2.row_count() >= 1, "Session 2 should see its data");
}

#[test]
fn test_session_auto_commit_mode() {
    // Verify auto-commit mode works correctly
    let db = GraphosDB::new_in_memory();
    let session = db.session();

    // Auto-commit should be on by default
    assert!(session.auto_commit());

    // Execute without explicit transaction
    session.execute("INSERT (:AutoCommit)").unwrap();

    // Data should be visible immediately in new session
    let session2 = db.session();
    let result = session2.execute("MATCH (n:AutoCommit) RETURN n").unwrap();
    assert!(result.row_count() >= 1, "Auto-committed data should be visible");
}
