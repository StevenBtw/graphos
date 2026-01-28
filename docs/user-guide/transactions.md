# Transactions in Graphos

Graphos provides ACID transactions with **Snapshot Isolation** semantics. This guide explains how transactions work, their guarantees, and important limitations to be aware of.

## Quick Start

```python
from graphos import GraphosDB

db = GraphosDB()

# Explicit transaction
with db.begin_transaction() as tx:
    tx.execute("CREATE (n:Person {name: 'Alice'})")
    tx.execute("CREATE (n:Person {name: 'Bob'})")
    tx.commit()  # All changes visible atomically

# Auto-commit mode (default)
db.execute("CREATE (n:Person {name: 'Charlie'})")  # Commits immediately
```

## Isolation Level: Snapshot Isolation

Graphos implements **Snapshot Isolation (SI)**, a widely-used isolation level that provides strong consistency while maintaining high concurrency.

### What You Get

| Guarantee | Description |
|-----------|-------------|
| **Repeatable Reads** | Reading the same data twice in a transaction returns the same result |
| **No Dirty Reads** | You never see uncommitted changes from other transactions |
| **No Lost Updates** | Write-write conflicts are detected and one transaction is aborted |
| **Consistent Snapshot** | All reads see the database as of transaction start time |

### How It Works

1. When a transaction starts, it receives a **start epoch** representing the current database state
2. All reads within the transaction see data as of that epoch
3. Writes are buffered and only become visible after commit
4. At commit time, the system checks for **write-write conflicts**
5. If another committed transaction wrote to the same entity, the commit fails

## Write-Write Conflict Detection

Graphos automatically detects when two transactions try to modify the same entity:

```python
# Thread 1
tx1 = db.begin_transaction()
tx1.execute("MATCH (n:Counter {id: 1}) SET n.value = n.value + 10")

# Thread 2 (concurrent)
tx2 = db.begin_transaction()
tx2.execute("MATCH (n:Counter {id: 1}) SET n.value = n.value + 20")

tx1.commit()  # Succeeds
tx2.commit()  # Fails with WriteConflict error
```

When a conflict is detected, your application should:
1. Catch the exception
2. Optionally retry the transaction
3. Or report the conflict to the user

## Important Limitation: Write Skew

Snapshot Isolation does **not** prevent all anomalies. The **write skew** anomaly can occur when transactions read overlapping data but write to different entities.

### Example: The Classic Write Skew

Consider a constraint where `A + B >= 0`:

```python
# Initial: A = 50, B = 50

# Transaction 1
tx1 = db.begin_transaction()
a = tx1.execute("MATCH (n:Account {name: 'A'}) RETURN n.balance").scalar()  # 50
b = tx1.execute("MATCH (n:Account {name: 'B'}) RETURN n.balance").scalar()  # 50
# Check: 50 + 50 - 100 = 0 >= 0, OK
tx1.execute("MATCH (n:Account {name: 'A'}) SET n.balance = -50")

# Transaction 2 (concurrent, sees same snapshot)
tx2 = db.begin_transaction()
a = tx2.execute("MATCH (n:Account {name: 'A'}) RETURN n.balance").scalar()  # 50
b = tx2.execute("MATCH (n:Account {name: 'B'}) RETURN n.balance").scalar()  # 50
# Check: 50 + 50 - 100 = 0 >= 0, OK
tx2.execute("MATCH (n:Account {name: 'B'}) SET n.balance = -50")

tx1.commit()  # Success (wrote to A)
tx2.commit()  # Success (wrote to B, no conflict with A)

# Result: A = -50, B = -50, constraint violated!
```

### Workarounds for Write Skew

**Option 1: Promote Reads to Writes**

Add a dummy write to entities you read to force conflict detection:

```python
tx = db.begin_transaction()
# Read both accounts
a = tx.execute("MATCH (n:Account {name: 'A'}) RETURN n").scalar()
b = tx.execute("MATCH (n:Account {name: 'B'}) RETURN n").scalar()

# "Touch" both accounts to register them in write set
tx.execute("MATCH (n:Account {name: 'A'}) SET n._touched = timestamp()")
tx.execute("MATCH (n:Account {name: 'B'}) SET n._touched = timestamp()")

# Now make actual change
tx.execute("MATCH (n:Account {name: 'A'}) SET n.balance = -50")
tx.commit()  # Will conflict if another tx touched A or B
```

**Option 2: Application-Level Validation**

Re-check constraints before commit:

```python
def withdraw(db, account, amount):
    while True:
        tx = db.begin_transaction()
        try:
            # Read current state
            a = tx.execute("MATCH (n:Account {name: 'A'}) RETURN n.balance").scalar()
            b = tx.execute("MATCH (n:Account {name: 'B'}) RETURN n.balance").scalar()

            # Make change
            if account == 'A':
                new_a = a - amount
                if new_a + b < 0:
                    raise ValueError("Would violate constraint")
                tx.execute(f"MATCH (n:Account {{name: 'A'}}) SET n.balance = {new_a}")

            tx.commit()
            return  # Success
        except WriteConflictError:
            continue  # Retry
```

**Option 3: External Locking**

Use database-external locks for critical operations:

```python
import threading

account_lock = threading.Lock()

def withdraw(db, account, amount):
    with account_lock:  # Serializes all withdrawals
        tx = db.begin_transaction()
        # ... perform withdrawal ...
        tx.commit()
```

## Transaction Lifecycle

### States

| State | Description |
|-------|-------------|
| `Active` | Transaction is in progress, can read and write |
| `Committed` | Transaction completed successfully, changes visible |
| `Aborted` | Transaction was rolled back, changes discarded |

### Best Practices

1. **Keep transactions short**: Long transactions increase conflict probability
2. **Batch related changes**: Group related writes in a single transaction
3. **Handle conflicts gracefully**: Implement retry logic for write conflicts
4. **Use auto-commit for single operations**: Simpler and equally safe
5. **Don't hold transactions open during user interaction**: Risk of blocking GC

## API Reference

### Python

```python
# Start explicit transaction
tx = db.begin_transaction()

# Execute within transaction
result = tx.execute("MATCH (n) RETURN n")

# Commit changes
tx.commit()

# Or rollback
tx.rollback()

# Context manager (auto-rollback on exception)
with db.begin_transaction() as tx:
    tx.execute("CREATE (n:Test)")
    tx.commit()
```

### Rust

```rust
// Start transaction
let tx_id = session.begin_transaction()?;

// Execute queries
let result = session.execute("MATCH (n) RETURN n")?;

// Commit
session.commit()?;

// Or rollback
session.rollback()?;
```

## Garbage Collection

Graphos automatically garbage collects old transaction metadata and version chains:

- Aborted transactions are cleaned up immediately
- Committed transaction metadata is retained until no active transaction can see it
- Version chains are pruned based on the oldest active transaction's start epoch

This happens automatically; no manual intervention is needed.

## Future: Serializable Isolation

Full Serializable isolation (preventing write skew) is planned for a future release. This will include:

- Read-write conflict detection
- Serializable Snapshot Isolation (SSI) implementation
- Configurable isolation levels per transaction

For now, use the workarounds described above if your application requires serializable semantics.
