---
title: Gremlin Query Language
description: Learn the Gremlin traversal language for Graphos.
---

# Gremlin Query Language

Gremlin is a graph traversal language developed by Apache TinkerPop. Graphos supports Gremlin as an optional query language via feature flag.

## Overview

Gremlin uses a functional, step-based approach to traverse and manipulate graph data. It's designed around the concept of traversing vertices and edges through a series of chained steps.

## Enabling Gremlin

Gremlin support is optional and requires a feature flag:

=== "Rust"

    ```bash
    cargo add graphos-engine --features gremlin
    ```

=== "Python"

    ```bash
    uv add pygraphos[gremlin]
    ```

## Quick Reference

| Operation | Syntax |
|-----------|--------|
| All vertices | `g.V()` |
| Vertex by ID | `g.V(id)` |
| Filter by label | `g.V().hasLabel('Person')` |
| Filter by property | `g.V().has('name', 'Alice')` |
| Outgoing edges | `g.V().out('KNOWS')` |
| Incoming edges | `g.V().in('KNOWS')` |
| Both directions | `g.V().both('KNOWS')` |
| Get properties | `g.V().values('name')` |
| Count results | `g.V().count()` |
| Limit results | `g.V().limit(10)` |

## Basic Examples

### Finding Vertices

```gremlin
// All vertices
g.V()

// Vertices with a specific label
g.V().hasLabel('Person')

// Vertex with specific property
g.V().has('name', 'Alice')

// Multiple conditions
g.V().hasLabel('Person').has('age', gt(25))
```

### Traversing Edges

```gremlin
// Friends of Alice
g.V().has('name', 'Alice').out('KNOWS')

// People who know Bob
g.V().has('name', 'Bob').in('KNOWS')

// Two-hop traversal
g.V().has('name', 'Alice').out('KNOWS').out('KNOWS')
```

### Getting Properties

```gremlin
// Get names of all people
g.V().hasLabel('Person').values('name')

// Get multiple properties
g.V().hasLabel('Person').valueMap('name', 'age')
```

### Aggregations

```gremlin
// Count all people
g.V().hasLabel('Person').count()

// Count friends
g.V().has('name', 'Alice').out('KNOWS').count()
```

## Python Usage

```python
import graphos

db = graphos.GraphosDB()

# Create some data
db.execute("INSERT (:Person {name: 'Alice', age: 30})")
db.execute("INSERT (:Person {name: 'Bob', age: 25})")
db.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    INSERT (a)-[:KNOWS {since: 2020}]->(b)
""")

# Query with Gremlin
result = db.execute_gremlin("g.V().hasLabel('Person').values('name')")
for row in result:
    print(row)

# Traverse relationships
friends = db.execute_gremlin("g.V().has('name', 'Alice').out('KNOWS').values('name')")
```

## Rust Usage

```rust
use graphos_engine::GraphosDB;

let db = GraphosDB::new_in_memory();

// Create data with GQL
db.execute("INSERT (:Person {name: 'Alice'})").unwrap();

// Query with Gremlin
let result = db.execute_gremlin("g.V().hasLabel('Person')").unwrap();
```

## Supported Steps

### Source Steps
- `g.V()` - Start traversal from vertices
- `g.V(id)` - Start from specific vertex

### Filter Steps
- `hasLabel(label)` - Filter by vertex label
- `has(key, value)` - Filter by property equality
- `has(key, predicate)` - Filter by property predicate

### Traversal Steps
- `out(label?)` - Traverse outgoing edges
- `in(label?)` - Traverse incoming edges
- `both(label?)` - Traverse both directions
- `outE(label?)` - Get outgoing edges
- `inE(label?)` - Get incoming edges

### Property Steps
- `values(key)` - Get property values
- `valueMap(keys...)` - Get multiple properties as map

### Terminal Steps
- `count()` - Count elements
- `limit(n)` - Limit results

## Learn More

<div class="grid cards" markdown>

-   **[Basic Traversals](basic-traversals.md)**

    ---

    g.V(), hasLabel, has, and filtering.

-   **[Edge Traversals](edge-traversals.md)**

    ---

    out, in, both, and relationship patterns.

-   **[Properties](properties.md)**

    ---

    values, valueMap, and property access.

-   **[Aggregations](aggregations.md)**

    ---

    count, sum, and grouping.

</div>
