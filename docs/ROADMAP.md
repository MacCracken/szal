# Szal — Roadmap

## v1.2 — Released (2026-06-10)

- [x] Step-level condition caching — `CompiledCondition` + `ConditionCache`; conditions parse once per engine, ~3× faster steady-state evaluation
- [x] Persistent execution store backends — durable `ExecutionStore` over sqlx (`sqlite`, `postgres` features) with an ordered write-through engine bridge
- [x] Flow versioning and migration — `FlowDef::version` + `migration::MigrationRegistry` to run old definitions against new schemas
- [x] Distributed DAG execution across multiple engine instances — `Engine::run_distributed` over `majra::fleet::FleetQueue` with work-stealing rebalance
- [x] Step output streaming over SSE / WebSocket transport — `stream::ProgressHub` broadcast hub + SSE frame encoding (transport-agnostic)

## v1.3 — Planned

- Execution-store-backed crash recovery — resume in-flight flows from persisted `ExecutionRecord`s
- Redis execution store backend (alongside SQLite/PostgreSQL)
- Cross-host fleet transport for `run_distributed` (back the fleet queue with a distributed transport)
- Per-step resource requirements wired into fleet scheduling (GPU/VRAM-aware routing)
- Condition DSL: `in`/`contains` operators and array/object path indexing

## Future

- Flow-level checkpointing and partial replay
- Pluggable scheduler strategies (priority, fairness, deadline)
- OpenTelemetry trace export for cross-service flow correlation
