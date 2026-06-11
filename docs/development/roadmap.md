# szal — Roadmap (Rust → Cyrius port)

> Sequencing for the Rust→Cyrius port. This file is **what ships, in what order**.
> Live status (counts, pins, what compiles today) lives in [`state.md`](state.md).
> The full engineering spec — per-module API inventory, byte layouts, dep mapping,
> language gotchas — is [`port-plan.md`](port-plan.md) (the authoritative brief; read it
> before porting any module).
>
> **Source of truth:** the Rust implementation is frozen at `rust-old/` (13,172 lines,
> git tag `1.2.0`). Every Cyrius module is ported to match it function-for-function;
> diverge only with an ADR.

## Target

**Rust 1.2.0 → Cyrius 2.0.0.** Plain SemVer from 2.0.0 onward (majra is the precedent).
Toolchain pinned at `cyrius = "6.1.33"`. `VERSION` is the single source of truth.

## Pre-port decisions

| # | Decision | Status |
|---|----------|--------|
| License | GPL-3.0-only (matches all Cyrius siblings; AGPL in old Cargo.toml treated as drift) | ✅ decided |
| MD5 | hand-port RFC 1321 as `src/md5.cyr` (keeps the 54-tool MCP surface intact) | ✅ decided |
| Timestamps | epoch-ns `i64` internal, `iso8601()` second-precision at boundaries (subsecond RFC3339 cannot round-trip) | ✅ decided |
| UUID | `{hi, lo}` i64 pair internal, RFC-4122 string only at JSON/MCP boundaries | ✅ decided |
| SQL store | patra (stdlib) only; **postgres deferred**, prometheus passthrough dropped | ✅ decided |
| Version | 2.0.0; keep `rust-old/` as parity oracle, retire in a 2.0.x patch | ✅ decided |
| `registry_new` collision (bote-core × ai-hwaccel) | resolve before engine porting — vendor+rename locally (interim) or upstream ai-hwaccel rename (clean) | ⏳ open (blocks M2 hardware) |
| Engine concurrency model | threads + permit-channel + cancel tokens; cooperative cancel replaces `JoinHandle::abort()` (observable timeout/cancel delta) | ⏳ open (sign-off before M2 parallel) |
| Logging under threads | sakshi is single-threaded — chan-fed logging thread recommended | ⏳ open (before M2 parallel) |
| Pub/sub lag semantics | majra bounded-chan drop-newest vs tokio broadcast drop-oldest — contract change | ⏳ open (M3 stream/bus) |
| `validate_path` symlink semantics | component-walk + readlink (closest parity) vs lexical-only | ⏳ open (M3 MCP, security-relevant) |

## Milestones

### M0 — Port scaffold (done) ✅ 2026-06-11

- [x] `cyrius port` scaffold landed; 13,172 lines of Rust moved to `rust-old/`
- [x] Language/codebase review → [`port-plan.md`](port-plan.md) (7-brief synthesis)
- [x] Doc-tree per first-party-documentation standard

### M1 — Project wiring + foundation (compiling Cyrius core) — *in progress*

Project setup:
- [ ] `VERSION` → 2.0.0; `cyrius.cyml` `[package]`/`[build]`/`[lib]`/`[lib.core]`/`[deps]`
- [ ] GPL-3.0-only `LICENSE` text
- [ ] `cyrius lib sync` + `cyrius deps` provision `lib/`; smoke `main.cyr` builds

Foundation modules (no engine, no MCP — pure data + algorithms):
- [ ] `src/uuid.cyr` — `uuid_generate()` (hi/lo), `uuid_to_cstr` RFC-4122, `uuid_parse`
- [ ] `src/md5.cyr` — RFC 1321 `md5(data,len,out16)` + `md5_hex` (model: `lib/sha1.cyr`)
- [ ] `src/error.cyr` — `SzalErr` code enum + `szal_err_name` + detail-msg buffer
- [ ] `src/state.cyr` — `WorkflowState` FSM (8 states, exact transition table, `state_name`)
- [ ] `src/step.cyr` — `StepDef` heap struct + builders, backoff math, `StepStatus`, `StepResult`, `step_to/from_json`
- [ ] `src/condition.cyr` — DSL tokenizer→parser→evaluator, compiled cache, `render_template`, `build_step_context` (1,270 Rust lines — largest pure unit)
- [ ] `src/flow.cyr` — `FlowDef` + builders, `flow_validate` (Dag DFS cycle check), `flow_to/from_json`
- [ ] `src/bus.cyr` — `WorkflowEvent`/`EventType` (11), `event_topic`, `event_to_json`, `otel_event_sink` (majra-backed `EventBus` deferred to M3)
- [ ] Foundation tests (`tests/szal_core.tcyr`, `tests/szal_condition.tcyr`) — port the Rust unit + proptest assertions

**Exit:** `cyrius build --no-deps` green; foundation tests pass; `cyrius fmt --check` + `cyrius lint` clean.

### M2 — Engine core + executors

- [ ] `src/engine_result.cyr` — `FlowResult` (must precede storage; breaks the storage↔engine cycle)
- [ ] `src/storage.cyr` — `WorkflowStorage` + `ExecutionStore` fn-pointer vtables, in-memory impls
- [ ] `src/metrics.cyr` — majra 22-slot metrics vtable + thin wrappers
- [ ] `src/engine_core.cyr` — `FlowCtx`/`ExecCtx`, `EngineConfig`, `check_condition`, (fn-ptr, ctx) handler ABI (everything in `engine/mod.rs` except `sub_flow_handler`)
- [ ] `src/engine_step_exec.cyr` — retry/backoff/timeout via worker-thread + deadline (not `async_timeout` — it forks)
- [ ] `src/engine_sequential.cyr` — in-order; exact skip-reason strings
- [ ] `src/engine_parallel.cyr` — thread fan-out + permit-channel semaphore + cancel tokens *(needs concurrency sign-off)*
- [ ] `src/engine_dag.cyr` — Kahn wavefront, `unlock_dependents`, transitive failure propagation
- [ ] `src/engine_hierarchical.cyr` — recursive tree walk
- [ ] `src/engine_hardware.cyr` — `HardwareContext` over ai-hwaccel cached registry *(needs `registry_new` fix)*
- [ ] `src/engine_queue_runner.cyr` — majra `mq_*` (ResourcePool param dropped)
- [ ] `src/engine_distributed.cyr` — fleet workers + coordinator, reuses `unlock_dependents`
- [ ] `src/engine_runner.cyr` — `Engine`, `run`/`run_with_cancellation`/`run_distributed`, rollback, heartbeat guard, persistence (746 Rust lines — the heart)
- [ ] `src/engine_subflow.cyr` — `sub_flow_handler` (deferred last; constructs `Engine`)
- [ ] Engine test suites (`tests/szal_engine.tcyr` + per-mode) — the ~47 behavioral tests define semantics

### M3 — Streaming, persistence, MCP

- [ ] `src/stream.cyr` — `ProgressHub`, SSE frame encoding
- [ ] `src/sql_store.cyr` — patra-backed `ExecutionStore`, `szal_executions` table, synchronous writes
- [ ] `src/vendor/bote-core.cyr` + `src/vendor/majra.cyr` — synced dist pins + `scripts/sync-*.sh`
- [ ] `src/mcp.cyr` — `Tool` pairs, `register_tools` over bote-core dispatcher, `result_*` helpers, `validate_path` *(security)*
- [ ] `src/mcp_pool.cyr` + `src/mcp_tenant.cyr` — majra ratelimit buckets, tenant registry
- [ ] `src/mcp_tools_*.cyr` (15 files, 54 tools) — security checks (SSRF guard, git-ref validation, path confinement, size/count caps) must not regress
- [ ] MCP test suite (`tests/szal_mcp.tcyr`) + security/traversal tests

### M4 — Verification

- [ ] Test parity: port 294 Rust assertions across split `.tcyr` suites (fn-table cap)
- [ ] `benches/bench_all.bcyr` — emit CSV-compatible rows; `scripts/bench-history.sh` rewrite
- [ ] `fuzz/*.fcyr` — 4 property harnesses (step/flow deser, flow validate, state transitions)
- [ ] `cyrius audit` (self-host + test + fmt + lint) green; `cyrius capacity --check`
- [ ] `docs/benchmarks-rust-v-cyrius.md` — head-to-head (prerequisite for retiring `rust-old/`)

### M5 — Documentation + distribution → 2.0.0 release

> Docs rewrite is being done alongside M1 (this session) — see the Documentation task.

- [ ] README, CLAUDE.md, CHANGELOG (2.0.0 crossover entry), CONTRIBUTING, SECURITY rewritten for Cyrius
- [ ] `docs/architecture/overview.md`, `docs/guides/getting-started.md`, 5 examples ported
- [ ] NEW: `docs/adr/0002-port-from-rust-to-cyrius.md`, `DEPS-PATTERN.md`, `docs/development/semver.md`, `docs/cyrius-feedback.md`
- [ ] `.github/workflows/{ci,release}.yml` (bote/majra 6.x model); `Makefile`/`scripts` → cyrius CLI
- [ ] `dist/szal.cyr` + `dist/szal-core.cyr` committed; `cyrius.lock` committed
- [ ] zugot `marketplace/szal.cyml` rewritten on the Cyrius shape
- [ ] Tag `2.0.0` (release workflow asserts tag == VERSION)

## Deferred (post-2.0.0)

- Retire `rust-old/` (2.0.x patch, after the benchmark-comparison doc lands)
- Postgres execution store (patra-only in 2.0.0)
- Crash recovery — resume in-flight flows from persisted `ExecutionRecord`s *(carried from Rust v1.3)*
- Redis execution store backend *(carried from v1.3)*
- Cross-host fleet transport for `run_distributed` *(carried from v1.3)*
- Per-step resource requirements wired into fleet scheduling — GPU/VRAM-aware routing *(carried from v1.3)*
- Condition DSL `in`/`contains` operators + array/object path indexing *(carried from v1.3)*
- Flow-level checkpointing and partial replay; pluggable scheduler strategies; OpenTelemetry trace export *(carried from Future)*

## Consumer contract

- **daimon** (Cyrius): no szal dep today — the port defines the contract. Must **not** export bare `mcp_*` symbols (daimon owns that prefix); prefix everything `szal_`/`flow_`/`step_`.
- **AgnosAI**: consumes the Rust crate until its own port — no Cyrius contract yet.
- **sutra**, **samay**: planned consumers of `dist/szal.cyr`.
- **secureyeoman** (stays Rust): pins `szal = "1.0"` — the Rust repo/tags must remain intact.
