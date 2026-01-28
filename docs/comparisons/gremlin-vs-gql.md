---
title: Gremlin vs GQL
description: Compare Gremlin traversal language with GQL query language.
---

# Gremlin vs GQL

This guide compares Gremlin (Apache TinkerPop) with GQL (ISO/IEC 39075) to help you choose the right query language for your use case.

## Philosophy

| Aspect | Gremlin | GQL |
|--------|---------|-----|
| **Style** | Imperative traversal | Declarative pattern matching |
| **Origin** | Apache TinkerPop | ISO standard (39075) |
| **Focus** | Step-by-step navigation | What to find, not how |

## Syntax Comparison

### Finding Nodes

=== "Gremlin"

    ```gremlin
    g.V().hasLabel('Person').has('name', 'Alice')
    ```

=== "GQL"

    ```sql
    MATCH (p:Person {name: 'Alice'})
    RETURN p
    ```

### Traversing Relationships

=== "Gremlin"

    ```gremlin
    g.V().has('name', 'Alice').out('KNOWS').values('name')
    ```

=== "GQL"

    ```sql
    MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(friend)
    RETURN friend.name
    ```

### Multiple Hops

=== "Gremlin"

    ```gremlin
    g.V().has('name', 'Alice').out('KNOWS').out('KNOWS').values('name')
    ```

=== "GQL"

    ```sql
    MATCH (a:Person {name: 'Alice'})-[:KNOWS]->()-[:KNOWS]->(fof)
    RETURN fof.name
    ```

### Counting

=== "Gremlin"

    ```gremlin
    g.V().hasLabel('Person').count()
    ```

=== "GQL"

    ```sql
    MATCH (p:Person)
    RETURN COUNT(p)
    ```

### Filtering

=== "Gremlin"

    ```gremlin
    g.V().hasLabel('Person').has('age', gt(25))
    ```

=== "GQL"

    ```sql
    MATCH (p:Person)
    WHERE p.age > 25
    RETURN p
    ```

## When to Use Each

### Choose Gremlin When

- You prefer imperative, step-by-step traversal logic
- You're familiar with functional programming patterns
- You need fine-grained control over traversal order
- Porting from another TinkerPop-compatible database

### Choose GQL When

- You prefer declarative pattern matching
- You're familiar with SQL-like syntax
- You want ISO-standard compatibility
- Complex pattern matching is a priority
- You need clear, readable queries

## Feature Comparison

| Feature | Gremlin | GQL |
|---------|---------|-----|
| Pattern matching | Via chained steps | Native MATCH clause |
| Aggregations | `count()`, `sum()`, etc. | `COUNT()`, `SUM()`, etc. |
| Path queries | `repeat().until()` | Variable-length patterns |
| Subqueries | Lambda steps | EXISTS, subqueries |
| Mutations | `addV()`, `addE()` | INSERT, SET, DELETE |
| Readability | Method chaining | SQL-like syntax |

## Mixing Languages

Graphos allows you to use both languages in the same database:

```python
import graphos

db = graphos.GraphosDB()

# Create data with GQL
db.execute("INSERT (:Person {name: 'Alice'})")
db.execute("INSERT (:Person {name: 'Bob'})")
db.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    INSERT (a)-[:KNOWS]->(b)
""")

# Query with Gremlin
result = db.execute_gremlin("g.V().hasLabel('Person').values('name')")

# Or query with GQL
result = db.execute("MATCH (p:Person) RETURN p.name")
```

## Recommendation

For most users, we recommend **GQL** as the primary query language due to its:

- ISO standardization
- Readable, declarative syntax
- Powerful pattern matching
- Familiar SQL-like structure

Use **Gremlin** when you need imperative traversal control or are migrating from a TinkerPop-based system.
