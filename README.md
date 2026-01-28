# Graphos

[![Crates.io](https://img.shields.io/crates/v/graphos.svg)](https://crates.io/crates/graphos)
[![PyPI](https://img.shields.io/pypi/v/pygraphos.svg)](https://pypi.org/project/pygraphos/)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Docs](https://img.shields.io/badge/docs-graphos.tech-blue)](https://graphos.tech)

A pure-Rust, high-performance, embeddable graph database supporting both **Labeled Property Graph (LPG)** and **RDF** data models.

## Features

- **Dual data model support**: LPG and RDF with optimized storage for each
- **Multi-language queries**: GQL, Cypher, Gremlin, GraphQL, and SPARQL
- **GQL** (ISO/IEC 39075) - enabled by default
- **Cypher** (openCypher 9.0) - via feature flag
- **Gremlin** (Apache TinkerPop) - via feature flag
- **GraphQL** - via feature flag, supports both LPG and RDF
- **SPARQL** (W3C 1.1) - via feature flag for RDF queries
- Embeddable with zero external dependencies
- Python bindings via PyO3
- In-memory and persistent storage modes
- MVCC transactions with snapshot isolation

## Query Language & Data Model Support

| Query Language | LPG | RDF | Status |
|----------------|-----|-----|--------|
| GQL (ISO/IEC 39075) | ✅ | — | Default |
| Cypher (openCypher 9.0) | ✅ | — | Feature flag |
| Gremlin (Apache TinkerPop) | ✅ | — | Feature flag |
| GraphQL | ✅ | ✅ | Feature flag |
| SPARQL (W3C 1.1) | — | ✅ | Feature flag |

Graphos uses a modular translator architecture where query languages are parsed into ASTs, then translated to a unified logical plan that executes against the appropriate storage backend (LPG or RDF).

### Data Models

- **LPG (Labeled Property Graph)**: Nodes with labels and properties, edges with types and properties. Ideal for social networks, knowledge graphs, and application data.
- **RDF (Resource Description Framework)**: Triple-based storage (subject-predicate-object) with SPO/POS/OSP indexes. Ideal for semantic web, linked data, and ontology-based applications.

## Installation

### Rust

```bash
cargo add graphos-engine
```

With additional query languages:

```bash
cargo add graphos-engine --features cypher   # Add Cypher support
cargo add graphos-engine --features gremlin  # Add Gremlin support
cargo add graphos-engine --features graphql  # Add GraphQL support
cargo add graphos-engine --features full     # All query languages
```

### Python

```bash
uv add pygraphos
```

## Quick Start

### Python

```python
import graphos

# Create an in-memory database
db = graphos.GraphosDB()

# Or open/create a persistent database
# db = graphos.GraphosDB("/path/to/database")

# Create nodes using GQL
db.execute("INSERT (:Person {name: 'Alice', age: 30})")
db.execute("INSERT (:Person {name: 'Bob', age: 25})")

# Create a relationship
db.execute("""
    MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
    INSERT (a)-[:KNOWS {since: 2020}]->(b)
""")

# Query the graph
result = db.execute("""
    MATCH (p:Person)-[:KNOWS]->(friend)
    RETURN p.name, friend.name
""")

for row in result:
    print(row)

# Or use the direct API
node = db.create_node(["Person"], {"name": "Carol"})
print(f"Created node with ID: {node.id}")
```

### Rust

```rust
use graphos_engine::GraphosDB;

fn main() {
    // Create an in-memory database
    let db = GraphosDB::new_in_memory();

    // Or open a persistent database
    // let db = GraphosDB::open("./my_database").unwrap();

    // Execute GQL queries
    db.execute("INSERT (:Person {name: 'Alice'})").unwrap();

    let result = db.execute("MATCH (p:Person) RETURN p.name").unwrap();
    for row in result.rows {
        println!("{:?}", row);
    }
}
```

## Documentation

Full documentation is available at [graphos.tech](https://graphos.tech).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines.

## License

Apache-2.0
