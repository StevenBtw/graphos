---
title: GraphQL Query Language
description: Query your graph data using GraphQL syntax.
---

# GraphQL Query Language

GraphQL is a query language for APIs developed by Facebook. Graphos supports GraphQL as an optional query language, allowing you to query graph data using familiar GraphQL syntax.

## Overview

GraphQL provides a schema-driven approach to querying data. In Graphos, node labels map to GraphQL types, and relationships map to nested fields.

## Enabling GraphQL

GraphQL support is optional and requires a feature flag:

=== "Rust"

    ```bash
    cargo add graphos-engine --features graphql
    ```

=== "Python"

    ```bash
    uv add pygraphos[graphql]
    ```

## Quick Reference

| Operation | Syntax |
|-----------|--------|
| Query type | `{ Person { ... } }` |
| Get fields | `{ Person { name age } }` |
| Filter | `{ Person(name: "Alice") { ... } }` |
| Where clause | `{ Person(where: { age_gt: 30 }) { ... } }` |
| Pagination | `{ Person(first: 10, skip: 5) { ... } }` |
| Ordering | `{ Person(orderBy: { name: ASC }) { ... } }` |
| Nested relations | `{ Person { friends { name } } }` |
| Aliases | `{ alice: Person(name: "Alice") { ... } }` |
| Create mutation | `mutation { createPerson(name: "Alice") { id } }` |
| Delete mutation | `mutation { deletePerson(id: 123) }` |

## Basic Examples

### Simple Queries

```graphql
# Get all people with their names
{
  Person {
    name
  }
}

# Get multiple fields
{
  Person {
    name
    age
    email
  }
}
```

### Filtering with Arguments

```graphql
# Find person by name
{
  Person(name: "Alice") {
    name
    age
  }
}

# Filter by multiple properties
{
  Person(age: 30, city: "Seattle") {
    name
    email
  }
}
```

### Where Clause with Operators

Use the `where` argument for advanced filtering with comparison operators:

```graphql
# Greater than
{
  Person(where: { age_gt: 30 }) {
    name
    age
  }
}

# Multiple conditions (AND)
{
  Person(where: { age_gte: 25, age_lte: 40 }) {
    name
    age
  }
}

# String operations
{
  Person(where: { name_contains: "Ali", email_ends_with: "@example.com" }) {
    name
    email
  }
}
```

**Supported operators:**

| Suffix | Operator | Example |
|--------|----------|---------|
| (none) | Equals | `name: "Alice"` |
| `_gt` | Greater than | `age_gt: 30` |
| `_gte` | Greater than or equal | `age_gte: 30` |
| `_lt` | Less than | `age_lt: 50` |
| `_lte` | Less than or equal | `age_lte: 50` |
| `_ne` | Not equal | `status_ne: "inactive"` |
| `_contains` | Contains substring | `name_contains: "Ali"` |
| `_starts_with` | Starts with | `name_starts_with: "A"` |
| `_ends_with` | Ends with | `email_ends_with: ".com"` |
| `_in` | In list | `status_in: ["active", "pending"]` |

### Pagination

Use `first` and `skip` to paginate results:

```graphql
# Get first 10 people
{
  Person(first: 10) {
    name
  }
}

# Skip first 20, get next 10
{
  Person(first: 10, skip: 20) {
    name
  }
}

# With filtering
{
  Person(where: { age_gt: 25 }, first: 5) {
    name
    age
  }
}
```

### Ordering

Use `orderBy` to sort results:

```graphql
# Sort by name ascending
{
  Person(orderBy: { name: ASC }) {
    name
    age
  }
}

# Sort by age descending
{
  Person(orderBy: { age: DESC }) {
    name
    age
  }
}

# Multiple sort keys
{
  Person(orderBy: { age: DESC, name: ASC }) {
    name
    age
  }
}

# Combined with pagination
{
  Person(orderBy: { age: DESC }, first: 10) {
    name
    age
  }
}
```

### Nested Relationships

```graphql
# Get person and their friends
{
  Person(name: "Alice") {
    name
    friends {
      name
      age
    }
  }
}

# Multiple levels deep
{
  Person(name: "Alice") {
    name
    friends {
      name
      friends {
        name
      }
    }
  }
}
```

### Using Aliases

```graphql
# Query multiple people with aliases
{
  alice: Person(name: "Alice") {
    name
    age
  }
  bob: Person(name: "Bob") {
    name
    age
  }
}
```

## Mutations

GraphQL mutations allow you to create and delete nodes in the graph.

### Creating Nodes

Use `create<Type>` to create new nodes:

```graphql
# Create a new person
mutation {
  createPerson(name: "Alice", age: 30) {
    id
    name
  }
}

# Create with multiple properties
mutation {
  createUser(name: "Bob", email: "bob@example.com", active: true) {
    id
    name
    email
  }
}
```

The mutation field name follows the pattern `create<NodeLabel>`. Arguments become node properties.

### Deleting Nodes

Use `delete<Type>` to delete nodes:

```graphql
# Delete by ID
mutation {
  deletePerson(id: 123)
}

# Delete by property
mutation {
  deleteUser(email: "bob@example.com")
}
```

### Mutation Examples in Python

```python
import graphos

db = graphos.GraphosDB()

# Create a node with GraphQL
result = db.execute_graphql('''
mutation {
  createPerson(name: "Alice", age: 30) {
    id
    name
  }
}
''')

# Query the created node
result = db.execute_graphql('''
{
  Person(name: "Alice") {
    name
    age
  }
}
''')

# Delete the node
db.execute_graphql('''
mutation {
  deletePerson(name: "Alice")
}
''')
```

## Python Usage

```python
import graphos

db = graphos.GraphosDB()

# Create some data using GQL
db.execute("INSERT (:Person {name: 'Alice', age: 30})")
db.execute("INSERT (:Person {name: 'Bob', age: 25})")
db.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    INSERT (a)-[:friends]->(b)
""")

# Query with GraphQL
result = db.execute_graphql('''
{
  Person(name: "Alice") {
    name
    age
    friends {
      name
    }
  }
}
''')

for row in result:
    print(row)
```

## Rust Usage

```rust
use graphos_engine::GraphosDB;

let db = GraphosDB::new_in_memory();

// Create data
db.execute("INSERT (:Person {name: 'Alice'})").unwrap();

// Query with GraphQL
let result = db.execute_graphql(r#"
{
  Person {
    name
  }
}
"#).unwrap();
```

## How It Maps to the Graph

GraphQL queries are translated to graph traversals:

| GraphQL | Graph Operation |
|---------|-----------------|
| Type name (e.g., `Person`) | Node label filter |
| Arguments | Property filters |
| Nested field | Edge traversal |
| Field name | Property access |

### Example Translation

```graphql
{
  Person(name: "Alice") {
    age
    friends {
      name
    }
  }
}
```

Translates to:

1. Find nodes with label `Person` where `name = "Alice"`
2. Return the `age` property
3. Traverse `friends` edges
4. Return `name` property of connected nodes

## Supported Features

### Query Operations
- Root type queries (label-based node selection)
- Field selection (property access)
- Arguments (property filtering)
- Where clause with comparison operators (`_gt`, `_lt`, `_contains`, etc.)
- Pagination (`first`, `skip`)
- Ordering (`orderBy` with `ASC`/`DESC`)
- Nested selections (relationship traversal)
- Aliases (multiple queries with names)
- Fragments (named and inline)

### Mutation Operations

- `create<Type>` - Create new nodes with properties
- `delete<Type>` - Delete nodes by property match

### Type Mapping
- GraphQL types map to node labels
- GraphQL fields map to properties or relationships
- Scalar fields return property values
- Object fields traverse relationships

## Learn More

<div class="grid cards" markdown>

-   **[Basic Queries](basic-queries.md)**

    ---

    Types, fields, and simple selections.

-   **[Arguments](arguments.md)**

    ---

    Filtering with query arguments.

-   **[Relationships](relationships.md)**

    ---

    Nested queries and traversals.

-   **[Aliases & Fragments](aliases.md)**

    ---

    Advanced query composition.

</div>
