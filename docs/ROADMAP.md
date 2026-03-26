# Szal — Roadmap to v1.0

## Backlog

- Majra ManagedQueue — queue-backed step execution for distributed workloads
- Majra heartbeat — engine health reporting
- Connection pooling and backpressure for network tools
- Multi-tenant isolation (tenant-scoped tool access, quota enforcement)
- OpenTelemetry + Prometheus metrics via Majra MajraMetrics

## Completed

- ~~Hierarchical execution mode~~ — static sub-step trees with recursive executor
- ~~Integrate `EventBus` into `Engine::run()`~~ — `EventSink` type, events at all 10 lifecycle points
- ~~`error.rs` structured variants~~ — `StepTimeout`, `RetryExhausted`, `RollbackFailed` now constructed
- ~~Execution throughput benchmarks~~ — 7 criterion benchmarks across all modes
- ~~Tracing flow context~~ — `flow_id` and `flow_name` on all spans via `FlowCtx`/`ExecCtx`
- ~~MCP structured error codes~~ — `McpErrorCode` enum with transient/permanent distinction
- ~~Convert blocking `std::fs` to `tokio::fs`~~ — all 18 call sites async, `validate_path` async

## v1.0 — Unified MCP Engine

- Stable public API
- All transports production-hardened
- SecureYeoman and AGNOS backed by szal for MCP
- Majra powering all inter-service comms, queueing, health
- Published to crates.io as canonical MCP workflow engine
- Full docs, migration guide, tool catalog
