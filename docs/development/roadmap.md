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
Toolchain pinned at `cyrius = "6.1.35"` (was 6.1.33 at M0). `VERSION` is the single source of truth.

## Pre-port decisions

| # | Decision | Status |
|---|----------|--------|
| License | GPL-3.0-only (matches all Cyrius siblings; AGPL in old Cargo.toml treated as drift) | ✅ decided |
| MD5 | hand-port RFC 1321 as `src/md5.cyr` (keeps the 54-tool MCP surface intact) | ✅ decided |
| Timestamps | epoch-ns `i64` internal, `iso8601()` second-precision at boundaries (subsecond RFC3339 cannot round-trip) | ✅ decided |
| UUID | `{hi, lo}` i64 pair internal, RFC-4122 string only at JSON/MCP boundaries | ✅ decided |
| SQL store | patra (stdlib) only; **postgres deferred**, prometheus passthrough dropped | ✅ decided |
| Version | 2.0.0; keep `rust-old/` as parity oracle, retire in a 2.0.x patch | ✅ decided |
| `registry_new` collision (bote-core × ai-hwaccel) | resolved bote-side: bote 2.7.5 renamed its tool-registry ctor `registry_new`→`tool_registry_new` (dissolves the clash — bote no longer owns the symbol). filed [`issues/2026-06-11-registry-new-collision.md`](issues/2026-06-11-registry-new-collision.md) | ✅ **RESOLVED** (2026-06-13) — re-synced bote, updated `mcp.cyr` caller, ported row 17 `engine_hardware`; collision scan clean |
| Engine concurrency model | threads + permit-channel (bounded `chan`) + cancel tokens; cooperative cancel replaces `JoinHandle::abort()` (observable timeout/cancel delta) | ✅ decided + implemented (parallel/dag/queue verified; see parity-notes §8) |
| Logging under threads | szal's own emit/metric/log calls stay on the MAIN thread (workers only run handlers), so sakshi is never touched cross-thread — no logging thread needed | ✅ decided + implemented |
| Pub/sub lag semantics | majra bounded-chan drop-newest vs tokio broadcast drop-oldest — contract change | ⏳ open (M3 stream/bus) |
| `validate_path` symlink semantics | component-walk + readlink (closest parity) vs lexical-only | ⏳ open (M3 MCP, security-relevant) |

## Milestones

### M0 — Port scaffold (done) ✅ 2026-06-11

- [x] `cyrius port` scaffold landed; 13,172 lines of Rust moved to `rust-old/`
- [x] Language/codebase review → [`port-plan.md`](port-plan.md) (7-brief synthesis)
- [x] Doc-tree per first-party-documentation standard

### M1 — Project wiring + foundation (compiling Cyrius core) — ✅ done 2026-06-11

Project setup:
- [x] `VERSION` → 2.0.0; `cyrius.cyml` `[package]`/`[build]`/`[deps]` (`[lib]`/`[lib.core]` dist lists deferred to M5 `cyrius distlib`)
- [x] GPL-3.0-only `LICENSE` text
- [x] `cyrius lib sync` provisions `lib/`; smoke `main.cyr` builds + runs

Foundation modules (no engine, no MCP — pure data + algorithms):
- [x] `src/uuid.cyr` — `uuid_generate()` (hi/lo), `uuid_to_str` RFC-4122, `uuid_parse`
- [x] `src/md5.cyr` — RFC 1321 `md5(data,len,out16)` + `md5_hex` (model: `lib/sha1.cyr`)
- [x] `src/error.cyr` — `SzalErr` code enum + `szal_err_name` + detail-msg buffer
- [x] `src/state.cyr` — `WorkflowState` FSM (8 states, exact transition table; Display + serde PascalCase forms)
- [x] `src/step.cyr` — `StepDef` heap struct + builders, backoff math, `StepStatus`, `StepResult`, `step_to/from_json`
- [x] `src/condition.cyr` — DSL tokenizer→parser→evaluator, compiled cache, `render_template`, `cond_build_step_context`
- [x] `src/flow.cyr` — `FlowDef` + builders, `flow_validate` (Dag DFS cycle check), `flow_to/from_json`
- [x] `src/bus.cyr` — `WorkflowEvent`/`EventType` (11), `event_topic`, `event_to_json`, `otel_event_sink` (majra-backed `EventBus` deferred to M3)
- [x] `src/migration.cyr` — `MigrationRegistry`, `migration_register`, `latest_version`, `migrate_to`/`migrate_latest` (pure, sync)
- [x] Foundation tests (`szal_core.tcyr` + per-module suites) — Rust unit + proptest assertions ported

**Exit:** ✅ `cyrius build --strict` green; foundation tests pass; `cyrius fmt --check` + `cyrius lint` + `cyrius doc --check` clean.

### M2 — Engine core + executors

- [x] `src/engine_result.cyr` — `FlowResult` (must precede storage; breaks the storage↔engine cycle) ✅
- [x] `src/storage.cyr` — `WorkflowStorage` + `ExecutionStore` fn-pointer vtables, in-memory impls ✅
- [x] `src/metrics.cyr` — majra 22-slot metrics vtable + thin wrappers ✅
- [x] **Full majra vendoring** ✅ `src/vendor/majra.cyr` (majra 2.4.6, 3,131 lines) via `scripts/sync-majra.sh` — 9-symbol collision rename applied (`MJ_ERR_`/`MJ_STEP_`/`MJ_TRIGGER_` + `majra_uuid_generate`/`majra_step_result_new`); only `lib/thread.cyr` needed (bigint was a false alarm). Build clean, 0 dup-symbol warnings. Unblocks `engine_queue_runner`/`engine_distributed` + M3 `stream`/`mcp_pool`. See [`majra-vendoring.md`](majra-vendoring.md).
- [x] `src/engine_core.cyr` — `FlowCtx`/`ExecCtx`, `EngineConfig`, `check_condition`, (fn-ptr, ctx) handler ABI (everything in `engine/mod.rs` except `sub_flow_handler`) ✅ verified 0-finding
- [x] `src/engine_step_exec.cyr` — retry/backoff/timeout via worker-thread + deadline (not `async_timeout` — it forks) ✅ cooperative-cancel delta in parity-notes §8
- [x] `src/engine_sequential.cyr` — in-order; exact skip-reason strings ✅
- [x] `src/engine_parallel.cyr` — thread fan-out + permit-channel semaphore + cancel tokens ✅ (alloc thread-safe; cooperative-cancel §8)
- [x] `src/engine_dag.cyr` — Kahn wavefront, `unlock_dependents`, transitive failure propagation ✅ (ordinal-indexed arenas; reuses the parallel worker/semaphore)
- [x] `src/engine_hierarchical.cyr` — recursive tree walk ✅
- [x] `src/engine_hardware.cyr` — `HardwareContext` over ai-hwaccel cached registry ✅ (2026-06-13) — wired into `engine_runner` at all three entry points (`_engine_check_hardware`, no-op when `config.hardware==0`). The `registry_new` collision (Q9) was **resolved** by the bote 2.7.5 re-sync (`registry_new`→`tool_registry_new`) — [`issues/2026-06-11-registry-new-collision.md`](issues/2026-06-11-registry-new-collision.md). 2 accepted divergences (parity-notes §11). **M2 fully closed.**
- [x] `src/engine_queue_runner.cyr` — majra `mq_*` (ResourcePool param dropped) ✅ first functional majra-queue use
- [x] `src/engine_distributed.cyr` — fleet workers + coordinator, reuses `unlock_dependents` ✅ (poll-loop translation of `select!{biased}`; result-chan cap = total+1 ⇒ no deadlock)
- [x] `src/engine_runner.cyr` — `Engine`, `run`/`run_with_cancellation`/`run_distributed`, rollback, heartbeat guard, persistence ✅ (queue-path + heartbeat-ticker + hw-check divergences documented in-module)
- [x] `src/engine_subflow.cyr` — `sub_flow_handler` (constructs a fresh child `Engine`) ✅
- [x] Engine test suites (per-module `tests/szal_engine_*.tcyr`) — ✅ all six modes + core/step_exec/runner/subflow covered, 0 failures (live suite totals in [`state.md`](state.md))

### M3 — Streaming, persistence, MCP

- [x] `src/stream.cyr` — `ProgressHub` (per-subscriber chans) + SSE frame encoding ✅ (lag-semantics divergence in parity-notes §9)
- [x] `src/sql_store.cyr` — patra-backed `ExecutionStore`, `szal_executions` table, synchronous writes ✅ (schema/upsert/sync divergences in parity-notes §10; Postgres dropped)
- [x] `src/vendor/majra.cyr` (M2) + `src/vendor/bote-core.cyr` ✅ (bote 2.7.5 dist, 1-symbol rename `compiled_compile`→`bote_compiled_compile`; `scripts/sync-bote.sh`; collision scan clean; 2.7.5 renamed `registry_new`→`tool_registry_new`, dissolving Q9)
- [x] `src/mcp.cyr` **(core)** — `result_ok/ok_json/error/error_typed`, `McpErrorCode`, **`validate_path`** (lexical component-walk + CWD confinement; security), `mcp_tool_def`/`mcp_tool_new`, `register_tools[_with]` over bote-core dispatcher ✅ (`tests/szal_mcp.tcyr`, 26 — incl. traversal-rejection). The 54-tool `all_tools()` aggregator lands with the tool files.
- [x] `src/mcp_pool.cyr` + `src/mcp_tenant.cyr` ✅ (2026-06-13) — `NetworkPool` (3 majra ratelimit buckets: HTTP/DNS/port) + `TenantRegistry`/quota/tool-access (mutex map, permissive-unknown). First functional use of majra `ratelimit_*`. `tests/szal_mcp_pool_tenant.tcyr` (32 — burst exhaustion, refill, serde). lint/fmt/doc clean.
- [ ] `src/mcp_tools_*.cyr` (15 files, 54 tools) — **4/15 done**: `mcp_tools_encoding.cyr` ✅ (szal_uuid + szal_base64, 19 tests; parity-notes §12), `mcp_tools_hash.cyr` ✅ (szal_sha256/md5/random_token, 19 tests; parity-notes §13), `mcp_tools_system.cyr` ✅ (szal_system_info/cwd/env_get/timestamp, 20 tests; parity-notes §14), `mcp_tools_json.cyr` ✅ (szal_json_path/diff/validate, 20 tests; parity-notes §15), `mcp_tools_template.cyr` ✅ (szal_template_render/wc/text_replace/split/join, 25 tests; parity-notes §16), `mcp_tools_conversion.cyr` ✅ (szal_base_convert/byte_format/duration_format, 20 tests; parity-notes §17), `mcp_tools_math.cyr` ✅ (szal_math_eval recursive-descent evaluator, 26 tests; parity-notes §18), `mcp_tools_state.cyr` ✅ (szal_state_check/transition/lifecycle, reuses state.cyr, 21 tests), `mcp_tools_step.cyr` ✅ (szal_step_create/validate/inspect, reuses step.cyr, 24 tests), `mcp_tools_flow.cyr` ✅ (szal_flow_create/validate/from_json/list_modes/add_step, reuses flow.cyr, 29 tests; parity-notes §19), `mcp_tools_engine.cyr` ✅ (szal_engine_create/result_inspect/step_status_list/error_list/server_info, 28 tests; parity-notes §20), `mcp_tools_file.cyr` ✅ (szal_file_read/write/dir_list/file_stat/path_exists, validate_path-gated + newfstatat, 28 tests; parity-notes §21). 3 files / ~13 tools left (process/git/net — security-sensitive; net file adds all_tools()/szal_register_tools()). Security checks (SSRF guard, git-ref validation, path confinement, size/count caps) must not regress; each tool is a `mcp_tool_new(mcp_tool_def(...), &handler)` pair, aggregated by `all_tools()` + `szal_register_tools()` in the last tool file. Handler pattern + traps captured in state.md.
- [x] MCP core test suite (`tests/szal_mcp.tcyr`) — result builders, error codes, validate_path traversal security, dispatcher registration ✅; per-tool security tests land with the tool files

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
