---
title: Quick Start
description: Get up and running with Grafeo in 5 minutes.
---

# Quick Start

This guide will get you up and running with Grafeo in just a few minutes.

## Create a Database

=== "Python"

    ```python
    import grafeo

    # Create an in-memory database
    db = grafeo.GrafeoDB()

    # Or create a persistent database
    db = grafeo.GrafeoDB(path="my_graph.db")
    ```

=== "Rust"

    ```rust
    use grafeo::Database;

    // Create an in-memory database
    let db = Database::open_in_memory()?;

    // Or create a persistent database
    let db = Database::open("my_graph.db")?;
    ```

## Add Data

Use GQL to insert nodes and edges:

=== "Python"

    ```python
    with db.session() as session:
        # Create nodes
        session.execute("""
            INSERT (:Person {name: 'Alice', age: 30})
            INSERT (:Person {name: 'Bob', age: 25})
            INSERT (:Person {name: 'Carol', age: 35})
        """)

        # Create edges
        session.execute("""
            MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
            INSERT (a)-[:KNOWS {since: 2020}]->(b)
        """)

        session.execute("""
            MATCH (b:Person {name: 'Bob'}), (c:Person {name: 'Carol'})
            INSERT (b)-[:KNOWS {since: 2022}]->(c)
        """)
    ```

=== "Rust"

    ```rust
    let session = db.session()?;

    // Create nodes
    session.execute(r#"
        INSERT (:Person {name: 'Alice', age: 30})
        INSERT (:Person {name: 'Bob', age: 25})
        INSERT (:Person {name: 'Carol', age: 35})
    "#)?;

    // Create edges
    session.execute(r#"
        MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'})
        INSERT (a)-[:KNOWS {since: 2020}]->(b)
    "#)?;

    session.execute(r#"
        MATCH (b:Person {name: 'Bob'}), (c:Person {name: 'Carol'})
        INSERT (b)-[:KNOWS {since: 2022}]->(c)
    "#)?;
    ```

## Query Data

Retrieve data using pattern matching:

=== "Python"

    ```python
    with db.session() as session:
        # Find all people
        result = session.execute("""
            MATCH (p:Person)
            RETURN p.name, p.age
            ORDER BY p.age
        """)

        for row in result:
            print(f"{row['p.name']} is {row['p.age']} years old")

        # Find who Alice knows
        result = session.execute("""
            MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(friend)
            RETURN friend.name
        """)

        for row in result:
            print(f"Alice knows {row['friend.name']}")

        # Find friends of friends
        result = session.execute("""
            MATCH (a:Person {name: 'Alice'})-[:KNOWS]->()-[:KNOWS]->(fof)
            RETURN DISTINCT fof.name
        """)

        for row in result:
            print(f"Friend of friend: {row['fof.name']}")
    ```

=== "Rust"

    ```rust
    let session = db.session()?;

    // Find all people
    let result = session.execute(r#"
        MATCH (p:Person)
        RETURN p.name, p.age
        ORDER BY p.age
    "#)?;

    for row in result {
        println!("{} is {} years old",
            row.get::<String>("p.name")?,
            row.get::<i64>("p.age")?
        );
    }

    // Find who Alice knows
    let result = session.execute(r#"
        MATCH (a:Person {name: 'Alice'})-[:KNOWS]->(friend)
        RETURN friend.name
    "#)?;

    for row in result {
        println!("Alice knows {}", row.get::<String>("friend.name")?);
    }
    ```

## Update Data

Modify existing nodes and edges:

=== "Python"

    ```python
    with db.session() as session:
        # Update a property
        session.execute("""
            MATCH (p:Person {name: 'Alice'})
            SET p.age = 31
        """)

        # Add a new property
        session.execute("""
            MATCH (p:Person {name: 'Bob'})
            SET p.email = 'bob@example.com'
        """)
    ```

=== "Rust"

    ```rust
    let session = db.session()?;

    // Update a property
    session.execute(r#"
        MATCH (p:Person {name: 'Alice'})
        SET p.age = 31
    "#)?;
    ```

## Delete Data

Remove nodes and edges:

=== "Python"

    ```python
    with db.session() as session:
        # Delete an edge
        session.execute("""
            MATCH (a:Person {name: 'Alice'})-[r:KNOWS]->(b:Person {name: 'Bob'})
            DELETE r
        """)

        # Delete a node (must delete connected edges first)
        session.execute("""
            MATCH (p:Person {name: 'Carol'})
            DETACH DELETE p
        """)
    ```

=== "Rust"

    ```rust
    let session = db.session()?;

    // Delete a node and its edges
    session.execute(r#"
        MATCH (p:Person {name: 'Carol'})
        DETACH DELETE p
    "#)?;
    ```

## Next Steps

- [Your First Graph](first-graph.md) - Build a complete graph application
- [GQL Query Language](../user-guide/gql/index.md) - Learn more about queries
- [Python API](../user-guide/python/index.md) - Python-specific features
- [Rust API](../user-guide/rust/index.md) - Rust-specific features
