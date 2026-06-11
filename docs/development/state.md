# szal — Current State

> Refreshed every release. CLAUDE.md is preferences/process/procedures
> (durable); this file is **state** (volatile).

## Version

**2.0.0** (in development) — Rust→Cyrius port. Rust 1.2.0 (13172 lines) frozen at
`rust-old/` (git tag `1.2.0`) as the parity oracle. `VERSION` is the single source of truth;
`cyrius.cyml` reads `${file:VERSION}`.

## Toolchain

- **Cyrius pin**: `6.1.35` (in `cyrius.cyml [package].cyrius`), matching the installed wrapper —
  no drift. Bumped 6.1.34→6.1.35 on 2026-06-11 after the parity audit landed green under the new
  wrapper (full build/test/fmt/lint/doc re-verified). History: 6.1.33→6.1.34 earlier to chase the
  same drift.

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

**M2 — Engine core + executors. ⏳ in progress.** Porting port-plan §4 rows 8–21 in topological
order. Open questions Q9 (`registry_new` collision, blocks `engine_hardware`), Q10 (concurrency),
Q11 (logging under threads) still gate the parallel/hardware rows — the early rows below are
unblocked.

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
- ⏳ Next: row 11 `engine_core.cyr` (the big one: `engine/mod.rs` minus `sub_flow_handler` —
  FlowCtx/ExecCtx, EngineConfig, handler ABI, check_condition; needs no majra, holds `metrics_vt`
  as an opaque slot). Then step_exec → sequential → dag → hierarchical (pure logic, unblocked).
  Parallel/hardware still gated on Q9/Q10/Q11; queue_runner/distributed now have majra (still need
  the concurrency sign-off).

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

## Next

**M2 in progress — rows 8–10 (`engine_result`, `storage`, `metrics`) done & verified; next is
row 11 `src/engine_core.cyr`.** engine_core is the large one (`engine/mod.rs` minus
`sub_flow_handler`: FlowCtx/ExecCtx, EngineConfig, the (fn-ptr, ctx) handler ABI, check_condition,
emit, progress sink). It needs **no majra** — it holds `metrics_vt`/`storage_vt` as opaque slots
(0 = none). After it: step_exec → sequential → dag → hierarchical are pure-logic and unblocked.
**Gated rows** need sign-off + the full majra vendoring: parallel/hardware (Q9 `registry_new`, Q10
concurrency, Q11 logging) and queue_runner/distributed (full majra — see
[`majra-vendoring.md`](majra-vendoring.md)). See [`roadmap.md`](roadmap.md) M2,
[`port-plan.md`](port-plan.md) §4, and accepted divergences in [`parity-notes.md`](parity-notes.md).
