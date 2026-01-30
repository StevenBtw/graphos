---
title: Database Operations
description: Creating and managing databases in Python.
tags:
  - python
  - database
---

# Database Operations

Learn how to create and configure Grafeo databases in Python.

## Creating a Database

```python
import grafeo

# In-memory database
db = grafeo.GrafeoDB()

# Persistent database
db = grafeo.GrafeoDB(path="my_graph.db")
```

## Configuration Options

```python
db = grafeo.GrafeoDB(
    path="my_graph.db",
    memory_limit=4 * 1024 * 1024 * 1024,  # 4 GB
    threads=8,
    read_only=False
)
```

## Database Lifecycle

```python
# Create database
db = grafeo.GrafeoDB(path="my_graph.db")

# Use the database
with db.session() as session:
    session.execute("INSERT (:Person {name: 'Alice'})")

# Close explicitly (optional - closes on garbage collection)
db.close()
```

## Context Manager

```python
# Database as context manager
with grafeo.GrafeoDB(path="my_graph.db") as db:
    with db.session() as session:
        session.execute("INSERT (:Person {name: 'Alice'})")
# Database is automatically closed
```

## Multiple Sessions

```python
db = grafeo.GrafeoDB()

# Create multiple sessions for concurrent access
session1 = db.session()
session2 = db.session()

# Each session has its own transaction context
session1.execute("INSERT (:Person {name: 'Alice'})")
session2.execute("INSERT (:Person {name: 'Bob'})")

session1.close()
session2.close()
```
