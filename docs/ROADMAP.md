# Szál — Roadmap to v1.0

## v0.22 — MCP Foundation + Project Completeness

### Shipped (this milestone)
- [x] deny.toml — license/advisory policy
- [x] codecov.yml — coverage tracking config
- [x] supply-chain/ — cargo-vet initialized
- [x] Examples: sequential, dag, state_machine, retry_rollback
- [x] Benchmarks: step (builder + serde), flow (serde + wide DAG), state (transitions)
- [x] Makefile: bench, vet, deny, clean targets
- [x] CI: cargo-vet, semver-checks, benchmark jobs
- [x] README.md with usage examples and roadmap table
- [x] CHANGELOG.md
- [x] version-bump.sh with validation
- [x] proptest dev-dependency added

### Deferred to v0.22 release
- [x] **Fuzz targets** — fuzz step deserialization, flow deserialization, flow validation (random DAG wiring), state transition sequences
- [x] **Fuzz CI job** — 30s per target in CI (match dhvani pattern)
- [x] **Property-based tests (proptest)** — terminal state invariants, DAG acyclicity (linear + fanout), serde roundtrip for arbitrary step configs, builder identity preservation
- [x] **MCP tool layer** — uses bote as MCP backend (protocol, dispatch, transport)
- [x] **Tool trait + 16 built-in tools** — step, flow, state, engine tools with bote ToolDef
- [x] **Majra integration** — event bus via pub/sub (WorkflowEvent types, topic hierarchy, subscribe with wildcards)
- [x] **Execution engine** — sequential, parallel, DAG execution with retry, timeout, rollback
- [x] **Feature gates** — `majra` feature for event bus
- [x] **Doc tests** — runnable examples in rustdoc for all public API
- [ ] **ai-hwaccel integration** — hardware-aware step scheduling via optional `hardware` feature
- [ ] **Majra ManagedQueue** — queue-backed step execution for distributed workloads
- [ ] **Majra heartbeat** — engine health reporting

### Engineering Backlog
- [ ] Extract duplicate "unlock dependents" DAG logic into helper (`engine.rs` — 3 identical blocks)
- [ ] Hierarchical execution mode — currently a silent no-op delegating to sequential
- [ ] Integrate `EventBus` into `Engine::run()` — bus exists but is disconnected from execution
- [ ] `DirList` recursive mode silently swallows subdirectory read errors
- [ ] `Exec` command metacharacter filter is misleading — `Command::new()` doesn't use a shell; validate `cwd` for path traversal instead
- [ ] `Sha256` output uses compact JSON (`json!().to_string()`) while all other tools use `to_string_pretty`
- [ ] `fuzz_flow_validate` — non-DAG flows with deps now always return Err after validation fix; update fuzzer to only wire deps for DAG mode
- [ ] `error.rs` — `StepTimeout`, `RetryExhausted`, `RollbackFailed` variants are never constructed; engine uses plain strings instead
- [ ] Benchmarks only cover validation/serde — add execution throughput benchmarks (parallel, DAG scheduling overhead)
- [ ] Convert remaining MCP tools to `result_ok_json` pattern (5 done, ~49 remaining)
- [ ] Event builder duplication in `bus.rs` — 6 near-identical static builder methods
- [ ] Magic numbers scattered across MCP tools — extract constants for timeouts, sizes, limits
- [ ] MCP tool naming inconsistency (`szal_wc` vs `szal_file_read`) — establish naming convention
- [ ] `parse_state` / `all_workflow_states` duplication in `state_tools.rs`
- [ ] Tracing calls missing flow context in concurrent scenarios — add flow_id to all spans
- [ ] MCP error responses lack structured error codes (transient vs permanent)
- [ ] Remaining `unwrap_or_default()` on serde serialization in MCP tools — propagate errors
- [ ] `ready.pop_front().unwrap()` in DAG loop — safe but is unwrap in production code
- [ ] Convert remaining blocking `std::fs` calls to `tokio::fs` (DirList read_dir, FileWrite append, validate_path canonicalize/exists, system_tools /etc/hostname + /proc/uptime, template_tools file read)
- [ ] `git blame` file parameter has no path validation (low risk — git rejects non-tracked files)

## v0.23 — Agent Orchestration + LLM Gateway

- [ ] Port daimon agent orchestrator (sub-agent delegation, swarms, teams, DAG workflows)
- [ ] Port hoosh multi-provider LLM routing (15 providers, fallback chains, token budgets, streaming)
- [ ] Agent lifecycle as szal flows (spawn, monitor, terminate)
- [ ] Majra heartbeat for fleet-wide agent health + GPU telemetry
- [ ] Federation via Majra relay
- [ ] ~200 tools total

## v0.24 — Security, Governance + Integration Tools

- [ ] RBAC, sandboxing (Landlock/seccomp/WASM), audit trails
- [ ] OPA/CEL policy gates as workflow steps
- [ ] Port 38 platform integrations (Slack, Discord, GitHub, Gmail, Teams, WhatsApp)
- [ ] 5 code forge adapters, CI/CD tools, artifact registries
- [ ] Majra rate limiting on all external-facing tools
- [ ] ~350 tools total

## v0.25 — Knowledge, Training, Simulation

- [ ] Document ingestion (PDF, HTML, MD, URL), RAG with hybrid FTS+vector
- [ ] Full training pipeline tools: distillation, LoRA, DPO, LLM-as-Judge eval
- [ ] GPU-aware job scheduling via Majra ManagedQueue
- [ ] Simulation engine tools (tick-driven, emotion model, spatial)
- [ ] Workflow templates (port all 22 from SecureYeoman)
- [ ] ~490 tools — feature parity with SecureYeoman

## v0.26 — Consolidation + Hardening

- [ ] Deduplicate SecureYeoman (490) + AGNOS (144) tools into ~530 canonical set
- [ ] Multi-tenant isolation (tenant-scoped tool access, quota enforcement)
- [ ] Fuzz every tool input
- [ ] Supply chain security (SBOM, SLSA, signed releases)
- [ ] OpenTelemetry + Prometheus metrics via Majra MajraMetrics
- [ ] Connection pooling, backpressure, performance benchmarks

## v1.0 — Unified MCP Engine

- [ ] Stable public API
- [ ] All transports production-hardened
- [ ] SecureYeoman and AGNOS backed by szal for MCP
- [ ] Majra powering all inter-service comms, queueing, health
- [ ] Published to crates.io as canonical MCP workflow engine
- [ ] Full docs, migration guide, tool catalog
