# Szal — Roadmap to v1.0

## Backlog

- Hierarchical execution mode — currently a silent no-op delegating to sequential
- Integrate `EventBus` into `Engine::run()` — bus exists but is disconnected from execution
- `error.rs` — `StepTimeout`, `RetryExhausted`, `RollbackFailed` variants are never constructed; engine uses plain strings instead
- Benchmarks only cover validation/serde — add execution throughput benchmarks (parallel, DAG scheduling overhead)
- Tracing calls missing flow context in concurrent scenarios — add flow_id to all spans
- MCP error responses lack structured error codes (transient vs permanent)
- Convert remaining blocking `std::fs` calls to `tokio::fs` (DirList read_dir, FileWrite append, validate_path canonicalize/exists, system_tools /etc/hostname + /proc/uptime, template_tools file read)
- Majra ManagedQueue — queue-backed step execution for distributed workloads
- Majra heartbeat — engine health reporting
- Connection pooling and backpressure for network tools
- Multi-tenant isolation (tenant-scoped tool access, quota enforcement)
- OpenTelemetry + Prometheus metrics via Majra MajraMetrics

## v1.0 — Unified MCP Engine

- Stable public API
- All transports production-hardened
- SecureYeoman and AGNOS backed by szal for MCP
- Majra powering all inter-service comms, queueing, health
- Published to crates.io as canonical MCP workflow engine
- Full docs, migration guide, tool catalog
