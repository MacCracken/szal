# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.23.3] — 2026-03-23

### Changed
- Bump bote dependency to 0.22.3 (crates.io, was local path)
- Bump majra dependency to 0.22.3
- Version alignment with hoosh ecosystem (0.23.3)

## [0.21.3] — 2026-03-21

### Added

- `step` module — atomic workflow steps with builder pattern, timeout, retry, rollback, DAG dependencies
- `flow` module — flow definitions with sequential, parallel, DAG, and hierarchical execution modes
- `engine` module — execution configuration and flow result aggregation
- `state` module — workflow state machine with validated transitions (8 states)
- `error` module — typed errors (step failure, timeout, retry exhaustion, cycle detection, rollback failure)
- DAG cycle detection via DFS
- Dependency validation for DAG flows
- Serde serialization for all core types
- Criterion benchmarks for DAG validation
- CI workflow (fmt, clippy, test, audit, deny, MSRV, coverage)
- Release workflow (multi-platform build, crates.io publish, GitHub release)
