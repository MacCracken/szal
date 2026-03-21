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
- [ ] **Fuzz targets** — fuzz step deserialization, flow validation (DAG cycle inputs, malformed JSON), state transition sequences
- [ ] **Fuzz CI job** — 30s per target in CI (match dhvani pattern)
- [ ] **Property-based tests (proptest)** — state machine invariants (no valid transition from terminal states), DAG acyclicity after random step insertion, serde roundtrip for arbitrary configs
- [ ] **MCP protocol layer** — streamable HTTP, SSE, stdio transports
- [ ] **Tool/Resource/Prompt traits** and dynamic registry
- [ ] **Majra integration** — pub/sub for event dispatch, ManagedQueue for job scheduling, heartbeat for health
- [ ] **ai-hwaccel integration** — hardware-aware step scheduling via optional `hardware` feature
- [ ] **Feature gates** — modular features as crate grows (e.g. `mcp`, `hardware`, `majra`)
- [ ] **Doc tests** — runnable examples in rustdoc for all public API

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
