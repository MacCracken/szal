# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.23.4] — 2026-03-23

### Added
- `unlock_dependents` helper extracts DAG scheduling logic from 3 duplicate blocks in engine
- Builder methods on `WorkflowEvent` (`with_flow`, `with_step`, `with_duration`, `with_attempt`, `with_error`)
- Named constants for magic numbers across MCP tools (file limits, timeouts, byte sizes, durations)
- Path validation on `git blame` file parameter (rejects option injection and path traversal)
- `DirList` recursive mode now logs unreadable subdirectories instead of silently swallowing errors

### Changed
- All MCP tools now use `result_ok_json` — eliminates `unwrap_or_default()` on serde serialization (35 call sites)
- `Exec` command filter rewritten: rejects path traversal and absolute paths instead of misleading shell-metacharacter check
- `WorkflowEvent` builders refactored from 7 manual field-setting methods to chained builder pattern
- `parse_state` / `all_workflow_states` deduplicated into single static table in `state_tools`
- `fuzz_flow_validate` only wires dependencies for DAG mode flows
- MD5 tool output now returns structured JSON matching SHA-256 format
- `ready.pop_front().unwrap()` in DAG loop replaced with `let Some(id) = ... else { break }`
- `EventBus::publish` propagates serialization errors via `tracing::warn` instead of `unwrap_or_default`

### Fixed
- `cargo fmt` violations across examples and MCP tools
- `cargo vet --locked` — added 46 new exemptions, upgraded 4 from `safe-to-run` to `safe-to-deploy`

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
