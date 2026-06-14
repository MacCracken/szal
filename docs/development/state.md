# szal — Current State

> Refreshed every release. CLAUDE.md is preferences/process/procedures
> (durable); this file is **state** (volatile).

## Version

**2.0.0** (in development) — Rust→Cyrius port. Rust 1.2.0 (13172 lines) frozen at
`rust-old/` (git tag `1.2.0`) as the parity oracle. `VERSION` is the single source of truth;
`cyrius.cyml` reads `${file:VERSION}`.

## Toolchain

- **Cyrius pin**: `6.1.37` (in `cyrius.cyml [package].cyrius`). The full suite (1036 assertions across
  27 module test files + the `szal.tcyr` smoke) + main are green under **6.1.37** and the installed
  **6.2.2** wrapper (2026-06-13; pin drift is a pending bookkeeping reconcile, suite green on both); the
  pin matches the installed wrapper — drift cleared. (Silence drift / lib-shadow notes per-invocation
  with `CYRIUS_NO_WARN_PIN_DRIFT=1` / `CYRIUS_NO_WARN_SHADOW_LIB=1`.) History: 6.1.33 (M0) → 6.1.34 →
  6.1.35 → 6.1.36 → 6.1.37.

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

**M2 — Engine core + executors. ✅ COMPLETE (2026-06-13).** All §4 rows 8–21 ported in topological
order, each cross-checked vs `rust-old` and self-tested `--strict` green. **Q9, Q10, Q11 all
RESOLVED.** Q10 (concurrency) + Q11 (logging) shipped in practice on the cooperative-cancel +
thread-safe-`alloc` model (parity-notes §8). **Q9 (`registry_new` collision) resolved 2026-06-13** by
the bote 2.7.5 re-sync (`registry_new`→`tool_registry_new`), which unblocked and shipped **row 17
`engine_hardware`** (below). The `Engine` (row 20) drives all six modes end-to-end, `sub_flow_handler`
(row 21) runs nested flows, and `engine_hardware` (row 17) gates execution on accelerator
availability — **M2 is feature-complete.**

**M3 — Streaming, persistence, MCP. ⏳ in progress (rows 22–23 done; bote vendored + MCP core done;
MCP pool/tenant + the 54 tools remain).**
- ✅ **row 22 `src/stream.cyr`** — `ProgressHub` + SSE encoding. Rust's `tokio::broadcast` →
  **per-subscriber bounded channels** (port-plan's documented alternative): `hub_new(cap≥1)`/
  `hub_subscribe`(→`chan_new(cap)`)/`hub_sink`(ProgressSink cb pair fanning to every subscriber)/
  `hub_subscriber_count`. `sse_frame(event/0, id/0, data)` (multi-line `data` → multiple `data:`
  lines + blank-line terminator), `progress_to_sse` (event=`SSE_EVENT_NAME`, id=step id, data=
  serialized StepProgress), `SSE_EVENT_NAME="step_progress"`. ONE lag-semantics divergence
  (parity-notes §9): `chan_send` blocks when a subscriber is full vs tokio drop-oldest (no
  `chan_try_send` in stdlib). `tests/szal_stream.tcyr` (14). lint/fmt/doc clean.
- ✅ **row 23 `src/sql_store.cyr`** — patra-backed `ExecutionStore`. Schema
  `execution_id STR, flow_name STR, data TEXT` (patra TEXT isn't WHERE-matchable + no PRIMARY KEY →
  keyed/filtered cols are 256-byte STR; only the JSON record is TEXT). Upsert = DELETE-then-INSERT
  (no `ON CONFLICT`; only INSERT writes TEXT). `sqlite_store_connect/migrate/save/get/list/remove/
  engine_sink/close`; `engine_sink` is a **synchronous** ExecutionStore vtable (no SpawnSink mirror/
  writer — patra writes synchronously, so ADR-0001 ordering holds trivially). Postgres dropped (Q6).
  Divergences in parity-notes §10. `tests/szal_sql_store.tcyr` (20, incl. a real `engine_run`
  persistence check through `engine_sink`). lint/fmt/doc clean. **First functional use of stdlib patra.**
- ✅ **bote-core vendored** `src/vendor/bote-core.cyr` (bote **2.7.5** `dist/bote-core.cyr`, 2,025
  lines) via `scripts/sync-bote.sh`, with a 1-symbol rename `compiled_compile`→`bote_compiled_compile`
  (vs szal's condition compiler; full fn/var collision scan otherwise clean). hoosh pattern (no
  `[deps.bote]` block → no recursive bloat). **Re-synced 2.7.3→2.7.5 (2026-06-13):** bote renamed its
  tool-registry ctor `registry_new`→`tool_registry_new` upstream; szal's sole caller
  (`mcp.cyr register_tools_with`) updated to match. This **dissolves Q9** — bote no longer owns the
  `registry_new` symbol, so the bote×ai-hwaccel collision that gated row 17 is gone. Full suite green
  on 2.7.5 (27 files / 1026 assertions).
- ✅ **row 24 (core) `src/mcp.cyr`** — `result_ok/ok_json/error/error_typed` (JSON `{content,isError}`
  + `_meta.error_code/retryable`), `McpErrorCode` (6; Timeout/IoError/Internal retryable),
  **`validate_path`** (the SECURITY boundary: lexical component-walk resolving `.`/`..` + CWD
  confinement via raw `getcwd` syscall 79 — no fs canonicalize in Cyrius; symlink resolution is the
  roadmap open Q, lexical is the 2.0.0 choice), `mcp_tool_def`/`mcp_tool_new` (Tool = (ToolDef,
  handler_fp) pair), `register_tools[_with]` over the bote-core dispatcher. Handler ABI
  `fn(args_cstr, claims) → result_cstr`. `tests/szal_mcp.tcyr` (26: result builders, error codes,
  validate_path traversal rejection `/etc/passwd` + `../../`, dispatcher registration). lint/fmt/doc
  clean. The 54-tool `all_tools()`/`szal_register_tools()` aggregator is deferred to the last tool
  file (single-pass: no forward ref).
- ✅ **row 24 (pool) `src/mcp_pool.cyr`** (2026-06-13) — `NetworkPool` = 3 majra `ratelimit_new`
  buckets (HTTP 10/s b50, DNS 100/s b200, Port 50/s b100; fixed-point ×1000, cstr-content-keyed so
  hosts get independent buckets). `network_pool_new`/`pool_check_http`/`_dns`/`_port` + lazy global
  `pool()` (Rust `NETWORK_POOL` LazyLock). **First functional use of vendored majra `ratelimit_*`.**
- ✅ **row 24 (tenant) `src/mcp_tenant.cyr`** (2026-06-13) — `TenantQuota` (rate f64 + max_flows),
  `TenantCtx` (id/display_name?/quota/allowed_tools set?), `TenantRegistry` (mutex-guarded
  `map_new_str`; RwLock→mutex per parity-notes §7). `tenant_register`/`get`/`deregister`,
  `check_tenant_quota` (unknown→permissive Ok; shared `tenant_limiter()` 100/s b500 keyed by id) +
  `check_tenant_tool_access` (allowlist membership; exact `tenant <id> is not permitted to use tool
  <tool>` msg). `tenant_ctx_to/from_json` serde (rate as JSON float via `bayan_json_v_float*`;
  allowed_tools as array). Shared `tests/szal_mcp_pool_tenant.tcyr` (32) ports all pool.rs + tenant.rs
  tests (incl. burst exhaustion, time-based refill, serde round-trip). lint/fmt/doc clean.
- ✅ **MCP tools group 1/15 `src/mcp_tools_encoding.cyr`** (2026-06-13) — `szal_uuid` (count default
  1, cap 100; 1→bare string, N→JSON array) + `szal_base64` (encode/decode; missing-input + bad-op
  Validation errors). Handlers follow the reusable pattern above (`_parse_args` Str-wrap, CO-01
  binding, `base64_decode` `{ptr,len}`). `encoding_tools()` returns the group's 2 Tool pairs.
  `tests/szal_mcp_tools_encoding.tcyr` (19) ports all encoding_tools.rs tests + registration. One
  accepted divergence (parity-notes §12: base64 decode has no error/UTF-8 signalling). lint/fmt/doc clean.
- ✅ **MCP tools group 2/15 `src/mcp_tools_hash.cyr`** (2026-06-13) — `szal_sha256` (string `input` or
  `file` via validate_path → IoError/PermissionDenied; `{algorithm,hash,input_bytes}`), `szal_md5`
  (required string), `szal_random_token` (default 32 / cap 256 bytes → 2× hex via `random_bytes` +
  `hex_encode`). Uses sigil `sha256_hex` + szal `md5_hex`. `tests/szal_mcp_tools_hash.tcyr` (19) ports
  all hash_tools.rs tests (known SHA-256 vector, file hashing, validation errors, token lengths) +
  registration. One accepted divergence (parity-notes §13: sha256 file read capped at 1 MiB).
  Surfaced the **sigil/`ERR_NONE` include-ordering trap** (now in the pattern notes above). lint/fmt/doc clean.
- ✅ **MCP tools group 3/15 `src/mcp_tools_system.cyr`** (2026-06-13) — `szal_system_info`
  (hostname from /etc/hostname, os/arch literals, cpus via `sched_getaffinity` popcount, uptime float
  from /proc/uptime), `szal_cwd` (`_mcp_getcwd`), `szal_env_get` (`getenv`; unset → NotFound),
  `szal_timestamp` (iso8601 + unix secs/ms). `tests/szal_mcp_tools_system.tcyr` (20) ports all
  system_tools.rs tests + registration. One accepted divergence (parity-notes §14: hardcoded
  os/arch + second-precision timestamp). lint/fmt/doc clean.
- ✅ **MCP tools group 4/15 `src/mcp_tools_json.cyr`** (2026-06-13) — `szal_json_path` (dot-path walk;
  numeric segs index arrays, others key objects; NotFound on miss), `szal_json_diff` (structural
  `_json_eq` — order-independent objects, INT≠FLOAT like serde — + both type names), `szal_json_validate`
  (`{valid,type,size_bytes}` or `{valid:false,error,position}`). `tests/szal_mcp_tools_json.tcyr` (20)
  ports all json_tools.rs tests + key-order/int-vs-float equality edges + registration. One accepted
  divergence (parity-notes §15: validate reports byte position, not line/column). lint/fmt/doc clean.
- ✅ **MCP tools group 5/15 `src/mcp_tools_template.cyr`** (2026-06-13) — `szal_template_render`
  (reuses szal's `condition.cond_render_template`), `szal_wc` (lines/words/chars/bytes of text or
  file), `szal_text_replace` (all/first via a ported `_str_replace`), `szal_text_split` (string
  delimiter → JSON array via `_split_by_str`), `szal_text_join`. `tests/szal_mcp_tools_template.tcyr`
  (25) ports all template_tools.rs tests + missing-field validations + registration. One accepted
  divergence (parity-notes §16: wc file read capped at 1 MiB). lint/fmt/doc clean.
- ✅ **MCP tools group 6/15 `src/mcp_tools_conversion.cyr`** (2026-06-13) — `szal_base_convert`
  (2/8/10/16, prefix-strip, i64), `szal_byte_format` (KB/MB/GB/TB; `fmt_float_buf` 2-decimal), 
  `szal_duration_format` (Xd Xh Xm Xs). `tests/szal_mcp_tools_conversion.tcyr` (20) ports all
  conversion_tools.rs tests + validation + registration. One accepted divergence (parity-notes §17:
  base_convert i64 not u128). lint/fmt/doc clean.
- ✅ **MCP tools group 7/15 `src/mcp_tools_math.cyr`** (2026-06-13) — `szal_math_eval`: a
  recursive-descent f64 evaluator (tokenize → add/sub → mul/div/mod → atom; unary minus, parens;
  `+ - * / %`; char-allowlist injection guard; div/mod-by-zero errors). Uses `lib/math.cyr`
  (f64_parse_ok/trunc/fract). Token helpers renamed `_mev_*` to avoid condition.cyr's `_tok_new`
  collision. `tests/szal_mcp_tools_math.tcyr` (26) ports math_tools.rs tests + precedence/parens/unary/
  mod/div-zero/unbalanced/missing. One accepted divergence (parity-notes §18: non-integer float
  formatting; `^` omitted as in Rust). lint/fmt/doc clean.
- ✅ **MCP tools group 8/15 `src/mcp_tools_state.cyr`** (2026-06-13) — `szal_state_check`
  (terminal-ness + valid transitions), `szal_state_transition` (from→to validity), `szal_state_lifecycle`
  (full FSM as an 8-element array). All three reuse szal's own `state.cyr` FSM (`state_name`/
  `state_from_name`/`state_is_terminal`/`state_valid_transition`), so state semantics are inherited
  exactly — **no parity divergence**. `tests/szal_mcp_tools_state.tcyr` (21) ports all state_tools.rs
  tests + validation + registration. lint/fmt/doc clean.
- ✅ **MCP tools group 9/15 `src/mcp_tools_step.cyr`** (2026-06-13) — `szal_step_create` (builds a
  StepDef from optional fields; depends_on UUIDs parsed via uuid_parse → invalid-UUID validation),
  `szal_step_validate` (parse + empty-name/zero-timeout checks → `valid: step '<name>' (id=<uuid>)`),
  `szal_step_inspect` (structured config + dependency list). All reuse szal's own `step.cyr` builders
  + `step_to_json`/`step_from_json` — **no parity divergence**. `tests/szal_mcp_tools_step.tcyr` (24)
  ports all step_tools.rs tests + bad-dep/missing-field + registration (step JSON built via
  step.cyr+bayan for correct escaping). lint/fmt/doc clean.
- ✅ **MCP tools group 10/15 `src/mcp_tools_flow.cyr`** (2026-06-13) — `szal_flow_create` (name/mode +
  rollback/timeout + inline steps), `szal_flow_validate` (reuses `flow_validate` → DAG cycle/missing-dep
  detection), `szal_flow_from_json` (structured summary + steps), `szal_flow_list_modes` (4 modes),
  `szal_flow_add_step`. All reuse szal's own `flow.cyr`/`step.cyr` — flow/step serde + cycle validation
  inherited exactly. `tests/szal_mcp_tools_flow.tcyr` (29) ports all flow_tools.rs tests (incl. the DAG
  cycle case) + validation + registration. One minor divergence (parity-notes §19: inline-step lenient
  deserialization, §3 family). lint/fmt/doc clean.
- ✅ **MCP tools group 11/15 `src/mcp_tools_engine.cyr`** (2026-06-13) — `szal_engine_create` (config
  echo, default concurrency 16), `szal_result_inspect` (FlowResult summary + Completed/Failed step
  counts), `szal_step_status_list` / `szal_error_list` / `szal_server_info` (static JSON). Pure
  JSON-shaping — no szal core module needed. `tests/szal_mcp_tools_engine.tcyr` (28) ports all
  engine_tools.rs tests + result counting + validation + registration. One accepted divergence
  (parity-notes §20: server version hardcoded — keep in sync with VERSION). lint/fmt/doc clean.
- ✅ **MCP tools group 12/15 `src/mcp_tools_file.cyr`** (2026-06-13) — `szal_file_read` (max_bytes cap),
  `szal_file_write` (create/append), `szal_dir_list` (recursive, depth-20/max-entries bounded),
  `szal_file_stat`, `szal_path_exists`. **EVERY op routed through validate_path** (CWD confinement);
  stat via `newfstatat` (syscall 262, x86_64-stable). `tests/szal_mcp_tools_file.tcyr` (28) ports all
  file_tools.rs tests **including the path-traversal/outside-cwd rejections** + round-trip/append/
  dir-list/stat + registration (cwd-relative paths, self-cleaning). Two divergences (parity-notes §21:
  byte-boundary read truncation; second-precision stat mtime). lint/fmt/doc clean.
- ⏳ **Next: MCP tool groups 13–15/15** (~13 tools left; **security-sensitive**:
  process → git → net). Each a
  `mcp_tool_new(mcp_tool_def(...), &handler)` + a `<group>_tools()` accumulator; the LAST file defines
  `all_tools()`/`szal_register_tools()`. **Security checks must not regress**: validate_path on all
  file ops, 1 MiB read cap / 10k dir entries / depth 20; process: no shell, reject `..`/`/`, 30s
  timeout; git: `validate_git_ref` rejects leading `-`, log cap 100; net: `is_safe_url` SSRF guard —
  metadata/localhost/RFC1918 — + the `pool()` rate-limit checks. Follow the handler pattern above.

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
- ✅ **row 17 `src/engine_hardware.cyr`** (2026-06-13) — `engine/hardware.rs`. `HardwareContext` = the
  ai-hwaccel `CachedRegistry` handle (`hw_detect`=`cached_registry_new(300)`, `hw_with_ttl`,
  `hw_registry`=`cached_get`); `hw_check_requirements(ctx, steps)` (skip `REQ_NONE`; first
  unsatisfiable → `Err(ERR_HW_UNAVAILABLE)` via `count_satisfying`, exact `hardware unavailable: step
  '<name>' requires <req>…` message); `hw_effective_concurrency(ctx, steps, base)` (GPU/TPU cap, floor
  1 — latent, never called by the engine); `hw_debug`; `engine_config_with_hardware` (mirrors
  `EngineConfig::with_hardware`). **Wired into `engine_runner`** at all three entry points
  (`engine_run`/`_with_cancellation`/`_distributed`) via `_engine_check_hardware` — runs after
  `flow_validate`, no-op when `config.hardware==0`, mirroring the Rust `#[cfg(feature="hardware")]`
  blocks. ai-hwaccel overlaid into the build (`lib/ai-hwaccel.cyr` + its `lib/fs`/`lib/process` deps);
  **Q9 dissolved** by the bote 2.7.5 re-sync (no more `registry_new` clash; collision scan clean).
  Two accepted divergences (parity-notes §11: flattened `min_chips`=1; `requirement_name` token).
  `tests/szal_engine_hardware.tcyr` (10) ports all five `hardware.rs` tests (accelerator-guarded like
  Rust) + szal runner-wiring (hw-pass / hw-reject / no-ctx-noop). Self-parity-checked branch-for-branch
  (oracle pristine). lint/fmt/doc clean. **M2 is now feature-complete (rows 8–21 all done).**
- ⏳ **Next: continue M3 — MCP pool/tenant + the 54 tools** (see below). No engine rows remain.

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

### MCP tool-handler porting pattern (reusable across all `mcp_tools_*.cyr`)

Hard-won while porting the first tool group (`mcp_tools_encoding.cyr`) — apply to every tool file:
- **Handler ABI:** `fn _name(args, claims)` where `args` is a **cstr**; return `str_data(result)`
  (the `result_*` builders NUL-terminate via `str_builder_build`, so the cstr is valid).
- **`bayan_json_v_parse` takes a `Str`, not a cstr** (it calls `str_data(src)` internally). Handler
  args arrive as a cstr → wrap first: `fn _parse_args(a) { var s = str_from(a); return bayan_json_v_parse(s); }`.
  Passing the raw cstr segfaults (reads `cstr+0` as the Str data-ptr field). This is distinct from
  the inline-parse gotcha above — it's a type mismatch.
- **CO-01 bites hard here:** `str_from(uuid_to_cstr(...))` / any `outer(inner_cstr_helper(...))` as the
  sole arg SIGSEGVs. Always bind the inner to a local first.
- **`base64_decode` returns a `{ptr, len}` pair**, not a cstr → `str_new(load64(p), load64(p+8))`.
  `base64_encode` returns a plain NUL-terminated cstr (use directly).
- **Result envelope `content` is always a JSON array**, so substring `"["` checks on a result are
  meaningless — in tests, extract `content[0].text` (parse the result, `arr_get(content,0)`, `."text"`)
  to inspect the real payload.
- **Schema-prop value cstrs can exceed the 120-col lint** — assemble long ones in two
  `str_builder_add_cstr` pieces (keeps exact description text). Property `description`/schema spacing
  is below the parity threshold (developer-facing hint, untested) — compact JSON is fine.
- **Invoke a handler fn-pointer with `fncall2(fp, args, 0)`** (lib/fnptr.cyr); register a group with
  `register_tools(<group>_tools())`.
- **stdlib `ERR_NONE` collision (sigil):** `lib/sigil.cyr` (and likely other crypto libs) defines its
  own `ERR_NONE` enum constant that collides with `src/error.cyr`'s. Including sigil BEFORE error.cyr
  breaks error.cyr's enum resolution (`error.cyr:36 var buf[SZAL_ERR_MSG_CAP]` → "array size must be
  an enum constant"). Fix: include such libs AFTER the szal core modules, right before the tool file
  that needs them (see `main.cyr`: ct/thread_local/sigil/random sit just before `mcp_tools_hash`).
  sigil also needs `lib/ct.cyr` + `lib/thread_local.cyr` (constant-time + TLS helpers) included first;
  its unreachable SHA-3/threading paths emit benign "undefined function" warnings (DCE'd, build OK).

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
- **bote-core 2.7.5 — VENDORED** at `src/vendor/bote-core.cyr` (2,025 lines; 1-symbol rename
  `compiled_compile`→`bote_compiled_compile`). Re-sync: `scripts/sync-bote.sh`. 2.7.5 renamed
  `registry_new`→`tool_registry_new` (Q9 dissolved; see below).

## Consumers

_None yet — the port defines the `dist/szal.cyr` contract (daimon/sutra/AgnosAI/samay)._

## Next — ▶ START HERE (handoff)

**Done so far (M1 ✅ + M2 ✅ COMPLETE (rows 8–21) + M3 rows 22–24-core + pool/tenant + tool groups
1–12/15 (encoding…file) + bote vendoring, all parity-verified 0-findings): 41 modules, 1347
assertions, 0 failures, oracle pristine. Pin 6.1.37 (installed 6.2.2 — green both).**
All engine modules ported (six modes + core + step_exec + Engine + sub_flow + **hardware/row 17**).
M3: streaming (`stream.cyr`) + persistence (`sql_store.cyr`, patra) + MCP core (`mcp.cyr` —
result/errcode/**validate_path security**/registration) + **MCP pool + tenant** done; **bote-core
2.7.5 vendored (re-synced 2026-06-13; Q9 dissolved)**. Full majra 2.4.6 + ai-hwaccel 2.3.9 (overlaid)
in the build. Build recipe + gotchas above (add `CYRIUS_NO_WARN_SHADOW_LIB=1` to silence lib-shadow).

**Pick up at: MCP tool groups 13–15 (M3) — the remaining SECURITY-SENSITIVE ones. Groups 1–12/15
done; ~13 tools left.** No engine rows remain; MCP infra + pool/tenant done. **Read the "MCP
tool-handler porting pattern" gotchas above first.** File group establishes the validate_path +
`newfstatat` stat pattern to reuse.

MCP infra is fully in place: bote-core vendored, `mcp.cyr` core, `mcp_pool.cyr`, `mcp_tenant.cyr`,
and 12 tool groups: encoding, hash, system, json, template, conversion, math, state, step, flow,
engine, file (file_read/write/dir_list/file_stat/path_exists, validate_path-gated) — all tested.
**SECURITY (do NOT regress): process: no shell, reject `..`/`/`, 30s timeout; git: validate_git_ref
rejects leading `-`, log cap 100; net: is_safe_url SSRF guard (metadata/localhost/RFC1918) + pool()
rate limits.** The LAST tool file (net) must define `all_tools()` (aggregate all 15 groups' vecs) +
`szal_register_tools()` = `register_tools(all_tools())`. Remaining MCP:
1. **`src/mcp_tools_*.cyr`** (3 files left, ~13 tools; first 12 groups ✅ done) — order-free; suggested
   process → git → net. Each tool:
   `mcp_tool_new(mcp_tool_def(name, desc, props_vec, required_vec),
   &handler)` with handler `fn(args_cstr, claims) → result_cstr` returning a `result_*` string. The
   LAST file defines `all_tools()` (aggregates every group's tools into one vec) + `szal_register_tools()`
   = `register_tools(all_tools())`. Tool names are `szal_*` (≥1 underscore); do NOT export bare `mcp_*`
   (daimon owns that). **Security checks must NOT regress:** `validate_path` (done — use it) on all
   file ops, 1 MiB read cap / 10k dir entries / depth 20; process: no shell, reject `..`/`/`, 30s
   timeout; git: `validate_git_ref` rejects leading `-`, log cap 100; net: `is_safe_url` SSRF guard
   (metadata endpoints + localhost + RFC1918) + the pool rate-limit checks. Port + test per tool group.

**Row 17 `engine_hardware.cyr` — ✅ DONE (2026-06-13, see the row-17 bullet above + the resolved
[`issues/2026-06-11-registry-new-collision.md`](issues/2026-06-11-registry-new-collision.md)).** Q9
was dissolved by the bote 2.7.5 re-sync; ai-hwaccel is overlaid into the build and `engine_runner`
consults `config.hardware` at all three entry points. Nothing left to do here.

See [`roadmap.md`](roadmap.md) M3, [`port-plan.md`](port-plan.md) §4 (per-module spec),
[`parity-notes.md`](parity-notes.md) (accepted divergences §1–11 + audit log), and
[`majra-vendoring.md`](majra-vendoring.md) (re-sync).
