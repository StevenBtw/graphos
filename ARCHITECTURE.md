# Graphos Architecture

A pure-Rust, high-performance, embeddable graph database with Python bindings.

## Design Principles

1. **Pure Rust** - No C/C++ dependencies, maximum safety and performance
2. **Embeddable** - Library-first design, importable from Python
3. **Fast** - Vectorized execution, morsel-driven parallelism, worst-case optimal joins
4. **Flexible** - LPG and RDF support, GQL and Cypher query languages
5. **ACID** - Full transactional guarantees with MVCC
6. **Extensible** - Plugin architecture for algorithms and integrations

## System Overview

```text
Python/Rust App
       │
       ▼
   GraphosDB ──► Query ──► Plan ──► Optimize ──► Execute
       │                                             │
       ▼                                             ▼
   Transaction                                   Results
   Manager (MVCC)                            (Arrow/Polars)
       │
       ▼
   Graph Store (Arena + Structural Sharing)
       │
       ▼
   Persistence (WAL + Snapshots)
```

## Architecture Layers

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                            BINDINGS LAYER                                    │
│  ┌───────────────────────┐  ┌───────────────────┐  ┌───────────────────┐   │
│  │    Python (PyO3)      │  │    C FFI          │  │    WASM           │   │
│  │  - Sync API           │  │    (future)       │  │    (future)       │   │
│  │  - Async API          │  │                   │  │                   │   │
│  │  - Polars/Arrow       │  │                   │  │                   │   │
│  │  - NetworkX compat    │  │                   │  │                   │   │
│  └───────────┬───────────┘  └─────────┬─────────┘  └─────────┬─────────┘   │
│              └────────────────────────┴────────────────────────┘            │
│                                       │                                      │
│                          ═════════════╧═════════════                        │
│                              CANNOT IMPORT ↓                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            ENGINE LAYER                                      │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         GraphosDB                                      │  │
│  │  - Database lifecycle (open, close, checkpoint)                       │  │
│  │  - Session/Connection management                                       │  │
│  │  - Configuration (memory limits, parallelism, storage mode)           │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                       │                                      │
│          ┌────────────────────────────┼────────────────────────────┐        │
│          ▼                            ▼                            ▼        │
│  ┌─────────────────┐      ┌─────────────────────┐      ┌─────────────────┐  │
│  │  Transaction    │      │   Query Processor   │      │    Catalog      │  │
│  │    Manager      │      │                     │      │    Manager      │  │
│  │                 │      │  ┌───────────────┐  │      │                 │  │
│  │  - MVCC epochs  │      │  │    Parser     │  │      │  - Node types   │  │
│  │  - Read/write   │      │  │  (GQL/Cypher) │  │      │  - Edge types   │  │
│  │    isolation    │      │  └───────┬───────┘  │      │  - Properties   │  │
│  │  - Commit/abort │      │          ▼          │      │  - Constraints  │  │
│  │  - Deadlock     │      │  ┌───────────────┐  │      │  - Indexes      │  │
│  │    detection    │      │  │    Binder     │  │      │                 │  │
│  │                 │      │  │  (semantic)   │  │      │                 │  │
│  └─────────────────┘      │  └───────┬───────┘  │      └─────────────────┘  │
│                           │          ▼          │                            │
│                           │  ┌───────────────┐  │                            │
│                           │  │   Planner     │  │                            │
│                           │  │  (logical)    │  │                            │
│                           │  └───────┬───────┘  │                            │
│                           │          ▼          │                            │
│                           │  ┌───────────────┐  │                            │
│                           │  │  Optimizer    │  │                            │
│                           │  │  - Pushdown   │  │                            │
│                           │  │  - Join order │  │                            │
│                           │  │  - Cardinality│  │                            │
│                           │  └───────┬───────┘  │                            │
│                           │          ▼          │                            │
│                           │  ┌───────────────┐  │                            │
│                           │  │   Executor    │  │                            │
│                           │  │  (physical)   │  │                            │
│                           │  └───────────────┘  │                            │
│                           └─────────────────────┘                            │
│                          ═════════════╧═════════════                        │
│                              CANNOT IMPORT ↓                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ADAPTERS LAYER                                     │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐  │
│  │   Storage Adapter   │  │  Query Language     │  │   Plugin Adapter    │  │
│  │                     │  │     Adapters        │  │                     │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │ Memory-only   │  │  │  │  GQL Parser   │  │  │  │   NetworkX    │  │  │
│  │  │ (default)     │  │  │  │  (default)    │  │  │  │   Bridge      │  │  │
│  │  └───────────────┘  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │ Mmap-backed   │  │  │  │ Cypher Parser │  │  │  │   solvOR      │  │  │
│  │  │ (persistence) │  │  │  │ (feature flag)│  │  │  │   Bridge      │  │  │
│  │  └───────────────┘  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  ┌───────────────┐  │  │                     │  │  ┌───────────────┐  │  │
│  │  │ WAL Manager   │  │  │                     │  │  │ Custom Plugin │  │  │
│  │  │ (durability)  │  │  │                     │  │  │   Trait       │  │  │
│  │  └───────────────┘  │  │                     │  │  └───────────────┘  │  │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘  │
│                          ═════════════╧═════════════                        │
│                              CANNOT IMPORT ↓                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                             CORE LAYER                                       │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐  │
│  │    Graph Models     │  │   Index Structures  │  │ Execution Primitives│  │
│  │                     │  │                     │  │                     │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │  LPG Model    │  │  │  │  Hash Index   │  │  │  │  DataChunk    │  │  │
│  │  │  (default)    │  │  │  │  (primary key)│  │  │  │  (2048 tuples)│  │  │
│  │  │               │  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  │  - Node       │  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │  - Edge       │  │  │  │  BTree Index  │  │  │  │  ValueVector  │  │  │
│  │  │  - Property   │  │  │  │  (range query)│  │  │  │  (columnar)   │  │  │
│  │  │  - Label      │  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  └───────────────┘  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  ┌───────────────┐  │  │  │ Chunked Adj   │  │  │  │ SelectionVec  │  │  │
│  │  │  RDF Model    │  │  │  │ (edges+delta) │  │  │  │  (filtering)  │  │  │
│  │  │  (optional)   │  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  │               │  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │  - Subject    │  │  │  │  Trie Index   │  │  │  │  Operators    │  │  │
│  │  │  - Predicate  │  │  │  │ (WCOJ, lazy)  │  │  │  │  (physical)   │  │  │
│  │  │  - Object     │  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  │  └───────────────┘  │  │                     │  │                     │  │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘  │
│                          ═════════════╧═════════════                        │
│                              CANNOT IMPORT ↓                                 │
└─────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            COMMON LAYER                                      │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐  │
│  │       Types         │  │       Memory        │  │       Utils         │  │
│  │                     │  │                     │  │                     │  │
│  │  - NodeId (u64)     │  │  - Arena allocator  │  │  - FxHash (fast)    │  │
│  │  - EdgeId (u64)     │  │  - Epoch tracking   │  │  - Serialization    │  │
│  │  - PropertyKey      │  │  - Pool allocator   │  │  - Error types      │  │
│  │  - Value (enum)     │  │  - NUMA-aware       │  │  - Result types     │  │
│  │  - LogicalType      │  │  - Bump allocator   │  │  - Metrics/tracing  │  │
│  │  - Timestamp        │  │                     │  │                     │  │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Storage Model: Hybrid Arena + WAL

### Overview

Graphos uses a novel storage architecture combining:
- **Epoch-based arena allocation** for cache-friendly memory layout
- **Structural sharing** for memory-efficient MVCC
- **Write-Ahead Logging** for durability
- **Async snapshots** for fast recovery

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TRANSACTION MANAGER                                  │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │  Active Txns    │  │ Committed Vers  │  │  Version GC     │             │
│  │                 │  │                 │  │                 │             │
│  │  TxId -> Epoch  │  │  Epoch -> Root  │  │  Prune old      │             │
│  │  Read set       │  │  Timestamp      │  │  epochs when    │             │
│  │  Write set      │  │  Visibility     │  │  no readers     │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
└────────────────────────────────┬────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                            GRAPH STORE                                       │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    Epoch-Based Arena Allocator                         │  │
│  │                                                                        │  │
│  │   Epoch 1 (old)        Epoch 2 (current)      Epoch 3 (new txn)       │  │
│  │  ┌──────────────┐     ┌──────────────┐       ┌──────────────┐         │  │
│  │  │ ┌──────────┐ │     │ ┌──────────┐ │       │ ┌──────────┐ │         │  │
│  │  │ │  Nodes   │ │     │ │  Nodes   │ │       │ │  Nodes   │ │         │  │
│  │  │ │  Arena   │ │     │ │  Arena   │ │       │ │  Arena   │ │         │  │
│  │  │ └──────────┘ │     │ └──────────┘ │       │ └──────────┘ │         │  │
│  │  │ ┌──────────┐ │     │ ┌──────────┐ │       │ ┌──────────┐ │         │  │
│  │  │ │  Edges   │ │     │ │  Edges   │ │       │ │  Edges   │ │         │  │
│  │  │ │  Arena   │ │     │ │  Arena   │ │       │ │  Arena   │ │         │  │
│  │  │ └──────────┘ │     │ └──────────┘ │       │ └──────────┘ │         │  │
│  │  │ ┌──────────┐ │     │ ┌──────────┐ │       │ ┌──────────┐ │         │  │
│  │  │ │Properties│ │     │ │Properties│ │       │ │Properties│ │         │  │
│  │  │ │  Arena   │ │     │ │  Arena   │ │       │ │  Arena   │ │         │  │
│  │  │ └──────────┘ │     │ └──────────┘ │       │ └──────────┘ │         │  │
│  │  └──────────────┘     └──────────────┘       └──────────────┘         │  │
│  │         │                    │                      │                  │  │
│  │         └────────────────────┴──────────────────────┘                  │  │
│  │                              │                                         │  │
│  │                    Structural Sharing                                  │  │
│  │         (unchanged nodes/edges shared across epochs)                   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                       Index Structures                                 │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │  │
│  │  │  NodeId →   │  │  Label →    │  │  Property   │  │  Chunked    │   │  │
│  │  │   Node*     │  │  NodeId[]   │  │   Index     │  │  Adjacency  │   │  │
│  │  │  (Hash)     │  │  (Hash)     │  │  (BTree)    │  │  (+Delta)   │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────┬────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        PERSISTENCE LAYER                                     │
│                                                                              │
│  ┌─────────────────────────────────┐  ┌─────────────────────────────────┐   │
│  │      Write-Ahead Log (WAL)      │  │      Async Snapshots            │   │
│  │                                 │  │                                 │   │
│  │  ┌───────────────────────────┐  │  │  ┌───────────────────────────┐  │   │
│  │  │  Log Entry Format:        │  │  │  │  Snapshot Strategy:       │  │   │
│  │  │                           │  │  │  │                           │  │   │
│  │  │  ┌─────────────────────┐  │  │  │  │  - Fork-based COW         │  │   │
│  │  │  │ TxId (u64)          │  │  │  │  │    (Unix) or              │  │   │
│  │  │  │ Timestamp (u64)     │  │  │  │  │  - Background thread      │  │   │
│  │  │  │ Operation (enum)    │  │  │  │  │    with epoch pinning     │  │   │
│  │  │  │ Payload (bytes)     │  │  │  │  │                           │  │   │
│  │  │  │ Checksum (u32)      │  │  │  │  │  - Incremental deltas     │  │   │
│  │  │  └─────────────────────┘  │  │  │  │  - Compressed output      │  │   │
│  │  │                           │  │  │  │                           │  │   │
│  │  │  Operations:              │  │  │  └───────────────────────────┘  │   │
│  │  │  - CreateNode             │  │  │                                 │   │
│  │  │  - DeleteNode             │  │  │  Recovery:                      │   │
│  │  │  - CreateEdge             │  │  │  1. Load latest snapshot        │   │
│  │  │  - DeleteEdge             │  │  │  2. Replay WAL from checkpoint  │   │
│  │  │  - SetProperty            │  │  │  3. Rebuild indexes             │   │
│  │  │  - TxCommit               │  │  │                                 │   │
│  │  │  - TxAbort                │  │  │                                 │   │
│  │  │  - Checkpoint             │  │  │                                 │   │
│  │  └───────────────────────────┘  │  └─────────────────────────────────┘   │
│  └─────────────────────────────────┘                                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Arena Allocation Benefits

1. **Cache Locality**: Nodes/edges allocated contiguously
2. **Fast Allocation**: Bump pointer, no free-list management
3. **Bulk Deallocation**: Drop entire epoch when no readers
4. **NUMA Awareness**: Per-socket arenas for large graphs

### Structural Sharing for MVCC

```text
Transaction T1 (read-only)     Transaction T2 (write)
sees Epoch 2                   creates Epoch 3
        │                              │
        ▼                              ▼
   ┌─────────┐                    ┌─────────┐
   │ Root v2 │                    │ Root v3 │
   └────┬────┘                    └────┬────┘
        │                              │
   ┌────┴────┐                    ┌────┴────┐
   ▼         ▼                    ▼         ▼
┌─────┐   ┌─────┐              ┌─────┐   ┌─────┐
│  A  │   │  B  │              │  A  │   │ B'  │ ← Modified
└─────┘   └─────┘              └─────┘   └─────┘
   ▲                              │
   └──────────────────────────────┘
         Shared (immutable)
```

### Adjacency Storage: Chunked Lists + Delta Buffer

**Why not CSR?** CSR (Compressed Sparse Row) is excellent for read throughput but painful to update incrementally. Inserting an edge requires shifting all subsequent elements - O(E) per insert. For write-heavy workloads with MVCC, this is unacceptable.

**Solution: Chunked Adjacency Lists with Delta Buffers**

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    ADJACENCY STORAGE (Per Edge Type)                         │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                 Forward Lists (outgoing edges)                         │  │
│  │                                                                        │  │
│  │  Node 0    ┌──────────────┐   ┌──────────────┐                        │  │
│  │  ─────────►│   Chunk 0    │──►│   Chunk 1    │──► null                │  │
│  │            │ [1, 2, 3, 4] │   │ [5, 6, _, _] │                        │  │
│  │            │  cap: 4      │   │  cap: 4      │                        │  │
│  │            └──────────────┘   └──────────────┘                        │  │
│  │                                                                        │  │
│  │  Node 1    ┌──────────────┐                                           │  │
│  │  ─────────►│   Chunk 0    │──► null                                   │  │
│  │            │ [0, 2, _, _] │   (2 slots available)                     │  │
│  │            │  cap: 4      │                                            │  │
│  │            └──────────────┘                                            │  │
│  │                                                                        │  │
│  │  Benefits:                                                             │  │
│  │  • Insert: O(1) amortized (append to last chunk or alloc new)         │  │
│  │  • Delete: Tombstone + lazy compaction                                 │  │
│  │  • Scan: Cache-friendly within chunks                                  │  │
│  │  • MVCC: Copy-on-write at chunk granularity                           │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                 Backward Lists (incoming edges) - OPTIONAL             │  │
│  │                                                                        │  │
│  │  Same structure as forward lists, maintained separately.              │  │
│  │  Enables efficient: MATCH (a)<-[:KNOWS]-(b)                           │  │
│  │                                                                        │  │
│  │  Config: config.backward_edges = true | false (default: true)         │  │
│  │                                                                        │  │
│  │  When disabled:                                                        │  │
│  │  • 2x less edge storage                                                │  │
│  │  • 2x less write overhead                                              │  │
│  │  • Backward traversal requires full scan (acceptable for some         │  │
│  │    workloads: event logs, append-only relationships, DAGs)            │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                 Delta Buffer (Hot Writes)                              │  │
│  │                                                                        │  │
│  │  For high write throughput, recent changes buffered before merge:     │  │
│  │                                                                        │  │
│  │  struct DeltaBuffer {                                                  │  │
│  │      inserts: HashMap<NodeId, SmallVec<[EdgeId; 4]>>,                 │  │
│  │      deletes: HashSet<(NodeId, EdgeId)>,                              │  │
│  │      epoch: EpochId,                                                   │  │
│  │  }                                                                     │  │
│  │                                                                        │  │
│  │  Query path:  Delta → Chunked Lists (merged view)                     │  │
│  │  Compaction:  Merge delta into chunks when |delta| > threshold        │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                 Trie Index (Built Lazily for WCOJ)                     │  │
│  │                                                                        │  │
│  │  NOT the primary storage - built on-demand for complex patterns:      │  │
│  │                                                                        │  │
│  │  • Triangle queries: MATCH (a)--(b)--(c)--(a)                         │  │
│  │  • Clique detection                                                    │  │
│  │  • Multi-way joins where WCOJ outperforms binary joins                │  │
│  │                                                                        │  │
│  │  Built at query time for relevant edge types, cached if beneficial.   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Memory Accounting (Realistic)

**Naive approach problem**: Using standard Rust types naively leads to high overhead:

| Component | Naive Size | Notes |
|-----------|-----------|-------|
| NodeId | 8 bytes | u64 |
| Labels (SmallVec) | 24 bytes | ptr + len + cap minimum |
| PropertyMap (HashMap) | 48+ bytes | Empty hashbrown overhead |
| MVCC version ptr | 8 bytes | |
| **Total (empty node)** | **~88 bytes** | Before any actual data! |

**Optimized approach**: Columnar storage with arena allocation

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         OPTIMIZED NODE STORAGE                               │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  NodeRecord (fixed 32 bytes, cache-line friendly)                      │  │
│  │                                                                        │  │
│  │  #[repr(C)]                                                            │  │
│  │  struct NodeRecord {                                                   │  │
│  │      id: NodeId,              //  8 bytes (u64)                        │  │
│  │      label_bits: u64,         //  8 bytes (bitmap, up to 64 labels)   │  │
│  │      props_offset: u32,       //  4 bytes (into property arena)       │  │
│  │      props_count: u16,        //  2 bytes                              │  │
│  │      flags: u16,              //  2 bytes (deleted, has_version, etc) │  │
│  │      epoch: EpochId,          //  8 bytes (u64 for large epoch space) │  │
│  │  }                            // Total: 32 bytes (naturally aligned)  │  │
│  │                                                                        │  │
│  │  // Note: With u64 epoch, struct is exactly 32 bytes:                 │  │
│  │  // 8 + 8 + 4 + 2 + 2 + 8 = 32 (no padding needed)                    │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  Property Storage (Columnar, per property key)                         │  │
│  │                                                                        │  │
│  │  "name" column:   StringPool + offset array                           │  │
│  │  "age" column:    [i64; N] dense array                                │  │
│  │  "active" column: BitVec (1 bit per node)                             │  │
│  │                                                                        │  │
│  │  Dense properties:  ~8 bytes/node/property (value only)               │  │
│  │  Sparse properties: ~12 bytes/node/property (offset + value)          │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  MVCC Overhead (Amortized)                                             │  │
│  │                                                                        │  │
│  │  Most nodes: 0 extra bytes (version in NodeRecord.version field)      │  │
│  │                                                                        │  │
│  │  Nodes with history (concurrent modifications):                        │  │
│  │  VersionChain stored separately in HashMap<NodeId, VersionChain>      │  │
│  │  ~40 bytes per historical version                                      │  │
│  │                                                                        │  │
│  │  Typical workload: <1% of nodes have version chains                   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Revised Memory Targets**:

| Scenario | Per Node | Notes |
|----------|----------|-------|
| Core record only | 32 bytes | Fixed NodeRecord |
| + 3 dense properties | 56 bytes | 32 + 3×8 |
| + 3 sparse properties | 68 bytes | 32 + 3×12 |
| + adjacency (avg 10 edges) | +80 bytes | ~8 bytes/edge in chunks |
| + MVCC history (rare) | +40 bytes | Only for modified nodes |

**Working set estimate**: 100-150 bytes/node typical (with properties + edges)

## Execution Model: Adaptive Hybrid

### Three Execution Strategies

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                        QUERY EXECUTOR                                        │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    Adaptive Query Router                               │  │
│  │                                                                        │  │
│  │  Input: Logical Plan + Cardinality Estimates                          │  │
│  │                                                                        │  │
│  │  Decision Matrix:                                                      │  │
│  │  ┌────────────────────┬───────────────────────────────────────────┐   │  │
│  │  │ Query Pattern      │ Strategy Selection                        │   │  │
│  │  ├────────────────────┼───────────────────────────────────────────┤   │  │
│  │  │ Simple scan/filter │ Vectorized Pipeline                       │   │  │
│  │  │ Binary joins       │ Vectorized + Morsel Parallel              │   │  │
│  │  │ Multi-way joins    │ WCOJ (triangle, clique, paths)            │   │  │
│  │  │ Aggregations       │ Vectorized with hash tables               │   │  │
│  │  │ Mixed workload     │ Hybrid: WCOJ core + Vectorized finish     │   │  │
│  │  └────────────────────┴───────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                       │                                      │
│              ┌────────────────────────┼────────────────────────┐            │
│              ▼                        ▼                        ▼            │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐  │
│  │    VECTORIZED       │  │   MORSEL-DRIVEN     │  │       WCOJ          │  │
│  │    PIPELINE         │  │    PARALLEL         │  │    (Leapfrog)       │  │
│  │                     │  │                     │  │                     │  │
│  │  Process 2048       │  │  Work-stealing      │  │  Worst-case         │  │
│  │  tuples at a time   │  │  scheduler          │  │  optimal joins      │  │
│  │                     │  │                     │  │                     │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │ Scan          │  │  │  │ Morsel Pool   │  │  │  │ Trie Index    │  │  │
│  │  │ (columnar)    │  │  │  │ (64K tuples)  │  │  │  │ (sorted)      │  │  │
│  │  └───────┬───────┘  │  │  └───────┬───────┘  │  │  └───────┬───────┘  │  │
│  │          ▼          │  │          ▼          │  │          ▼          │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │ Filter        │  │  │  │ Worker 1..N   │  │  │  │ Leapfrog      │  │  │
│  │  │ (SIMD)        │  │  │  │ (per-core)    │  │  │  │ Iterator      │  │  │
│  │  └───────┬───────┘  │  │  └───────┬───────┘  │  │  └───────┬───────┘  │  │
│  │          ▼          │  │          ▼          │  │          ▼          │  │
│  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │  ┌───────────────┐  │  │
│  │  │ Project       │  │  │  │ Local buffers │  │  │  │ Intersection  │  │  │
│  │  │ (selection)   │  │  │  │ (no locks)    │  │  │  │ (O(n) vs O(n²))│ │  │
│  │  └───────────────┘  │  │  └───────────────┘  │  │  └───────────────┘  │  │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘  │
│              │                        │                        │            │
│              └────────────────────────┴────────────────────────┘            │
│                                       │                                      │
│                                       ▼                                      │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                         DataChunk                                      │  │
│  │                                                                        │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│  │  │  Capacity: 2048 tuples (cache-line aligned)                     │  │  │
│  │  │                                                                  │  │  │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │  │  │
│  │  │  │ Vector 0 │ │ Vector 1 │ │ Vector 2 │ │ Vector N │           │  │  │
│  │  │  │ (NodeId) │ │ (Label)  │ │ (Prop A) │ │ (Prop N) │           │  │  │
│  │  │  │          │ │          │ │          │ │          │           │  │  │
│  │  │  │ [u64;2K] │ │ [u32;2K] │ │ [i64;2K] │ │ [T;2K]   │           │  │  │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │  │  │
│  │  │                                                                  │  │  │
│  │  │  SelectionVector: [u16; 2048] - indices of valid tuples         │  │  │
│  │  │  Count: usize - number of valid tuples                          │  │  │
│  │  └─────────────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### WCOJ (Worst-Case Optimal Join) for Graph Patterns

Traditional binary joins can be O(n²) for triangle queries. WCOJ achieves O(n^1.5):

```text
Query: MATCH (a)-[:KNOWS]->(b)-[:KNOWS]->(c)-[:KNOWS]->(a) -- Triangle

Binary Join Approach (bad):          WCOJ Leapfrog (good):

  KNOWS × KNOWS × KNOWS              Trie on KNOWS(src, dst):
  = O(|E|³) worst case
                                       src     dst
                                     ┌─────┬─────────┐
                                     │  1  │ [2,3,5] │
                                     │  2  │ [1,3]   │
                                     │  3  │ [1,2]   │
                                     └─────┴─────────┘

                                     Leapfrog intersects iterators:
                                     - For each a, get neighbors(a)
                                     - For each b in neighbors(a), get neighbors(b)
                                     - Intersect with neighbors(a) to find c

                                     = O(|E|^1.5) for triangles
```

## Graph Models: LPG and RDF

### Separate Implementations (No Abstraction Overhead)

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         GRAPH MODEL SELECTION                                │
│                                                                              │
│  GraphosDB::new(config)                                                      │
│       │                                                                      │
│       ├── config.model = GraphModel::LPG  ──────────────────┐               │
│       │                                                      ▼               │
│       │                                    ┌─────────────────────────────┐   │
│       │                                    │      LPG Implementation     │   │
│       │                                    │                             │   │
│       │                                    │  struct Node {              │   │
│       │                                    │    id: NodeId,              │   │
│       │                                    │    labels: SmallVec<Label>, │   │
│       │                                    │    properties: PropertyMap, │   │
│       │                                    │  }                          │   │
│       │                                    │                             │   │
│       │                                    │  struct Edge {              │   │
│       │                                    │    id: EdgeId,              │   │
│       │                                    │    type_: EdgeType,         │   │
│       │                                    │    src: NodeId,             │   │
│       │                                    │    dst: NodeId,             │   │
│       │                                    │    properties: PropertyMap, │   │
│       │                                    │  }                          │   │
│       │                                    │                             │   │
│       │                                    │  Indexes:                   │   │
│       │                                    │  - NodeId -> Node           │   │
│       │                                    │  - Label -> [NodeId]        │   │
│       │                                    │  - EdgeType -> [EdgeId]     │   │
│       │                                    │  - Chunked adjacency        │   │
│       │                                    └─────────────────────────────┘   │
│       │                                                                      │
│       └── config.model = GraphModel::RDF  ──────────────────┐               │
│                                                              ▼               │
│                                            ┌─────────────────────────────┐   │
│                                            │      RDF Implementation     │   │
│                                            │                             │   │
│                                            │  struct Triple {            │   │
│                                            │    subject: Term,           │   │
│                                            │    predicate: Term,         │   │
│                                            │    object: Term,            │   │
│                                            │  }                          │   │
│                                            │                             │   │
│                                            │  enum Term {                │   │
│                                            │    IRI(String),             │   │
│                                            │    BlankNode(u64),          │   │
│                                            │    Literal {                │   │
│                                            │      value: String,         │   │
│                                            │      datatype: IRI,         │   │
│                                            │      language: Option<Str>, │   │
│                                            │    },                       │   │
│                                            │  }                          │   │
│                                            │                             │   │
│                                            │  Indexes (6 permutations):  │   │
│                                            │  - SPO, SOP, PSO            │   │
│                                            │  - POS, OSP, OPS            │   │
│                                            └─────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Query Language Support

### GQL (Primary) + Cypher (Compile-Time Feature Flag)

Language selection is done at **compile time** via Cargo features, not at runtime.

```toml
# Cargo.toml
[features]
default = ["gql"]
gql = []                    # Always available, ISO/IEC 39075:2024
cypher = []                 # Opt-in, openCypher 9.0
full = ["gql", "cypher"]    # Both languages
```

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COMPILE-TIME LANGUAGE SELECTION                           │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      Feature-Gated Parsers                             │  │
│  │                                                                        │  │
│  │  #[cfg(feature = "gql")]                                               │  │
│  │  mod gql;      // Always compiled by default                           │  │
│  │                                                                        │  │
│  │  #[cfg(feature = "cypher")]                                            │  │
│  │  mod cypher;   // Only when --features cypher                          │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│            ┌────────────────────────────────────────┐                       │
│            ▼                                        ▼                       │
│  ┌─────────────────────────┐          ┌─────────────────────────┐          │
│  │      GQL Frontend       │          │   Cypher Frontend       │          │
│  │      (default)          │          │   (feature-gated)       │          │
│  │                         │          │                         │          │
│  │  ┌───────────────────┐  │          │  ┌───────────────────┐  │          │
│  │  │ Lexer + Parser    │  │          │  │ Lexer + Parser    │  │          │
│  │  │ (pest or logos)   │  │          │  │ (pest or logos)   │  │          │
│  │  └─────────┬─────────┘  │          │  └─────────┬─────────┘  │          │
│  │            ▼            │          │            ▼            │          │
│  │  ┌───────────────────┐  │          │  ┌───────────────────┐  │          │
│  │  │    GQL AST        │  │          │  │   Cypher AST      │  │          │
│  │  │  (with spans)     │  │          │  │  (with spans)     │  │          │
│  │  └─────────┬─────────┘  │          │  └─────────┬─────────┘  │          │
│  │            ▼            │          │            ▼            │          │
│  │  ┌───────────────────┐  │          │  ┌───────────────────┐  │          │
│  │  │ impl ToLogicalPlan│  │          │  │ impl ToLogicalPlan│  │          │
│  │  └───────────────────┘  │          │  └───────────────────┘  │          │
│  └────────────┬────────────┘          └────────────┬────────────┘          │
│               │                                    │                        │
│               └──────────────┬─────────────────────┘                        │
│                              ▼                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    Common IR (Logical Plan)                          │   │
│  │                                                                      │   │
│  │  // Graph-aware IR, not just SQL-like                                │   │
│  │  enum LogicalPlan {                                                  │   │
│  │      Scan { labels, alias, span },                                   │   │
│  │      Expand { src, edge_pattern, dst, direction, span },            │   │
│  │      Filter { predicate, span },                                     │   │
│  │      Project { expressions, span },                                  │   │
│  │      ShortestPath { src, dst, edge_filter, span },                  │   │
│  │      AllPaths { src, dst, min_hops, max_hops, span },               │   │
│  │      // ... etc                                                      │   │
│  │  }                                                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Error Messages with Source Spans

Errors reference the **original query syntax**, not internal IR:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ERROR HANDLING                                       │
│                                                                              │
│  pub struct SourceSpan {                                                    │
│      pub start: usize,       // Byte offset in original query               │
│      pub end: usize,                                                        │
│      pub line: u32,                                                         │
│      pub column: u32,                                                       │
│  }                                                                          │
│                                                                              │
│  pub struct QueryError {                                                    │
│      pub kind: QueryErrorKind,                                              │
│      pub message: String,                                                   │
│      pub span: Option<SourceSpan>,                                          │
│      pub source_query: Arc<str>,    // Original GQL/Cypher text            │
│      pub hint: Option<String>,      // "did you mean...?"                  │
│  }                                                                          │
│                                                                              │
│  // Pretty-printed error output:                                            │
│  //                                                                         │
│  // error[E0001]: Unknown node label 'Peron'                                │
│  //   --> query:1:8                                                         │
│  //    |                                                                    │
│  //  1 | MATCH (n:Peron) RETURN n                                           │
│  //    |         ^^^^^ unknown label                                        │
│  //    |                                                                    │
│  //  help: did you mean 'Person'?                                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Graphos Extensions (Minimal v1)

Extensions are handled via `CALL` statements, keeping parsers clean:

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         EXTENSION SYNTAX                                     │
│                                                                              │
│  GQL:                                                                       │
│    CALL graphos.pagerank(damping => 0.85)                                   │
│    YIELD node, score                                                        │
│    WHERE score > 0.1                                                        │
│    RETURN node.name, score                                                  │
│                                                                              │
│  Cypher:                                                                    │
│    CALL graphos.pagerank({damping: 0.85})                                   │
│    YIELD node, score                                                        │
│    WHERE score > 0.1                                                        │
│    RETURN node.name, score                                                  │
│                                                                              │
│  Implementation:                                                            │
│  - Parser recognizes CALL graphos.* as ProcedureCall AST node              │
│  - Engine resolves procedure from plugin registry                          │
│  - No language-specific extension syntax needed                            │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Plugin Architecture

### In-Process Plugin System

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PLUGIN SYSTEM                                       │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                        Plugin Trait                                    │  │
│  │                                                                        │  │
│  │  pub trait Plugin: Send + Sync {                                       │  │
│  │      fn name(&self) -> &str;                                           │  │
│  │      fn version(&self) -> &str;                                        │  │
│  │                                                                        │  │
│  │      // Register custom functions                                      │  │
│  │      fn register_functions(&self, registry: &mut FunctionRegistry);   │  │
│  │                                                                        │  │
│  │      // Register custom algorithms                                     │  │
│  │      fn register_algorithms(&self, registry: &mut AlgorithmRegistry); │  │
│  │                                                                        │  │
│  │      // Lifecycle hooks                                                │  │
│  │      fn on_load(&self, db: &GraphosDB) -> Result<()>;                 │  │
│  │      fn on_unload(&self, db: &GraphosDB) -> Result<()>;               │  │
│  │  }                                                                     │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                       │                                      │
│          ┌────────────────────────────┼────────────────────────────┐        │
│          ▼                            ▼                            ▼        │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐  │
│  │  NetworkX Bridge    │  │   solvOR   Bridge   │  │   Custom Plugins    │  │
│  │                     │  │                     │  │                     │  │
│  │  Algorithms:        │  │  Algorithms:        │  │  User-defined:      │  │
│  │  - pagerank()       │  │  - shortest_path()  │  │  - Functions        │  │
│  │  - betweenness()    │  │  - min_cost_flow()  │  │  - Algorithms       │  │
│  │  - clustering()     │  │  - tsp()            │  │  - Procedures       │  │
│  │  - communities()    │  │  - assignment()     │  │                     │  │
│  │  - connected_comp() │  │  - vehicle_routing()│  │                     │  │
│  │                     │  │                     │  │                     │  │
│  │  Python-side:       │  │  Python-side:       │  │                     │  │
│  │  - to_networkx()    │  │  - to_solvor()      │  │                     │  │
│  │  - from_networkx()  │  │                     │  │                     │  │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                     Algorithm Trait                                    │  │
│  │                                                                        │  │
│  │  pub trait Algorithm: Send + Sync {                                    │  │
│  │      fn name(&self) -> &str;                                           │  │
│  │      fn execute(                                                       │  │
│  │          &self,                                                        │  │
│  │          graph: &dyn GraphRead,                                        │  │
│  │          params: &Parameters,                                          │  │
│  │      ) -> Result<AlgorithmResult>;                                     │  │
│  │  }                                                                     │  │
│  │                                                                        │  │
│  │  // Can be invoked from queries:                                       │  │
│  │  // CALL graphos.pagerank({damping: 0.85}) YIELD node, score          │  │
│  │  // CALL graphos.shortest_path(a, b) YIELD path, cost                 │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Python API Design

```python
import graphos

# Create in-memory database (default: LPG model, GQL language)
db = graphos.Database()

# Or with configuration
db = graphos.Database(
    path="./my_graph.db",          # Optional persistence
    model=graphos.Model.LPG,       # or graphos.Model.RDF
    language=graphos.Language.GQL, # or graphos.Language.CYPHER
    memory_limit="4GB",
    threads=8,
)

# Sync API
with db.session() as session:
    # DDL
    session.execute("""
        CREATE NODE TYPE Person (
            name STRING NOT NULL,
            age INT64,
            PRIMARY KEY (name)
        )
    """)

    # DML
    session.execute("""
        INSERT (:Person {name: 'Alice', age: 30})
        INSERT (:Person {name: 'Bob', age: 25})
        INSERT (:Person {name: 'Alice'})-[:KNOWS]->(:Person {name: 'Bob'})
    """)

    # Query
    result = session.execute("""
        MATCH (p:Person)-[:KNOWS]->(friend:Person)
        WHERE p.age > 25
        RETURN p.name, friend.name
    """)

    # Iterate results
    for row in result:
        print(row["p.name"], "knows", row["friend.name"])

    # Or convert to Polars DataFrame
    df = result.to_polars()

    # Or to Arrow Table
    table = result.to_arrow()

# Async API
async with db.async_session() as session:
    result = await session.execute_async("MATCH (n) RETURN n LIMIT 10")
    async for row in result:
        print(row)

# Plugin usage
from graphos.plugins import networkx as gnx

# Run PageRank
scores = gnx.pagerank(db, damping=0.85)

# Export to NetworkX for visualization
G = gnx.to_networkx(db)

# solvOR integration
from graphos.plugins import solvor as gor

path = gor.shortest_path(db, source="Alice", target="Bob")
```

## Rust Workspace Structure

```text
graphos/
├── Cargo.toml                         # Workspace manifest
├── architecture.md                    # This document
├── README.md
│
├── crates/
│   ├── graphos-common/                # Foundation layer (no internal deps)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types/
│   │       │   ├── mod.rs
│   │       │   ├── id.rs              # NodeId, EdgeId, TxId
│   │       │   ├── value.rs           # Value enum (all supported types)
│   │       │   ├── logical_type.rs    # Type system
│   │       │   └── timestamp.rs       # Transaction timestamps
│   │       ├── memory/
│   │       │   ├── mod.rs
│   │       │   ├── arena.rs           # Epoch-based arena allocator
│   │       │   ├── pool.rs            # Object pool
│   │       │   └── bump.rs            # Bump allocator
│   │       └── utils/
│   │           ├── mod.rs
│   │           ├── hash.rs            # FxHash, stable hashing
│   │           ├── error.rs           # Error types, Result alias
│   │           └── serde.rs           # Serialization helpers
│   │
│   ├── graphos-core/                  # Core layer (depends on common)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── graph/
│   │       │   ├── mod.rs
│   │       │   ├── lpg/               # Labeled Property Graph
│   │       │   │   ├── mod.rs
│   │       │   │   ├── node.rs
│   │       │   │   ├── edge.rs
│   │       │   │   ├── property.rs
│   │       │   │   └── store.rs       # LPG storage implementation
│   │       │   └── rdf/               # RDF Graph
│   │       │       ├── mod.rs
│   │       │       ├── triple.rs
│   │       │       ├── term.rs
│   │       │       └── store.rs       # RDF storage implementation
│   │       ├── index/
│   │       │   ├── mod.rs
│   │       │   ├── hash.rs            # Hash index (primary key)
│   │       │   ├── btree.rs           # BTree index (range queries)
│   │       │   ├── adjacency.rs       # Chunked adjacency lists + delta
│   │       │   └── trie.rs            # Trie for WCOJ
│   │       └── execution/
│   │           ├── mod.rs
│   │           ├── chunk.rs           # DataChunk (2048 tuples)
│   │           ├── vector.rs          # ValueVector (columnar)
│   │           ├── selection.rs       # SelectionVector
│   │           └── operators/
│   │               ├── mod.rs
│   │               ├── scan.rs
│   │               ├── filter.rs
│   │               ├── project.rs
│   │               ├── join.rs
│   │               ├── aggregate.rs
│   │               └── wcoj.rs        # Leapfrog trie join
│   │
│   ├── graphos-adapters/              # Adapters layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── storage/
│   │       │   ├── mod.rs
│   │       │   ├── memory.rs          # Pure in-memory backend
│   │       │   ├── mmap.rs            # Memory-mapped persistence
│   │       │   └── wal/
│   │       │       ├── mod.rs
│   │       │       ├── log.rs         # WAL implementation
│   │       │       ├── record.rs      # Log record types
│   │       │       └── recovery.rs    # Crash recovery
│   │       ├── query/
│   │       │   ├── mod.rs
│   │       │   ├── gql/               # GQL parser (always included)
│   │       │   │   ├── mod.rs
│   │       │   │   ├── lexer.rs
│   │       │   │   ├── parser.rs
│   │       │   │   └── ast.rs
│   │       │   └── cypher/            # Cypher parser (feature-gated)
│   │       │       ├── mod.rs
│   │       │       ├── lexer.rs
│   │       │       ├── parser.rs
│   │       │       └── ast.rs
│   │       └── plugins/
│   │           ├── mod.rs
│   │           ├── trait.rs           # Plugin trait definition
│   │           ├── registry.rs        # Plugin registry
│   │           ├── networkx.rs        # NetworkX algorithm bridge
│   │           └── solvor.rs          # solvOR algorithm bridge
│   │
│   ├── graphos-engine/                # Engine layer (main entry point)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── database.rs            # GraphosDB struct
│   │       ├── session.rs             # Session/Connection
│   │       ├── config.rs              # Configuration
│   │       ├── transaction/
│   │       │   ├── mod.rs
│   │       │   ├── manager.rs         # Transaction manager
│   │       │   ├── mvcc.rs            # MVCC implementation
│   │       │   └── isolation.rs       # Isolation levels
│   │       ├── query/
│   │       │   ├── mod.rs
│   │       │   ├── processor.rs       # Query processor
│   │       │   ├── binder.rs          # Semantic binding
│   │       │   ├── planner.rs         # Logical planning
│   │       │   ├── optimizer/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── filter_pushdown.rs
│   │       │   │   ├── projection_pushdown.rs
│   │       │   │   ├── join_reorder.rs
│   │       │   │   └── cardinality.rs
│   │       │   └── executor/
│   │       │       ├── mod.rs
│   │       │       ├── physical.rs    # Physical plan
│   │       │       ├── vectorized.rs  # Vectorized execution
│   │       │       ├── morsel.rs      # Morsel-driven parallelism
│   │       │       └── router.rs      # Adaptive strategy selection
│   │       └── catalog/
│   │           ├── mod.rs
│   │           ├── schema.rs          # Schema definitions
│   │           ├── constraint.rs      # Constraints
│   │           └── index.rs           # Index catalog
│   │
│   └── graphos-python/                # Python bindings
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs                 # PyModule definition
│           ├── database.rs            # Python Database class
│           ├── session.rs             # Python Session class
│           ├── result.rs              # QueryResult, Arrow/Polars
│           ├── async_api.rs           # Async Python API
│           └── types.rs               # Python type conversions
│
├── python/                            # Pure Python package
│   └── graphos/
│       ├── __init__.py
│       ├── database.py                # High-level Python wrapper
│       ├── session.py
│       ├── result.py
│       └── plugins/
│           ├── __init__.py
│           ├── networkx.py            # NetworkX integration
│           └── solvor.py              # solvOR integration
│
├── tests/
│   ├── integration/
│   │   ├── test_lpg.rs
│   │   ├── test_rdf.rs
│   │   ├── test_gql.rs
│   │   ├── test_cypher.rs
│   │   ├── test_mvcc.rs
│   │   └── test_wal.rs
│   └── benchmarks/
│       ├── bench_insert.rs
│       ├── bench_query.rs
│       └── bench_algorithms.rs
│
└── examples/
    ├── basic_usage.rs
    ├── python_example.py
    └── fraud_detection.py
```

## Cargo Workspace Configuration

```toml
# graphos/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/graphos-common",
    "crates/graphos-core",
    "crates/graphos-adapters",
    "crates/graphos-engine",
    "crates/graphos-python",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/yourusername/graphos"

[workspace.dependencies]
# Internal crates
graphos-common = { path = "crates/graphos-common" }
graphos-core = { path = "crates/graphos-core" }
graphos-adapters = { path = "crates/graphos-adapters" }
graphos-engine = { path = "crates/graphos-engine" }

# External dependencies (shared versions)
thiserror = "2.0"
anyhow = "1.0"
parking_lot = "0.12"
crossbeam = "0.8"
rayon = "1.10"
hashbrown = "0.15"
smallvec = "1.13"
bumpalo = "3.16"
memmap2 = "0.9"
crc32fast = "1.4"
byteorder = "1.5"
bytes = "1.7"
arrow = "53"
polars = "0.44"
pyo3 = "0.22"
tokio = { version = "1.41", features = ["full"] }
tracing = "0.1"
serde = { version = "1.0", features = ["derive"] }

[workspace.lints.rust]
unsafe_code = "warn"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
```

## Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Insert throughput | 1M nodes/sec | Single thread, batched |
| Edge insert | 500K edges/sec | With index updates |
| Point lookup | < 1μs | By primary key |
| 1-hop traversal | < 10μs | Single edge follow |
| 2-hop traversal | < 100μs | With filtering |
| Triangle query | < 1ms/1K triangles | Using WCOJ |
| PageRank (1M nodes) | < 1s | 10 iterations |
| Memory overhead | < 100 bytes/node | Core storage only |

## Development Phases

### Phase 1: Foundation (MVP)
- [ ] graphos-common: Types, memory, utils
- [ ] graphos-core: LPG model, basic indexes (Hash, Chunked Adjacency)
- [ ] graphos-core: DataChunk, basic operators (Scan, Filter, Project)
- [ ] graphos-engine: Basic GraphosDB, in-memory only
- [ ] graphos-adapters: GQL parser (subset)
- [ ] graphos-python: Basic PyO3 bindings

### Phase 2: Query Engine
- [ ] Full GQL parser
- [ ] Query planner and optimizer
- [ ] Vectorized execution
- [ ] Hash join, aggregations
- [ ] Cypher parser (feature flag)

### Phase 3: Transactions & Persistence
- [ ] MVCC transaction manager
- [ ] WAL implementation
- [ ] Snapshot/checkpoint
- [ ] Crash recovery

### Phase 4: Performance
- [ ] WCOJ (Leapfrog trie join)
- [ ] Morsel-driven parallelism
- [ ] Adaptive query routing
- [ ] SIMD optimizations

### Phase 5: Extensions
- [ ] RDF model implementation
- [ ] Plugin architecture
- [ ] NetworkX bridge
- [ ] OR-Tools bridge
- [ ] Async Python API

### Phase 6: Production Ready
- [ ] Comprehensive testing
- [ ] Benchmarking suite
- [ ] Documentation
- [ ] CI/CD pipeline

## References

- [Kuzu](https://github.com/kuzudb/kuzu) - Embeddable property graph DBMS (C++)
- [DuckDB](https://github.com/duckdb/duckdb) - Embeddable analytical DBMS
- [GQL Standard](https://www.iso.org/standard/76120.html) - ISO/IEC 39075
- [openCypher](https://opencypher.org/) - Cypher specification
- [WCOJ Paper](https://arxiv.org/abs/1310.3314) - Worst-case optimal join algorithms
- [Morsel-Driven](https://db.in.tum.de/~leis/papers/morsels.pdf) - Morsel-driven parallelism
- [Umbra](https://umbra-db.com/) - Vectorized execution reference
