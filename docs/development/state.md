# szal — Current State

> Refreshed every release. CLAUDE.md is preferences/process/procedures
> (durable); this file is **state** (volatile).

## Version

**2.0.0** (in development) — Rust→Cyrius port. Rust 1.2.0 (13172 lines) frozen at
`rust-old/` (git tag `1.2.0`) as the parity oracle. `VERSION` is the single source of truth;
`cyrius.cyml` reads `${file:VERSION}`.

## Toolchain

- **Cyrius pin**: `6.1.37` (in `cyrius.cyml [package].cyrius`). The full suite (964 assertions across
  24 test files) + main + the top-level `szal.tcyr` smoke are green under **6.1.37** (2026-06-11); the
  pin matches the installed wrapper — drift cleared. (Silence any future drift warning per-invocation
  with `CYRIUS_NO_WARN_PIN_DRIFT=1`.) History: 6.1.33 (M0) → 6.1.34 → 6.1.35 → 6.1.36 → 6.1.37.

## Milestone

**M1 — Project wiring + foundation. ✅ COMPLETE (2026-06-11).** Wiring done; all 9 foundation
modules ported, tested, wired into `main`; adversarial parity audit run + both findings fixed.

- ✅ Wiring: `VERSION`→2.0.0, full `cyrius.cyml` ([package]/[build]/[deps]+ai-hwaccel block),
  GPL-3.0-only `LICENSE`, `/lib/` gitignored, `cyrius lib sync` (88 stdlib modules),
  `src/main.cyr` builds `--strict` green and runs.
- ✅ Foundation modules ported (port-plan §4 rows 0–7), each cross-checked vs `rust-old` and
  self-tested `--strict` green: `error` (30), `state` (61), `uuid` (33), `md5` (13), `bus` (65),
  `step` (142), `condition` (197), `flow` (49), `migration` (34) — **624 assertions, 0 failures**.
  Combined `tests/szal_core.tcyr` (24) proves single-pass composition. ~3,300 lines of Cyrius.
  (`step` grew +11 in M2: a `StepResult` deserializer `step_result_from_json`/`_step_result_from_v`
  was added to fill the to/from-json asymmetry — `engine_result.from_json` is its first consumer.)
- ✅ M1 exit gates green (re-verified under 6.1.35): `cyrius build --strict` clean · all tests
  pass · `cyrius lint` clean · `cyrius fmt <f> --check` clean · `cyrius doc --check` clean (0 undocumented).
- ✅ Foundation wired into `src/main.cyr`: full single-pass include order (proven by
  `tests/szal_core.tcyr`) + a smoke `main()` that builds a 2-step DAG and `flow_validate`s it.
  `cyrius build --strict` green, `./build/szal` prints `szal ready`, fmt+lint clean.
  (`cyrius.cyml` `[lib]`/`[lib.core]` dist-bundle lists stay deferred to M5 `cyrius distlib`
  per the manifest comment.) bus's majra `EventBus` deferred to M3.
- ✅ Adversarial parity audit vs `rust-old` (7 module auditors + per-finding skeptic verify):
  5 modules clean (error/migration/flow/step/condition); **2 confirmed divergences, both fixed
  Cyrius-side** (oracle untouched):
  - **state (critical, json-shape):** Rust `WorkflowState` derives `Serialize` with no
    `rename_all`, so serde emits PascalCase (`"RollingBack"`) — distinct from the Display
    snake_case (`"rolling_back"`). The port had only the snake_case form and mislabeled it
    "serde". Fix: added `state_json_name`/`state_from_json` (PascalCase serde wire form) alongside
    `state_name`/`state_from_name` (Display); +8 assertions. Unblocks `storage.cyr` (M2).
  - **bus (major, json-shape):** `duration_ms` Some(0) rendered `null` (a 0-sentinel can't tell
    Some(0) from None; Rust struct has no `skip_serializing_if`, so serde emits `0`). Fix: added
    `WE_DURATION_SET` presence flag (mirrors the existing `WE_ATTEMPT_SET`); WE_SIZE 72→80; +3 assertions.

**M2 — Engine core + executors. ⏳ in progress (rows 8–21 done; only the GATED row 17 remains).**
Ported port-plan §4 rows 8–21 in topological order. **Q10 (concurrency) + Q11 (logging) are RESOLVED
in practice** — the parallel/dag/queue/distributed rows shipped on the cooperative-cancel +
thread-safe-`alloc` model (parity-notes §8); only **Q9 (`registry_new` collision)** still gates
`engine_hardware` (row 17). The `Engine` (row 20) drives all six modes end-to-end, and
`sub_flow_handler` (row 21) runs nested flows — **M2 is feature-complete except the gated row 17.**

- ✅ **row 8 `src/engine_result.cyr` (`FlowResult`)** — ported + wired into `main`, cross-checked
  vs `engine/result.rs`. `FlowResult {flow_name, steps vec, total_duration_ms, success,
  rolled_back}` + `completed/failed/skipped_count` + `to_json/from_json` (serde shape, no skips).
  `tests/szal_engine_result.tcyr` (27) ports `flow_result_counts` + `flow_result_serde_roundtrip`.
  Prereq landed in `step.cyr` (StepResult deserializer, see above). **Unblocks `storage.cyr`.**
- ✅ Adversarial parity-verify of the new surface (3 diverse-lens auditors + skeptic verify, oracle
  guarded read-only): serde wire-shape lens **clean**; 9 "confirmed" findings triaged to **0
  behavioral changes** — all are accepted, codebase-wide idioms (§1 u64→i64 width, §3 lenient
  serde-default deserializers). Recorded in the new **`docs/development/parity-notes.md`** (which
  also resolved 5 dangling `see parity_notes` references from M1 modules). Audit "fixes" that
  proposed editing `rust-old/` were rejected (parity = match Rust, never mutate the oracle).
- ✅ **row 9 `src/storage.cyr`** — ported + wired. `WorkflowStorage` (3-slot vtable
  {get_by_name,get_by_id,list}) + `InMemoryStorage` (insert/remove); `ExecutionRecord`
  {execution_id, flow_name, state, result `Option<FlowResult>`, started_at, finished_at} +
  serde to/from_json; `ExecutionStore` (4-slot vtable {save,get,list(filter),remove}) +
  in-memory impl. `dyn Trait` → fat-pointer vtables via `lib/trait.cyr` (`trait_obj_new`/
  `trait_call0/1`); maps are `map_new_str` (Str content keys). `tests/szal_storage.tcyr` (38)
  ports all 6 Rust storage tests + an `ExecutionRecord` round-trip. Locking deferred to the M2
  parallel rows (parity-notes §7); get/remove return the stored ptr not a deep clone (§6).
- ✅ Adversarial parity-verify of storage (3 lenses, auditors primed with the parity-notes
  accepted-idioms list, oracle read-only): **0 findings, 0 changes** — full behavioral + serde
  parity. Disposition logged in parity-notes.
- ✅ **row 10 `src/metrics.cyr`** — ported + wired. 5 fire-and-forget wrappers (`metric_run_started/
  completed/failed`, `metric_step_started/finished`) over majra's 22-slot metrics vtable; sink is
  the vtable ptr with `0 = None` (no-op guard). `MetricsSink = Option<Arc<dyn MajraMetrics>>` →
  vtable-ptr handle (AGNOS "hand the consumer the vtable" model). `tests/szal_metrics.tcyr` (13)
  mirrors the Rust none/noop tests + a dispatch proof (wrapper reaches vtable slot 136).
- ✅ **Full majra vendored** `src/vendor/majra.cyr` (majra **2.4.6**, 3,131 lines) via
  `scripts/sync-majra.sh`, with the 9-symbol szal-collision rename (`MJ_ERR_`/`MJ_STEP_`/
  `MJ_TRIGGER_` + `majra_uuid_generate`/`majra_step_result_new`). The earlier "blockers" dissolved:
  the core dist references **no** `bigint`/`tls`/`sandhi`/`patra`, so the only stdlib addition was
  `lib/thread.cyr`. Full main (all szal modules + full majra) builds `--strict` with **0 undefined
  fns, 0 duplicate-symbol warnings**. The interim metrics shim was **retired** (deleted; `metrics.cyr`
  repointed — drop-in). This unblocks the majra-heavy rows (`engine_queue_runner`,
  `engine_distributed`, M3 `stream`/`mcp_pool`). cyrius.cyml pin still reads 2.4.5 → reconcile at
  M5 (dist byte-identical). Spec/maintenance record: `docs/development/majra-vendoring.md`.
- ✅ **row 11 `src/engine_core.cyr`** — `engine/mod.rs` minus `sub_flow_handler`. The shared engine
  infra: `FlowCtx`/`ExecCtx`, `EngineConfig` (11 fields + default `max_concurrency`=16 + accessors/
  setters), `StepProgress`/`ProgressReporter`+report, `emit`/`emit_step_type_metric`/`check_condition`.
  Central ABI: every `Option<Arc<dyn Fn → BoxFuture>>` → a **(fn_ptr, ctx_ptr) callback pair** (0 =
  None; handlers synchronous — no async, port-plan §1.7). `tests/szal_engine_core.tcyr` (38) covers
  config/setters, handler_invoke, emit None-guard+dispatch, the `"default"` step-type fallback,
  check_condition (no-cond/met/not-met/parse-err), FlowCtx/ExecCtx, ProgressReporter dispatch.
  Adversarial parity-verify (3 lenses, oracle read-only): **0 findings** — field-for-field parity.
- ✅ **row 12 `src/engine_step_exec.cyr`** — `execute_step_with_handler` (retry/backoff/per-attempt
  timeout). The first module that actually **runs handlers on threads**. Timeout = worker thread +
  `chan_try_recv` deadline poll (port-plan §1.7; `async_timeout` forks → loses step side effects).
  **Q10 clarified, not a blocker:** Cyrius *does* have async (`lib/async.cyr`) + an exact-parity
  `CancellationToken` (`cancel_token_*`); the only residual is the cooperative-cancel timeout delta
  (an orphaned wedged handler runs on — inherent to OS threads; even `std::thread`/`tokio` share it),
  documented in parity-notes **§8**. Handler ABI: `Ok(json_v)` | `Err(message_Str)`. Exact error
  strings (`step timeout: …`, `retry exhausted: …`). `tests/szal_engine_step_exec.tcyr` (27) covers
  success/attempts/output, retry-then-succeed, RetryExhausted, last-error, and the real worker-thread
  timeout. Adversarial parity-verify (3 lenses): **0 findings**.
- ✅ **row 13 `src/engine_sequential.cyr`** — `run_sequential`: in-order loop; skip on cancel /
  `prior step failed` / `flow timeout exceeded` / `condition not met` (exact strings, exact check
  order), else `execute_step_with_handler`; a Failed result cascades; a condition PARSE error is
  logged-and-run (not skipped). `tests/szal_engine_sequential.tcyr` (17) covers all five paths +
  the cancel token + the Skipped result shape. **The engine now runs a sequential flow end-to-end.**
  Adversarial parity-verify (single-lens): **0 findings**.
- ✅ **row 16 `src/engine_hierarchical.cyr`** — `run_hierarchical`: recursive pre-order tree walk
  (plain recursion, no boxed futures); sequential siblings; a successful step recurses into its
  `sub_steps`; a failed step skips its whole subtree (`parent step failed`) and cascades
  (`prior step failed`) to later siblings; cancel/timeout/condition skip the step + its subtree.
  Mutually-recursive skip helpers (Cyrius allows forward/mutual fn refs — proven by the condition
  parser). `tests/szal_engine_hierarchical.tcyr` (21) covers all cascades + pre-order. Parity-verify
  **running**. **The engine now runs sequential + hierarchical flows end-to-end.**
- ✅ **row 14 `src/engine_parallel.cyr`** — `run_parallel`: real bounded thread fan-out. Each step on
  a `thread_create` worker; concurrency capped by a counting semaphore (bounded `chan` pre-filled
  with N tokens; acquire=`chan_recv`, release=`chan_send`); join in spawn order; condition pre-pass
  collects `pre_skipped` FIRST; final order = pre_skipped ++ spawned. `thread_join` returns no value
  so each worker publishes its StepResult into a per-worker slot (§8); cancel/timeout at join Skips +
  orphans (cooperative). `tests/szal_engine_parallel.tcyr` (20) covers concurrent completion (spawn
  order via id), pre-skip-first ordering, no-cascade failure, cancel, flow-timeout. Parity-verify
  **running**. **The engine now runs sequential + hierarchical + parallel flows.**
- ✅ **row 15 `src/engine_dag.cyr`** — `run_dag`: Kahn wavefront, each ready wave run as a parallel
  batch (reuses engine_parallel's `_par_worker`/semaphore/slot), `unlock_dependents` (decrement
  in-degree → ready at 0, `STEP_I64_MAX` sentinel prevents re-queue), transitive failure via a
  `failed` set, `dependency failed` skips. Ordinal-indexed i64 arenas + one id→ordinal map (CLAUDE.md
  vec-arena-over-hashmap). `tests/szal_engine_dag.tcyr` (19): linear, diamond (d runs once), transitive
  failure, condition skip, cancellation (locked steps get no result), TriggerMode::Any-runs-once.
  Parity-verify **running**. **ALL FIVE execution modes are now ported** (sequential/parallel/dag/
  hierarchical + step_exec + core).
- ✅ **row 18 `src/engine_queue_runner.cyr`** — `run_queued`: enqueue all steps at `PRIORITY_NORMAL`
  into a majra `ManagedQueue`, single worker loop dequeues → `execute_step_with_handler` →
  `mq_complete`/`mq_fail` → collect; exits when all processed or drained. **First functional use of
  the vendored majra** (`mq_*` + `queue_item_payload`). ResourcePool arg dropped (port-plan §3.2).
  `tests/szal_engine_queue_runner.tcyr` (14). Parity-verify **running**.
- ✅ **row 19 `src/engine_distributed.cyr`** — `run_distributed_dag`: multi-node coordinator over a
  majra `FleetQueue`. One worker thread per registered node (`fleet_node_queue` via `map_keys`)
  loops {`done` cancel-check → `mq_dequeue` → `execute_step_with_handler` → `mq_complete`/`mq_fail`
  → report StepResult over a result `chan`}; the coordinator submits ready steps via `fleet_submit`
  (0 → Failed `no fleet node available`), `chan_try_recv`-polls completions, `_dag_unlock`s
  dependents, and `fleet_rebalance`s per completion. **Reuses engine_dag's bookkeeping wholesale**
  (`_dag_lookup`/`_dag_unlock`/`_dag_skip` + the id→ordinal map / ordinal in_degree[]·failed[] /
  vec-of-vecs dependents). Rust's two `select!{biased}` → poll loops (parity-notes §8). Skip reasons
  exact: `dependency failed`, `condition not met`, `no fleet node available` (Failed), post-loop
  `cancelled`/`flow timeout exceeded`/`not scheduled`. Safety: result `chan` cap = total+1 ⇒
  `chan_send` never blocks ⇒ workers always reach `done` ⇒ `thread_join` can't deadlock.
  `tests/szal_engine_distributed.tcyr` (14): diamond/2-node, 13-step fan-out/3-node, dependency-
  failure (1 Failed + 2 Skipped), condition-false skip, no-nodes-all-Failed — **stable across 20
  concurrent runs**. lint/fmt/doc clean. Self-parity-checked branch-for-branch vs `distributed.rs`
  (oracle pristine); the `rejects_non_dag_mode` Rust test guards the Engine wrapper → deferred to
  row 20. **The engine now runs sequential/parallel/dag/hierarchical/queue/distributed.**
- ✅ **row 20 `src/engine_runner.cyr`** — the heart: `Engine {config, handler, rollback_handler/0,
  event_sink/0, condition_cache}` + 10 mutate-and-return builders (`engine_new` +
  `engine_with_rollback/event_sink/storage/metrics/heartbeat/queue/execution_store/progress_sink/
  step_type_metrics`). `engine_run` EXACT sequence: `flow_validate?` → flow_started → save Running
  `ExecutionRecord` → metric_run_started → heartbeat register → resolve timeout (global‖flow‖i64max)
  → **queue path OR mode dispatch** (`run_sequential`/`parallel`/`dag`/`hierarchical`) → rollback
  completed+rollbackable steps in REVERSE on failure (emit `step_rollback` each; no handler→false) →
  terminal `flow_rolled_back`/`flow_failed`/`flow_completed` + metric → build `FlowResult` → save
  final record → deregister heartbeat. Plus `engine_run_with_cancellation` (was_cancelled = signalled
  && any Skipped-"cancelled"; **never persists**) and `engine_run_distributed` (Dag-only, else
  `Err(ERR_FLOW_INVALID)`; calls `run_distributed_dag`). Returns `Ok(FlowResult*)` or the validate
  Err. **Faithful divergences (runner.rs):** (a) the **queue path** does NOT persist a final record
  and always passes `"failed"` (never `"rolled_back"`) to flow_failed; (b) **hardware check** deferred
  to row 17 (config.hardware stays 0); (c) **heartbeat** registers/deregisters but omits the 10s
  ticker (observably identical sub-10s; "can no-op"); (d) `engine_with_event_bus` omitted — majra
  `EventBus` deferred to M3. `tests/szal_engine_runner.tcyr` (54) ports the mod.rs Engine integration
  suite: mode dispatch, retry success/exhaust, transitive DAG failure, rollback ok/fail, fail-fast,
  invalid-flow rejection, cancellation (pre/partial/uncancelled), event-sink ordering + rollback
  events, execution-store persistence + the cancel-no-persist + queue-Running-only divergences,
  distributed dispatch + non-Dag rejection — **stable across 15 runs** (incl. the concurrent
  distributed path). lint/fmt/doc clean. Self-parity-checked branch-for-branch vs runner.rs (oracle
  pristine). **The engine is now fully wired end-to-end.**
- ✅ **row 21 `src/engine_subflow.cyr`** — `sub_flow_handler(storage, inner)`: a higher-order
  StepHandler. Cyrius closures capture nothing, so it returns a `cb_new(&_sub_flow_dispatch, ctx)`
  pair over a `{storage_vt, inner_handler}` ctx. `_sub_flow_dispatch`: non-`sub_flow` steps (incl.
  step_type None) delegate to `inner`; a `sub_flow` step reads `config.flow_name`, resolves it via
  `workflow_storage_get_by_name`, runs that FlowDef on a FRESH `engine_new(engine_config_default(),
  inner)` child, and returns the child `FlowResult` as the step's json_v output (`flow_result_to_json`
  → reparse). Exact error strings (mod.rs): `sub_flow step requires config.flow_name`,
  `sub-flow '<name>' not found in storage`, `sub-flow '<name>' failed: <detail>` (child validate Err,
  detail = `szal_get_err_msg`), `sub-flow '<name>' failed: <n> step(s) failed`. ABI is the standard
  `fn(step, ctx) → Result<json_v, Str>`. `tests/szal_engine_subflow.tcyr` (14) ports
  sub_flow_execution (output FlowResult shape) + delegation + missing-flow_name + not-found. lint/fmt/
  doc clean; self-parity-checked vs mod.rs (oracle pristine). **All engine modules are ported.**
- ⏳ **Next: row 17 `engine_hardware.cyr` (GATED) or begin M3.** Row 17 is the only unported §4
  engine row — blocked on **Q9** (`registry_new` collision, bote-core × ai-hwaccel; both define a
  24-byte vs 32-byte `registry_new`). Needs the port-plan §3.3 resolution (rename in the vendored/
  synced ai-hwaccel copy, or upstream rename) before porting. Otherwise M3 surface modules
  (`stream`, `sql_store`, `mcp*`) begin — see roadmap.md.

## Toolchain gotchas found during the port (for docs/cyrius-feedback.md)

- **bayan inline-parse miscompile (6.1.34):** calling `bayan_json_v_parse(...)` directly in `main()`
  when bayan + several project modules are included makes the parser globals read stale → every
  parse returns 0. Fix: wrap parsing in a one-line helper fn (`ctx_of(json)`); never call it inline.
- **CO-01 tail-call miscompile:** a Str/cstr-returning helper as the SOLE final argument of a call
  (e.g. `log_info(str_data(json))`) can miscompile the arg register → SIGSEGV. Fix: bind to a local
  first (`var d = str_data(json); log_info(d);`).
- `var buf[N]` size must be an integer literal or a SINGLE enum constant (no arithmetic: `buf[64*8]`
  rejected); per-call buffers via `alloc(N)` (arithmetic OK there). LSP/editor diagnostics
  over-approximate (false `undefined function` / `array size` errors) — only `cyrius build` counts.

## Build/test recipe (validated)

```
cyrius lib sync                                              # provision ./lib/ (gitignored)
cyrius build --strict --no-deps src/main.cyr build/szal      # entry build
cyrius build --strict --no-deps tests/szal_<mod>.tcyr build/test_<mod> && ./build/test_<mod>
```

Note: editor/LSP diagnostics over-approximate (false "undefined function" / "array size"
warnings); only `cyrius build` verdicts are authoritative.

## Dependencies

- stdlib (88 modules via `cyrius lib sync`): string, fmt, alloc, freelist, vec, str, hashmap,
  syscalls, tagged, result, fnptr, chrono, bayan (JSON), sakshi/log, patra, sigil, …
- `[deps.ai-hwaccel]` 2.3.9 (declared; overlaid by `cyrius deps` from M2 onward).
- **majra 2.4.6 — VENDORED** at `src/vendor/majra.cyr` (full dist, 9-symbol collision rename; needs
  `lib/thread.cyr`). Re-sync: `scripts/sync-majra.sh`. See [`majra-vendoring.md`](majra-vendoring.md).
- bote-core 2.7.3 — vendored at `src/vendor/bote-core.cyr` when M3 MCP lands.

## Consumers

_None yet — the port defines the `dist/szal.cyr` contract (daimon/sutra/AgnosAI/samay)._

## Next — ▶ START HERE (handoff)

**Done so far (M1 ✅ + M2 rows 8–21 ✅, all parity-verified 0-findings): 23 modules, 964 assertions,
0 failures, oracle pristine. Pin 6.1.37.** ALL engine modules are ported: six execution modes
(sequential/parallel/dag/hierarchical/queue/distributed) + core + step_exec, the `Engine` (row 20)
drives them end-to-end with rollback/persistence/cancellation, and `sub_flow_handler` (row 21) runs
nested flows. **M2 is feature-complete except the GATED row 17.** Full majra 2.4.6 vendored. Build
recipe + gotchas above.

**Pick up at: row 17 `engine_hardware.cyr` (GATED) — or start M3.**

Row 17 `engine_hardware.cyr` (rust-old/src/engine/hardware.rs, ~170 lines) is the ONLY unported §4
engine row. It is **blocked on Q9** (port-plan §3.3): both bote-core and ai-hwaccel export
`registry_new` (a 24-byte tool registry vs a 32-byte profile registry), and ai-hwaccel's
`registry_detect*` call `registry_new()` internally, so include order can't fix it. **Resolve first:**
sed-rename `registry_new`→`hw_registry_new` in szal's vendored/synced ai-hwaccel copy (the hoosh
vendoring pattern, like the 9-symbol majra rename), OR an upstream ai-hwaccel rename. Once resolved:
`HardwareContext` over `cached_registry_new(300)`; `hw_check_requirements(steps)` (first unsatisfiable
→ ERR_HW_UNAVAILABLE — wire into engine_runner's deferred check at engine_run/run_distributed/
run_with_cancellation, where `config.hardware != 0`); `hw_effective_concurrency(steps, base)` (min 1;
latent — never called by the engine, port anyway). Dep: the ai-hwaccel bundle (`[deps.ai-hwaccel]`
2.3.9, normal git block — no vendoring needed; it has zero sub-deps).

**Or start M3 surface modules** (no gate): `stream.cyr` (row 22, ProgressHub over majra pubsub),
`sql_store.cyr` (row 23, patra-backed ExecutionStore), then `mcp*` (rows 24-26, needs vendored
bote-core). See port-plan §4 + roadmap.md M3.

See [`roadmap.md`](roadmap.md) M2, [`port-plan.md`](port-plan.md) §4 (per-module spec),
[`parity-notes.md`](parity-notes.md) (accepted divergences §1–8 + audit log), and
[`majra-vendoring.md`](majra-vendoring.md) (re-sync).
