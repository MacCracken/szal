# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.1.0] — 2026-07-29

Maintenance release: toolchain and vendored-dependency refresh for the Cyrius port. No functional
or behavioural changes to the workflow engine — the full suite (1,434 assertions across 45 test
files) is green, `rust-old/` parity oracle untouched.

### Changed
- **Cyrius toolchain 6.2.2 → 6.5.2** (`cyrius.cyml [package].cyrius`). Verified against the released
  `6.5.2-x86_64-linux` asset — the same artifact CI's installer fetches — so CI's "Verify toolchain
  matches pin" step passes. Stdlib re-provisioned from the 6.5.2 snapshot
- **Vendored majra 2.4.6 → 2.5.3** (`src/vendor/majra.cyr`, 3,131 → 3,289 lines). Collision scan
  re-run: no new clashes; the rename set shrank 9 → 7 symbols (see below). `MJ_ERR_`/`MJ_STEP_`/
  `MJ_TRIGGER_` + `majra_uuid_generate`/`majra_step_result_new` renames retained
- **Vendored bote-core 2.7.5 → 3.1.4** (`src/vendor/bote-core.cyr`, 2,025 → 2,612 lines). Despite the
  major version bump, **no szal changes were needed**: `compiled_compile` → `bote_compiled_compile`
  is still the only collision, and szal referenced none of the 13 bare `ERR_*` that bote 3.x prefixed
  to `BOTE_ERR_*`
- **Vendored ai-hwaccel 2.3.9 → 2.3.15** (`src/vendor/ai-hwaccel.cyr`, 6,210 → 6,348 lines). Its
  error codes are now `HWA_ERR_*`-prefixed upstream
- **szal's own error constants are now `SZAL_ERR_*`-prefixed** (11 constants: `ERR_NONE`,
  `ERR_STEP_FAILED`, `ERR_STEP_TIMEOUT`, `ERR_FLOW_INVALID`, `ERR_RETRY_EXHAUSTED`,
  `ERR_ROLLBACK_FAILED`, `ERR_CYCLE`, `ERR_MIGRATION`, `ERR_HW_UNAVAILABLE`, `ERR_QUEUE`,
  `ERR_OTHER`). Matches the convention sigil 6.5.2 / bote 3.1.4 / ai-hwaccel 2.3.15 all adopted
  upstream in the same window. Cyrius resolves fns, enum constants and globals in one flat namespace
  with last-definition-wins, so unprefixed error codes are a standing collision hazard
- `src/mcp_tools_conversion.cyr`'s `SECS_PER_*` / `BYTES_PER_*` constants are likewise `SZAL_`-prefixed
- `scripts/version-bump.sh` no longer rewrites `Cargo.toml` or regenerates `Cargo.lock` — dead steps
  inherited from the pre-port Rust project, which has no root Cargo manifest (the Rust oracle at
  `rust-old/` is never touched). It now writes only `VERSION`, validates the semver triple, and
  cross-checks that `cyrius.cyml` still resolves to it
- `src/engine_distributed.cyr` reformatted for the 6.5.2 formatter (continuation-line indent only)

### Fixed
- **`BYTES_PER_GB` value divergence** — szal defined it as `1073741824` (2^30) while vendored
  ai-hwaccel defines `var BYTES_PER_GB = 1000000000` (10^9). Under last-definition-wins these
  resolved correctly only because of `main.cyr`'s include order. Now `SZAL_BYTES_PER_GB`, so the
  byte-formatting tools can't be silently repointed at the decimal value by an include reshuffle
- **`json_v_parse_str` → `json_v_parse_buf`** in `src/mcp_tools_system.cyr` (`_sys_uptime_json`).
  bayan 1.3.0 renamed its cstr+len JSON entry point because the `_str` suffix is reserved for the
  Str-taking overload that Cyrius auto-dispatches to — while the cstr+len form held that name, every
  `bayan_json_v_parse(someStr)` in the ecosystem was silently rewritten into a 1-arg call to the
  2-arg function and returned 0 for valid JSON
- **Enum-constant array sizes replaced with literals** in `src/md5.cyr` and `src/error.cyr`. cycc
  resolves a `var buf[ENUM_CONST]` size through `FINDVAR`, which only honours var-table indices
  < 1024, so whether it compiles depends on how many globals the preceding includes declared. The
  larger 6.5.2 stdlib (sigil 19k → 26k lines, bayan 3.5k → 5.3k) pushed `md5.cyr:36` past the cap and
  broke three test builds. This is also the real mechanism behind the long-standing "full-deps
  `cyrius build` breaks `var buf[ENUM_CONST]`" gotcha
- **Zero duplicate-symbol warnings** from `cyrius build --strict --no-deps src/main.cyr`, down from
  four. The only remaining cross-library symbol anywhere is `REQ_NONE` (szal × ai-hwaccel), which is
  the intentionally shared hardware-requirement API
- Stale files left in `lib/` shadow the version-pinned stdlib snapshot, so a toolchain bump needs
  `rm -rf lib && cyrius lib sync` rather than a bare re-sync (documented in `state.md`)

## [1.2.0] — 2026-06-10

### Added
- **Step-level condition caching** — `CompiledCondition` (parse a condition once, evaluate against many contexts) and `ConditionCache` (thread-safe, memoizes compiled ASTs and parse errors by source string). The `Engine` now holds a `ConditionCache`, so a flow's conditions parse once even across repeated runs. ~3× faster steady-state evaluation (see `benches/condition.rs`: uncached ~783ns → cached ~257ns → pre-compiled ~215ns)
- **Flow versioning and migration** (`migration` module) — `FlowDef::version` (defaults to `1`; flows serialized before versioning deserialize to `1`) with `FlowDef::with_version()`. `FlowMigration` trait + `fn_migration()` closure constructor + `MigrationRegistry` to chain per-version upgrades. `migrate_to(target)` / `migrate_latest()` apply registered migrations in order; rejects downgrades and missing/overshooting paths
- **Step output streaming over SSE / WebSocket** (`stream` module) — `ProgressHub` broadcast hub (`tokio::sync::broadcast`) fans `StepProgress` out to many subscribers; its `sink()` plugs into `EngineConfig::progress_sink`. `progress_to_sse()` / `sse_frame()` encode events as `text/event-stream` frames. Transport-agnostic: no web-server dependency pulled into the library
- **Persistent execution store backends** (`sql_store` module, feature-gated) — durable, queryable `ExecutionStore` backed by sqlx. `sqlite` feature → `SqliteExecutionStore`, `postgres` feature → `PostgresExecutionStore`. Async API (`connect`/`migrate`/`save`/`get`/`list`/`remove`) plus `engine_sink()` which bridges to the synchronous `ExecutionStore` the engine consumes via an **ordered** background writer (a flow's `Running` save cannot overwrite its later `Completed` save) with an in-memory read mirror
- **Distributed DAG execution across engine instances** — `Engine::run_distributed(flow, fleet)` (`fleet` feature) distributes a DAG's ready steps across the nodes of a `majra::fleet::FleetQueue`, unlocking dependents as results arrive and rebalancing toward idle nodes. Honors the same event sink, metrics, condition cache, and execution store as `run()`
- `SzalError::MigrationFailed` variant
- `benches/condition.rs` — uncached vs. cached vs. pre-compiled condition evaluation
- Cargo features: `sqlite`, `postgres` (durable execution stores)

### Changed
- Bump `ai-hwaccel` dependency from `1.1` to `1.2`
- Refresh all transitive dependencies to latest semver-compatible versions (`tokio` 1.50→1.52, `serde` 1.0.228, `uuid` 1.23.3, et al.)
- `git_tools` blame author sort now uses `sort_by_key(Reverse(..))` (clippy lint under rustc 1.96)
- `deny.toml` now allows the `Zlib` license (introduced transitively by sqlx via `foldhash`)

### Fixed
- Ordered durable writes in the sqlx `engine_sink` bridge — previously two fire-and-forget saves (start `Running`, end `Completed`) could land out of order and leave a stale `Running` row

## [1.1.0] — 2026-04-03

### Changed
- Bump bote dependency from 0.50.0 to 0.92.0
- Bump majra dependency from 1.0.1 to 1.0.4
- Bump ai-hwaccel dependency from 0.23 to 1.1 (iterator-based device queries)
- Bump sha2 dependency from 0.10 to 0.11 (const generics, drops generic-array)
- Bump md-5 dependency from 0.10 to 0.11
- Tokenizer in `condition.rs` rewritten from `Vec<char>` to byte-level iteration — fixes potential panic on multi-byte UTF-8 in string literals
- `render_template` rewritten from `Vec<char>` to byte-level scanning with UTF-8 fallback — eliminates allocation
- `dag.rs` and `parallel.rs` use `Arc<str>` for flow name sharing instead of per-step `String::clone`
- `deny.toml` now allows GPL-3.0-only (AGNOS ecosystem license migration)
- README updated: version `1`, roadmap reflects v1.0 release

### Fixed
- Short-circuit evaluation in condition `&&`/`||` operators — `false && expr` now returns `false` without evaluating right side; `true || expr` returns `true` without evaluating right side

### Added
- Condition DSL: comparison operators `>`, `>=`, `<`, `<=` for numeric and string ordering
- Condition DSL: `!` (not) prefix operator for boolean negation
- Tests for short-circuit evaluation (`and_short_circuits_on_false`, `or_short_circuits_on_true`)
- Tests for Unicode in condition expressions and templates (`string_literal_with_unicode`, `render_template_with_unicode`, `render_template_with_unicode_literal_text`)
- Tests for comparison operators (13 tests) and not operator (5 tests)
- `StepTypeMetricsFn` callback type and `Engine::with_step_type_metrics()` builder for per-step-type duration histograms — works without `majra` feature, receives `(step_type, status, duration_ms)` after each step
- `emit_step_type_metric` wired into all 4 execution modes (sequential, parallel, DAG, hierarchical) and queue runner
- Tests for step-type metrics callback (success and failure paths)
- `StepProgress` event struct, `ProgressSink` callback type, and `ProgressReporter` handle for streaming step output mid-execution
- `Engine::with_progress_sink()` builder for attaching progress listeners
- `handler_fn_with_progress()` convenience constructor — creates a `StepHandler` that injects a `ProgressReporter` so handlers can call `reporter.report(data)` to emit progress events
- Progress reporting test (`progress_reporting`)
- `sub_flow_handler()` — creates a `StepHandler` that intercepts `step_type = "sub_flow"` steps and executes sub-flows from `WorkflowStorage`. Config must specify `flow_name`. Non-sub_flow steps delegate to the inner handler
- Tests for sub-flow execution (success, missing flow_name, flow not found)
- `ExecutionStore` trait and `InMemoryExecutionStore` for persisting workflow execution state (execution ID, flow name, state, result, timestamps)
- `ExecutionRecord` struct for execution state snapshots
- `Engine::with_execution_store()` builder — engine saves `Running` state at start and `Completed`/`Failed`/`RolledBack` at end
- Tests for execution store (save/get, list with filtering, remove, engine integration for success and failure)

## [1.0.1] — 2026-03-27

### Changed
- Bump bote dependency from 0.22.3 to 0.50.0
- Bump majra dependency from 1.0.0 to 1.0.1
- `tool_def()` helper now uses `ToolDef::new()` / `ToolSchema::new()` constructors (bote 0.50.0 made both `#[non_exhaustive]`)
- Re-export `AuditSink` and `EventSink` from bote in `mcp` module
- Replace duplicate `SzalMetrics` trait with re-export of `majra::metrics::MajraMetrics` (identical signatures, eliminates redundancy)
- `MetricsSink` type alias now uses `Arc<dyn MajraMetrics>` instead of `Arc<dyn SzalMetrics>`
- `Engine::with_metrics()` now accepts `Arc<dyn MajraMetrics>` — consumers get full infrastructure metrics (queue, pubsub, heartbeat, rate limiter) alongside workflow metrics
- Fix `prometheus` feature to actually enable `majra/prometheus` — `PrometheusMetrics` is now available when the feature is active

### Added
- `register_tools_with(audit, events)` — configure bote dispatcher with optional audit logging and event publishing sinks
- Consumers can now leverage bote 0.50.0 features: streaming handlers with progress/cancellation, dynamic tool registration/deregistration, tool versioning, tool deprecation, and compiled schema validation with type checking and default injection
- `barrier` feature — exposes majra's N-way barrier synchronisation with deadlock recovery
- `dag` feature — exposes majra's DAG dependency scheduler for queue-based execution
- `fleet` feature — exposes majra's distributed job queue with work-stealing

## [1.0.0] — 2026-03-26

Stable API release. All public enums are `#[non_exhaustive]`, all pure functions are `#[must_use]`.

### Added

#### Engine
- **Hierarchical execution mode** — static sub-step trees via `StepDef::with_sub_step()`. Recursive executor in `engine/hierarchical.rs` with fail-fast and sub-step skipping
- **EventBus integration** — `EventSink` type (`Option<Arc<dyn Fn(WorkflowEvent)>>`) with `emit()` helper. Events at all 10 lifecycle points (FlowStarted/Completed/Failed/RolledBack, StepStarted/Completed/Failed/Retry/Timeout/Skipped). `Engine::with_event_sink()` and `Engine::with_event_bus()` builders
- **Structured error construction** — `SzalError::StepTimeout`, `RetryExhausted`, `RollbackFailed` now constructed at their respective sites (previously unused)
- **Execution throughput benchmarks** — 7 criterion benchmarks in `benches/engine.rs` (sequential 10/100, parallel 10/100, DAG diamond/linear-100, hierarchical 10x10)
- **Tracing flow context** — `flow_id` and `flow_name` on all tracing spans via `FlowCtx`/`ExecCtx`. Spawned tasks in parallel/DAG carry flow context
- **Step type + config** — `StepDef::step_type: Option<String>` and `config: Option<serde_json::Value>` for handler dispatch (webhook, bash, HTTP, etc.)
- **Condition evaluation** — `StepDef::condition: Option<String>` with lightweight predicate DSL. `condition::evaluate()` recursive descent parser supporting dot-path access, `==`/`!=`, `&&`/`||`, parens, string/number/bool literals. Integrated into all 4 executors
- **'Any' trigger mode** — `TriggerMode::Any` for DAG dependencies. Step becomes ready when first dependency completes (vs all). Anti-duplicate queueing via sentinel in `unlock_dependents`
- **Backoff strategies** — `BackoffStrategy` enum (Fixed/Linear/Exponential) with `delay_ms()` calculation. `StepDef::with_backoff()` builder
- **Template path walking** — `condition::render_template()` resolves `{{steps.build.output.url}}` dot-notation paths in templates. `condition::resolve_path()` public utility
- **Dynamic subworkflow storage** — `WorkflowStorage` trait with `get_by_name()`/`get_by_id()`/`list()`. `InMemoryStorage` reference impl. `EngineConfig::storage` field and `Engine::with_storage()` builder
- **OTel adapter** — `bus::otel_event_sink()` maps `WorkflowEvent` to tracing spans with `workflow.*` attributes for OpenTelemetry export

#### Majra Integration (feature: `majra`)
- **Prometheus metrics** — `SzalMetrics` trait with workflow_run_started/completed/failed and workflow_step_started/finished. `MetricsSink` type threaded through `ExecCtx`. `Engine::with_metrics()` builder
- **Heartbeat health reporting** — `Engine::with_heartbeat()` with `ConcurrentHeartbeatTracker`. RAII `HeartbeatGuard` auto-registers/deregisters, heartbeats every 10s
- **ManagedQueue execution** — `Engine::with_queue()` for distributed step execution. `engine/queue_runner.rs` enqueues steps, worker loop dequeues + executes + marks complete/fail
- **Connection pooling** — `mcp::pool::NetworkPool` with per-host/domain/port `RateLimiter` instances. `LazyLock` static. Rate-limit checks in HttpRequest, DnsLookup, PortCheck tools
- **Multi-tenant isolation** — `mcp::tenant::TenantCtx` with per-tenant quota enforcement via `check_tenant_quota()` and tool access control via `check_tenant_tool_access()`
- `SzalError::QueueError` variant for queue operation failures

#### MCP
- **Structured error codes** — `McpErrorCode` enum (Validation, NotFound, PermissionDenied, Timeout, IoError, Internal) with `is_retryable()`. `result_error_typed()` adds `_meta.error_code` and `_meta.retryable` to responses. All 110 `result_error()` calls replaced
- **Async I/O** — all 18 blocking `std::fs` calls converted to `tokio::fs`. `validate_path()` is now async

### Changed
- All public enums now have `#[non_exhaustive]` (StepStatus, FlowMode, WorkflowState, EventType added)
- 60 `#[must_use]` annotations added to all pure public functions
- `EngineConfig` now has manual `Debug` impl (supports non-Debug majra types)
- Majra dependency updated from 0.22.3 to 1.0.0
- Criterion dev-dependency updated from 0.5 to 0.8

## [0.26.3] — 2026-03-26

### Added
- `scripts/bench-history.sh` — criterion benchmark runner with CSV history tracking (timestamp, version, commit, timing in nanoseconds); supports `--show` for recent history
- `benchmarks/` directory for persistent benchmark CSV data
- Makefile targets: `coverage`, `fuzz`, `semver`, `msrv`, `bench-history`
- Release profile: `opt-level = 3`, thin LTO, symbol stripping

### Changed
- CI clippy now runs with `--all-features` to match CLAUDE.md development process
- Makefile clippy target updated to `--all-features --all-targets`
- CI workflow scoped to least-privilege permissions (`contents: read`, `actions: read`)
- Release workflow: added `workflow_dispatch` for manual releases, SLSA provenance attestations (`id-token: write`, `attestations: write`), `cancel-in-progress: false` for release safety, `timeout-minutes: 30` on build jobs, scoped CI gate permissions
- README roadmap table updated to reflect current milestone

## [0.23.4] — 2026-03-23

### Added
- `unlock_dependents` helper extracts DAG scheduling logic from 3 duplicate blocks in engine
- Builder methods on `WorkflowEvent` (`with_flow`, `with_step`, `with_duration`, `with_attempt`, `with_error`)
- Named constants for magic numbers across MCP tools (file limits, timeouts, byte sizes, durations)
- Path validation on `git blame` file parameter (rejects option injection and path traversal)
- `DirList` recursive mode now logs unreadable subdirectories instead of silently swallowing errors

### Changed
- All MCP tools now use `result_ok_json` — eliminates `unwrap_or_default()` on serde serialization (35 call sites)
- `Exec` command filter rewritten: rejects path traversal and absolute paths instead of misleading shell-metacharacter check
- `WorkflowEvent` builders refactored from 7 manual field-setting methods to chained builder pattern
- `parse_state` / `all_workflow_states` deduplicated into single static table in `state_tools`
- `fuzz_flow_validate` only wires dependencies for DAG mode flows
- MD5 tool output now returns structured JSON matching SHA-256 format
- `ready.pop_front().unwrap()` in DAG loop replaced with `let Some(id) = ... else { break }`
- `EventBus::publish` propagates serialization errors via `tracing::warn` instead of `unwrap_or_default`

### Fixed
- `cargo fmt` violations across examples and MCP tools
- `cargo vet --locked` — added 46 new exemptions, upgraded 4 from `safe-to-run` to `safe-to-deploy`

## [0.23.3] — 2026-03-23

### Changed
- Bump bote dependency to 0.22.3 (crates.io, was local path)
- Bump majra dependency to 0.22.3
- Version alignment with hoosh ecosystem (0.23.3)

## [0.21.3] — 2026-03-21

### Added
- `step` module — atomic workflow steps with builder pattern, timeout, retry, rollback, DAG dependencies
- `flow` module — flow definitions with sequential, parallel, DAG, and hierarchical execution modes
- `engine` module — execution configuration and flow result aggregation
- `state` module — workflow state machine with validated transitions (8 states)
- `error` module — typed errors (step failure, timeout, retry exhaustion, cycle detection, rollback failure)
- DAG cycle detection via DFS
- Dependency validation for DAG flows
- Serde serialization for all core types
- Criterion benchmarks for DAG validation
- CI workflow (fmt, clippy, test, audit, deny, MSRV, coverage)
- Release workflow (multi-platform build, crates.io publish, GitHub release)
