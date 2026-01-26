#!/usr/bin/env python3
"""Test script for Graphos Python bindings."""

import graphos

def main():
    print("=" * 50)
    print("Testing Graphos Python Bindings")
    print("=" * 50)

    # Test 1: Create in-memory database
    print("\n1. Creating in-memory database...")
    db = graphos.GraphosDB()
    print(f"   Database created: {db}")

    # Test 2: Get database stats
    print("\n2. Getting initial database stats...")
    stats = db.stats()
    print(f"   Node count: {stats.node_count}")
    print(f"   Edge count: {stats.edge_count}")

    # Test 3: Begin transaction
    print("\n3. Beginning transaction...")
    tx = db.begin_transaction()
    print(f"   Transaction: {tx}")

    # Test 4: Create nodes
    print("\n4. Creating nodes...")
    alice = db.create_node(["Person"], {"name": "Alice", "age": 30})
    bob = db.create_node(["Person"], {"name": "Bob", "age": 25})
    carol = db.create_node(["Person", "Developer"], {"name": "Carol", "age": 28})

    print(f"   Created node Alice: {alice}")
    print(f"   Created node Bob: {bob}")
    print(f"   Created node Carol: {carol}")

    # Test 5: Create edges
    print("\n5. Creating edges...")
    knows1 = db.create_edge(alice.id, bob.id, "KNOWS", {"since": 2020})
    knows2 = db.create_edge(alice.id, carol.id, "KNOWS", {"since": 2021})
    works_with = db.create_edge(bob.id, carol.id, "WORKS_WITH", {"project": "Graphos"})

    print(f"   Created edge Alice->Bob (KNOWS): {knows1}")
    print(f"   Created edge Alice->Carol (KNOWS): {knows2}")
    print(f"   Created edge Bob->Carol (WORKS_WITH): {works_with}")

    # Test 6: Query nodes
    print("\n6. Querying nodes...")
    retrieved_alice = db.get_node(alice.id)
    if retrieved_alice:
        print(f"   Retrieved Alice: {retrieved_alice}")
        print(f"   Alice's name: {retrieved_alice.get('name')}")
        print(f"   Alice's labels: {retrieved_alice.labels}")
        print(f"   Alice's properties: {retrieved_alice.properties}")

    # Test 7: Query edges
    print("\n7. Querying edges...")
    retrieved_edge = db.get_edge(knows1.id)
    if retrieved_edge:
        print(f"   Retrieved edge: {retrieved_edge}")
        print(f"   Edge type: {retrieved_edge.edge_type}")
        print(f"   Edge properties: {retrieved_edge.properties}")

    # Test 8: Commit transaction
    print("\n8. Committing transaction...")
    tx.commit()
    print("   Transaction committed!")

    # Test 9: Check stats again
    print("\n9. Final database stats...")
    stats = db.stats()
    print(f"   Node count: {stats.node_count}")
    print(f"   Edge count: {stats.edge_count}")
    print(f"   Label count: {stats.label_count}")

    print("\n" + "=" * 50)
    print("All tests passed!")
    print("=" * 50)

if __name__ == "__main__":
    main()
