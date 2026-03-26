# Szal — Roadmap to v1.0

## Completed

- ~~Hierarchical execution mode~~ — static sub-step trees with recursive executor
- ~~Integrate `EventBus` into `Engine::run()`~~ — `EventSink` type, events at all 10 lifecycle points
- ~~`error.rs` structured variants~~ — `StepTimeout`, `RetryExhausted`, `RollbackFailed` now constructed
- ~~Execution throughput benchmarks~~ — 7 criterion benchmarks across all modes
- ~~Tracing flow context~~ — `flow_id` and `flow_name` on all spans via `FlowCtx`/`ExecCtx`
- ~~MCP structured error codes~~ — `McpErrorCode` enum with transient/permanent distinction
- ~~Convert blocking `std::fs` to `tokio::fs`~~ — all 18 call sites async, `validate_path` async
- ~~Majra ManagedQueue~~ — queue-backed step execution via `queue_runner.rs`
- ~~Majra heartbeat~~ — engine health reporting with RAII `HeartbeatGuard`
- ~~Connection pooling and backpressure~~ — `NetworkPool` with per-host/domain/port rate limiting
- ~~Multi-tenant isolation~~ — `TenantRegistry` with per-tenant quota enforcement and tool access control
- ~~OpenTelemetry + Prometheus metrics~~ — `SzalMetrics` trait with workflow/step lifecycle hooks

## v1.0 — Required

### P0 — Migration Blockers

These must land before SecureYeoman can migrate its workflow engine (1,755 LOC TypeScript) to szal.

- [ ] **Step type + config fields on StepDef** — Add `step_type: Option<String>` and `config: Option<serde_json::Value>` to `StepDef`. Without these, handler functions cannot dispatch to different step implementations. Consumers build a handler that matches on `step_type` to route to webhook, bash, HTTP, ML pipeline, etc. This is the single biggest blocker.
- [ ] **Condition evaluation** — Add `condition: Option<String>` field to `StepDef`. When set, the engine evaluates the expression before executing the step; if false, the step is skipped with status `Skipped`. Expression format: `steps.build.status == 'completed' && input.env == 'prod'`. Implementation options: embedded Rhai scripting, CEL evaluator, or simple predicate DSL. Must be sandboxed (no filesystem/network access from expressions).
- [ ] **'any' trigger mode for DAG dependencies** — Currently all dependencies must complete before a step fires. Add `trigger_mode: TriggerMode` to StepDef with `All` (default) and `Any` variants. In `Any` mode, a step becomes ready when any single dependency completes. ~10 LOC change in `dag.rs` ready-queue logic.

### P1 — Migration Quality

- [ ] **Template resolution with path walking** — Extend template substitution to support dot-notation path traversal: `{{steps.build.output.url}}` walks into nested JSON. Current `TemplateRender` MCP tool only does flat `{{key}}` replacement. Should be available as a core utility function, not just an MCP tool.
- [ ] **Dynamic subworkflow lookup** — Enable a step handler to fetch and execute a different workflow definition at runtime (by ID or name). Requires the handler to have access to a `WorkflowStorage` trait (or similar) for runtime workflow resolution. Currently only static hierarchical sub-steps are supported.
- [ ] **Update majra dependency to 1.0.0** — Currently pinned to `0.22.3`. Update when majra 1.0.0 is published on crates.io.

### P2 — Production Hardening

- [ ] **OTel adapter for EventSink** — Map `WorkflowEvent` lifecycle events to OpenTelemetry spans. The `EventSink` infrastructure exists; this adds a concrete adapter that creates spans with `workflow.step_type`, `workflow.run_id`, `workflow.status` attributes.
- [ ] **Exponential backoff option** — Add `backoff_strategy` field to StepDef: `Fixed` (current behavior), `Linear` (delay * attempt), `Exponential` (delay * 2^attempt). Currently only fixed delay between retries.
- [ ] **`#[must_use]` audit** — Add `#[must_use]` to all pure functions per CLAUDE.md conventions.

### v1.0 — Goals

- Stable public API
- All transports production-hardened
- SecureYeoman and AGNOS backed by szal for MCP
- Majra powering all inter-service comms, queueing, health
- Published to crates.io as canonical MCP workflow engine
- Full docs, migration guide, tool catalog
