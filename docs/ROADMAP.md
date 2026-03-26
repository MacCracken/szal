# Szal — Roadmap to v1.0

## P1 — Migration Quality

- [ ] **Template resolution with path walking** — Extend template substitution to support dot-notation path traversal: `{{steps.build.output.url}}` walks into nested JSON. Current `TemplateRender` MCP tool only does flat `{{key}}` replacement. Should be available as a core utility function, not just an MCP tool.
- [ ] **Dynamic subworkflow lookup** — Enable a step handler to fetch and execute a different workflow definition at runtime (by ID or name). Requires the handler to have access to a `WorkflowStorage` trait (or similar) for runtime workflow resolution. Currently only static hierarchical sub-steps are supported.
- [ ] **Update majra dependency to 1.0.0** — Currently pinned to `0.22.3`. Update when majra 1.0.0 is published on crates.io.

## P2 — Production Hardening

- [ ] **OTel adapter for EventSink** — Map `WorkflowEvent` lifecycle events to OpenTelemetry spans. The `EventSink` infrastructure exists; this adds a concrete adapter that creates spans with `workflow.step_type`, `workflow.run_id`, `workflow.status` attributes.
- [ ] **Exponential backoff option** — Add `backoff_strategy` field to StepDef: `Fixed` (current behavior), `Linear` (delay * attempt), `Exponential` (delay * 2^attempt). Currently only fixed delay between retries.
- [ ] **`#[must_use]` audit** — Add `#[must_use]` to all pure functions per CLAUDE.md conventions.

## v1.0 — Goals

- Stable public API
- All transports production-hardened
- SecureYeoman and AGNOS backed by szal for MCP
- Majra powering all inter-service comms, queueing, health
- Published to crates.io as canonical MCP workflow engine
- Full docs, migration guide, tool catalog
