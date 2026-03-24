# Szal — Roadmap to v1.0

## Backlog

- Hierarchical execution mode — currently a silent no-op delegating to sequential
- Integrate `EventBus` into `Engine::run()` — bus exists but is disconnected from execution
- `error.rs` — `StepTimeout`, `RetryExhausted`, `RollbackFailed` variants are never constructed; engine uses plain strings instead
- Benchmarks only cover validation/serde — add execution throughput benchmarks (parallel, DAG scheduling overhead)
- Tracing calls missing flow context in concurrent scenarios — add flow_id to all spans
- MCP error responses lack structured error codes (transient vs permanent)
- Convert remaining blocking `std::fs` calls to `tokio::fs` (DirList read_dir, FileWrite append, validate_path canonicalize/exists, system_tools /etc/hostname + /proc/uptime, template_tools file read)
- ai-hwaccel integration — hardware-aware step scheduling via optional `hardware` feature
- Majra ManagedQueue — queue-backed step execution for distributed workloads
- Majra heartbeat — engine health reporting

## v0.23 — Agent Orchestration + LLM Gateway

- Port daimon agent orchestrator (sub-agent delegation, swarms, teams, DAG workflows)
- Port hoosh multi-provider LLM routing (15 providers, fallback chains, token budgets, streaming)
- Agent lifecycle as szal flows (spawn, monitor, terminate)
- Majra heartbeat for fleet-wide agent health + GPU telemetry
- Federation via Majra relay
- ~200 tools total

## v0.24 — Security, Governance + Integration Tools

- RBAC, sandboxing (Landlock/seccomp/WASM), audit trails
- OPA/CEL policy gates as workflow steps
- Port 38 platform integrations (Slack, Discord, GitHub, Gmail, Teams, WhatsApp)
- 5 code forge adapters, CI/CD tools, artifact registries
- Majra rate limiting on all external-facing tools
- ~350 tools total

## v0.25 — Knowledge, Training, Simulation

- Document ingestion (PDF, HTML, MD, URL), RAG with hybrid FTS+vector
- Full training pipeline tools: distillation, LoRA, DPO, LLM-as-Judge eval
- GPU-aware job scheduling via Majra ManagedQueue
- Simulation engine tools (tick-driven, emotion model, spatial)
- Workflow templates (port all 22 from SecureYeoman)
- ~490 tools — feature parity with SecureYeoman

## v0.26 — Consolidation + Hardening

- Deduplicate SecureYeoman (490) + AGNOS (144) tools into ~530 canonical set
- Multi-tenant isolation (tenant-scoped tool access, quota enforcement)
- Fuzz every tool input
- Supply chain security (SBOM, SLSA, signed releases)
- OpenTelemetry + Prometheus metrics via Majra MajraMetrics
- Connection pooling, backpressure, performance benchmarks

## v1.0 — Unified MCP Engine

- Stable public API
- All transports production-hardened
- SecureYeoman and AGNOS backed by szal for MCP
- Majra powering all inter-service comms, queueing, health
- Published to crates.io as canonical MCP workflow engine
- Full docs, migration guide, tool catalog
