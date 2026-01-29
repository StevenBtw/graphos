# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - Unreleased

### Added
- Comprehensive Python test suite
- LPG tests
- RDF graph tests
- GQL query language tests
- Test fixtures and base classes

### Changed
- Fully functioning database implementation
- Complete query execution pipeline

## [0.1.1] - Unreleased

### Changed
- **Breaking**: Renamed project from Graphos to Grafeo
- Feature complete implementation

### Added
- GQL (ISO standard) query language parser
- MVCC transaction management
- Multiple index types (hash, btree, trie, adjacency)
- Storage backends (memory, WAL)
- PyO3 Python bindings

## [0.1.0] - Unreleased

### Added
- Initial database architecture
- Crate structure:
  - `grafeo-common`: Foundation types, memory allocators, hashing utilities
  - `grafeo-core`: LPG storage, indexes, execution engine
  - `grafeo-adapters`: Parser, storage backends, plugins
  - `grafeo-engine`: Database facade, sessions, transaction management
  - `grafeo-python`: Python bindings
- Placeholder implementations for core components
