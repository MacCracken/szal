# Szal — Roadmap

## v1.0 — Stable API (Done)

- Stable public API with `#[non_exhaustive]` enums and `#[must_use]` annotations
- Four execution modes: sequential, parallel, DAG, hierarchical
- Event bus with OTel adapter for observability
- Condition DSL for step-level predicate evaluation
- Majra integration: metrics, heartbeat, queue, pub/sub, rate limiting
- Hardware-aware scheduling via ai-hwaccel
- 50+ MCP tools via bote dispatcher
- Supply chain security: cargo-audit, cargo-deny, SLSA provenance

## v1.0.1 — Dependency Modernization (Done)

- bote 0.92 (streaming handlers, dynamic registration, schema validation)
- majra 1.0.4 (pubsub, queue, heartbeat, ratelimit)
- ai-hwaccel 1.1 (iterator-based device queries)
- sha2/md-5 0.11 (const generics, no generic-array)
- barrier, dag, fleet feature flags

## v1.1 — Planned

- Persistent workflow state (database-backed WorkflowStorage)
- Flow composition: sub-flow invocation from step handlers
- Streaming step output (progress reporting mid-execution)
- Condition DSL: comparison operators (`>`, `<`, `>=`, `<=`), `not` operator
- Step-level metrics (histogram per step type)
