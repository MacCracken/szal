# Szal — Claude Code Instructions

## Project Identity

**Szal** (Hungarian: szál — thread) is the AGNOS workflow engine. Step/flow execution with branching, retry, rollback, and parallel stages. Consumed by daimon, AgnosAI, and sutra.

- **Type**: Flat library crate (not a workspace)
- **License**: AGPL-3.0-only
- **MSRV**: 1.89
- **Version**: SemVer `0.D.M` pre-1.0 (date-based: `0.26.3` = March 26th)

## Architecture

```
szal/src/
├── lib.rs     — re-exports, doc examples
├── step.rs    — StepDef, step execution
├── flow.rs    — FlowDef, flow orchestration
├── engine.rs  — workflow engine, DAG execution
├── state.rs   — execution state, persistence
├── bus.rs     — event bus
├── error.rs   — SzalError
└── mcp/       — MCP tool definitions (bote integration)
```

## Consumers

| Consumer | What it uses |
|----------|-------------|
| daimon | Agent workflow orchestration |
| AgnosAI | Crew task pipelines |
| sutra | Playbook execution engine |

## Development Process

### P(-1): Scaffold Hardening (before any new features)

1. Test + benchmark sweep of existing code
2. Cleanliness check: `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo audit`, `cargo deny check`
3. Get baseline benchmarks (`./scripts/bench-history.sh`)
4. Initial refactor + audit (performance, memory, security, edge cases)
5. Cleanliness check — must be clean after audit
6. Additional tests/benchmarks from observations
7. Post-audit benchmarks — prove the wins
8. Repeat audit if heavy

### Development Loop (continuous)

1. Work phase — new features, roadmap items, bug fixes
2. Cleanliness check: `cargo fmt --check`, `cargo clippy --all-features --all-targets -- -D warnings`, `cargo audit`, `cargo deny check`
3. Test + benchmark additions for new code
4. Run benchmarks (`./scripts/bench-history.sh`)
5. Audit phase — review performance, memory, security, throughput, correctness
6. Cleanliness check — must be clean after audit
7. Deeper tests/benchmarks from audit observations
8. Run benchmarks again — prove the wins
9. If audit heavy → return to step 5
10. Documentation — update CHANGELOG, roadmap, docs
11. Return to step 1

### Key Principles

- **Never skip benchmarks.** Numbers don't lie. The CSV history is the proof.
- **Tests + benchmarks are the way.** Minimum 80%+ coverage target.
- **Own the stack.** If an AGNOS crate wraps an external lib, depend on the AGNOS crate.
- **No magic.** Every operation is measurable, auditable, traceable.
- **`#[non_exhaustive]`** on all public enums.
- **`#[must_use]`** on all pure functions.
- **`#[inline]`** on hot-path functions.
- **`write!` over `format!`** — avoid temporary allocations.
- **Cow over clone** — borrow when you can, allocate only when you must.
- **Vec arena over HashMap** — when indices are known, direct access beats hashing.
- **Feature-gate optional deps** — consumers pull only what they need.
- **tracing on all operations** — structured logging for audit trail.

## Conventions

- Error type: `SzalError` with `#[non_exhaustive]`
- Async runtime: tokio
- Serialization: serde + serde_json
- Benchmarks: criterion with CSV history via `scripts/bench-history.sh`
- CI: fmt + clippy + deny + audit + test + msrv + coverage

## DO NOT
- **Do not commit or push** — the user handles all git operations (commit, push, tag)

- **NEVER use `gh` CLI** — use `curl` to GitHub API only
- Do not add unnecessary dependencies — keep it lean
- Do not `unwrap()` or `panic!()` in library code
- Do not skip benchmarks before claiming performance improvements
- Do not commit `target/` or `Cargo.lock` (library crate)
