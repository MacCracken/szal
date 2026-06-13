# szal — Parity Notes (accepted Rust → Cyrius divergences)

> The port's correctness bar is "matches what Rust did" (`rust-old/` is the frozen oracle).
> This file is the canonical home for the **deliberate, accepted divergences** — the cases where
> the Cyrius port intentionally behaves differently from Rust for a documented reason. Inline
> `# … see parity-notes.md` comments across `src/*.cyr` point here. Anything NOT listed here is a
> bug if it diverges. Diverging further requires either an entry here or an ADR.
>
> Most entries are **non-observable in practice** — they only manifest at physically unreachable
> magnitudes or for hand-crafted/partial JSON that neither side's serializer ever emits. They are
> recorded so parity audits can distinguish "known + accepted" from "new regression".

## 1. Integer width: Rust `u64`/`u32` → Cyrius `i64`

**What:** Cyrius's native integer is a 64-bit **signed** `i64`; there is no `u64`. Every Rust
`u64`/`u32` field is carried as `i64` throughout the port: `StepDef.timeout_ms / max_retries /
retry_delay_ms`, `StepResult.duration_ms / attempts`, `WorkflowEvent.duration_ms / attempt`,
`FlowDef.timeout_ms / version`, `FlowResult.total_duration_ms`, backoff math, etc.

**Divergence:** Values in `(i64::MAX, u64::MAX]` (i.e. ≥ 2^63) wrap to negative in Cyrius and do
not round-trip through JSON. Rust's serde handles the full unsigned range.

**Why accepted:** These fields are durations-in-ms, retry counts, attempt counts, and versions. A
duration > `i64::MAX` ms is ~292 million years; a retry/attempt count near 2^63 is impossible. No
realistic workflow reaches these magnitudes, so the divergence is unobservable. Introducing a fake
`u64` only for these fields would be inconsistent with the whole codebase and the stdlib JSON
parser (`lib/bayan.cyr`, which is vendored and must not be modified).

**Where:** all numeric struct fields; backoff saturation is the sharpest case — see §2.

## 2. Backoff saturation: `i64::MAX` vs `u64::MAX`

**What:** `step_backoff_delay_ms` (Linear `base*attempt`, Exponential `base*2^(attempt-1)`) uses
saturating math. On overflow Cyrius clamps to `i64::MAX` (`0x7FFFFFFFFFFFFFFF`, `STEP_I64_MAX`);
Rust's `checked_shl(...).unwrap_or(u64::MAX)` / saturating mul clamps to `u64::MAX`.

**Divergence:** the saturation ceiling differs by the sign bit (`i64::MAX` ≈ half `u64::MAX`).

**Why accepted:** a backoff delay anywhere near `i64::MAX` ms is unreachable in practice; the
clamp only fires on pathological `base`/`attempt`. Consequence of §1. See `src/step.cyr:50-51,
123-156`.

## 3. Lenient serde-default deserializers (`_*_from_v` helpers)

**What:** the JSON deserializers — `_step_from_v` / `step_from_json`, `_flow_from_v` /
`flow_from_json`, `_step_result_from_v` / `step_result_from_json`, `_flow_result_from_v` /
`flow_result_from_json` — apply **serde-default semantics to every field**: a missing or
type-mismatched field falls back to its zero/default value (`""`, empty vec, `0`, `false`,
`Null`, `None`) rather than aborting. A missing/malformed UUID id yields a freshly generated id
(the port always carries a valid `StepId`/`FlowId`).

**Divergence:** Rust serde marks non-`Option`, non-`#[serde(default)]` fields as **required** and
**errors** on missing/unknown input (e.g. `StepResult.{step_id, status, output, duration_ms,
attempts}` and all `FlowResult` fields have no `#[serde(default)]`). The Cyrius port instead
succeeds with defaults. So partial/hand-crafted JSON that Rust rejects round-trips in Cyrius.

**Why accepted:** this is a deliberate, **port-wide** convention (not a per-field decision) — the
deserializers are uniformly lenient so the engine never aborts on a slightly-stale persisted
record. It is **unobservable for real round-trips**: every serializer (`*_to_json`) emits all
fields unconditionally (the structs derive `Serialize` with no skip attrs except documented
`Option`/`Vec::is_empty` cases), so the leniency only changes behavior for inputs neither side
produces. Confirmed intentional in `docs/development/port-plan.md` §4 (step "serde-default
semantics: missing backoff/trigger_mode/sub_steps must default, not fail").

**Where:** `src/step.cyr` (`_step_from_v`, `_step_result_from_v`), `src/flow.cyr`
(`_flow_from_v`), `src/engine_result.cyr` (`_flow_result_from_v`).

## 4. Timestamp precision: epoch-ns `i64` internal, second-precision ISO-8601 at boundaries

**What:** `WorkflowEvent.timestamp` is epoch-nanoseconds `i64` internally; `event_to_json` emits
ISO-8601 at **second** precision (the epoch-ns is truncated to seconds via `iso8601`).

**Divergence:** Rust's `chrono::DateTime<Utc>` serializes at sub-second (RFC-3339) precision; the
Cyrius boundary form drops sub-second digits, so a serialized timestamp does not byte-match Rust's.

**Why accepted:** pre-port decision (roadmap "Timestamps"): `iso8601()` second precision cannot
round-trip sub-second, and second precision is sufficient for event ordering. See
`src/bus.cyr:280-289`.

## 5. Condition numeric comparison as `f64`; cross-type compares are false

**What:** the condition DSL evaluator compares numbers as `f64` and returns `0` (false) for any
cross-type comparison. Equality is same-type-only.

**Why accepted:** mirrors the Rust evaluator's truthiness/comparison rules exactly (this is parity,
documented here because `src/condition.cyr:498` points at parity-notes). See `src/condition.cyr`.

## 6. Storage: returns the stored pointer, not a deep clone

**What:** `WorkflowStorage::get_by_name`/`get_by_id` and `ExecutionStore::get`/`remove` in Rust
return `.cloned()` values (independent deep copies). The Cyrius `in_memory_storage_*` /
`in_memory_execution_store_*` impls return the **stored heap pointer** directly.

**Divergence:** a caller that mutates a returned `FlowDef`/`ExecutionRecord` would also mutate the
stored copy (aliasing), whereas Rust hands back an independent clone.

**Why accepted:** the storage read path exists to resolve a sub-flow definition for execution,
which is read-only; there is no `flow_clone`/`record_clone` in the port and adding deep-copy
machinery for an unused mutation path is unwarranted. Observable results of get/list/remove match
Rust exactly. See `src/storage.cyr`.

## 7. Storage: `RwLock` deferred (single-threaded until concurrency sign-off)

**What:** Rust wraps the storage/execution-store maps in `std::sync::RwLock`. The port uses a plain
map with no lock.

**Why accepted:** the whole engine is single-threaded until the concurrency model is signed off
(roadmap M2, Q10/Q11). The data-structure behavior the tests assert is lock-independent; the lock
lands together with the parallel engine rows (`engine_parallel`/`engine_distributed`), not before.
See `src/storage.cyr`.

## 8. Step timeout: cooperative cancel (orphaned worker), not async abort

**What:** Rust wraps each step attempt in `tokio::time::timeout(...)` (async; drops the future on
the deadline). Cyrius runs the handler on a **worker thread** and polls its result channel against a
`clock_now_ms()` deadline (`engine_step_exec.cyr`). On timeout, szal returns `StepTimeout` on time,
but the orphaned worker thread runs to completion.

**Divergence:** a handler that exceeds its deadline keeps executing in the background instead of
being cancelled. Observable only if the handler has side effects after the deadline.

**Why accepted (and why it's the *only* faithful option):** OS threads cannot be force-aborted —
this is true of Rust's own `std::thread` (no `.abort()`); only async tasks are cancellable, and even
`tokio::timeout` only cancels at `await` points (it cannot interrupt blocking work either). Cyrius
has a cooperative async runtime (`lib/async.cyr`) but **`async_timeout` forks a child process**, so a
step's shared-state mutations would be lost — wrong for steps. The port-plan §1.7 therefore
prescribes exactly the worker-thread + deadline-poll approach used here. Token-based cancellation
(`run_with_cancellation`) has **exact** parity: Rust's `tokio_util::CancellationToken` is itself
cooperative/poll-based and maps 1:1 to `cancel_token_new/signal/check`. Only the *timeout abort*
differs, and only for misbehaving handlers. See `src/engine_step_exec.cyr`.

> Concurrency scope: in sequential execution the main thread only polls while a worker runs.
> **For parallel execution (rows 14/15) both prerequisites are already satisfied:** (a) `alloc()`
> is thread-safe — `lib/alloc.cyr` has a global allocation lock (v6.0.64) precisely because real
> Cyrius threads share one process heap; (b) handler errors are per-worker isolated by design — the
> handler ABI returns `Err(message_Str)` (no shared global error buffer). So `engine_parallel`/
> `engine_dag` need only a permit-semaphore (build from a bounded `chan` or mutex+counter) +
> `cancel_token_*` — no new allocator work. The model template is ai-hwaccel's `async_detect`
> (`thread_create` + per-resource `mutex`).

## 9. ProgressHub: per-subscriber channels back-pressure, not drop-oldest (`stream.cyr`)

**What:** Rust's `ProgressHub` is a `tokio::sync::broadcast` channel — lossy under back-pressure: a
slow subscriber observes `RecvError::Lagged` and the producer never stalls (the oldest buffered
events are dropped for that subscriber). The Cyrius port (`stream.cyr`) fans each event out to a
per-subscriber bounded `chan_new(capacity)` (port-plan §4 row 22's documented alternative).

**Divergence:** Cyrius has only a **blocking `chan_send`** (no `chan_try_send`), so when a
subscriber's channel is full the producer BLOCKS until that subscriber drains, rather than dropping
the oldest event and continuing. A wedged slow subscriber back-pressures the engine's progress
emission instead of losing events.

**Why accepted:** the stdlib offers no non-blocking bounded send, and the alternatives (a background
drain thread per subscriber, or a custom ring buffer) add real machinery for a fan-out whose only
caller is fire-and-forget progress telemetry. Per-subscriber channels give exact `subscriber_count`,
a configurable per-subscriber backlog (`capacity`, clamped ≥1 like Rust), and the no-subscribers-is-
a-no-op behavior — all of which the Rust tests assert. Only the *lag policy* differs (block vs
drop-oldest), and only for a subscriber that has stopped reading. Bound `capacity` for the expected
backlog. This is the M3 "pub/sub lag semantics" open question from roadmap.md, resolved for the
progress path. (The same block-vs-drop delta applies to any future majra-pubsub-backed `EventBus`.)
See `src/stream.cyr`.

## 10. SQL store: patra column model + DELETE-then-INSERT upsert + synchronous writes (`sql_store.cyr`)

**What:** Rust's `sql_store.rs` is sqlx-backed (SQLite + a feature-gated Postgres twin) with a
`TEXT TEXT TEXT` schema, `INSERT ... ON CONFLICT DO UPDATE` upsert, and an async fire-and-forget
`engine_sink` (ordered background writer + in-memory mirror to preserve submission order). The port
(`sql_store.cyr`) targets the stdlib **patra** engine.

**Divergences (all behavior-preserving):**
1. **Schema `execution_id STR, flow_name STR, data TEXT`** (not `TEXT TEXT TEXT`). patra's
   variable-length `TEXT` column is chain-page-backed and **not WHERE-matchable**, and patra has no
   `PRIMARY KEY`. The two filtered columns must be the fixed 256-byte `STR` type; only the large
   serialized record uses `TEXT`. A UUID id (36 chars) + a flow name both fit in 256 bytes.
2. **Upsert = DELETE-then-INSERT.** patra has no `ON CONFLICT`, and only `INSERT` writes a `TEXT`
   column. `save` deletes any existing row for the id then inserts. One-row-per-id preserved.
3. **Postgres dropped** for 2.0.0 (roadmap Q6); the Rust `macro_rules!` twin collapses to one store.
4. **Synchronous writes — no SpawnSink.** patra writes synchronously (port-plan §3.4 recommends
   exactly this), so the ordered-writer/mirror/`Once` machinery is unnecessary and the
   "Running must not overwrite a later Completed" guarantee (ADR 0001) holds trivially. Tests need
   no poll-for-persist loop (Rust's did, because its writes were fire-and-forget).

**Why accepted:** patra is the chosen SQL backend (port-plan §2/§3.4); each divergence is forced by
patra's column/SQL model and the synchronous-store decision, and all five Rust behavioral tests pass
unchanged in shape. See `src/sql_store.cyr`.

---

## 11. Hardware: flattened `AcceleratorRequirement` (no `min_chips`) + requirement-token spelling (`engine_hardware.cyr`)

**What:** Rust's `HardwareContext::check_requirements` builds the unavailable error from
`format!("{:?}", step.hardware)` — the `Debug` of the `AcceleratorRequirement` enum, which carries
data (`Tpu { min_chips }`). szal's `StepDef.hardware` was flattened to a plain `REQ_*` i64 at port
row 3 (port-plan §4), dropping the enum payload.

**Divergences (both behavior-preserving for szal's repr):**
1. **`min_chips` is fixed at 1** in `hw_check_requirements` (`count_satisfying(req, 1, profs)`).
   szal has no per-step `min_chips` to thread through, so a `REQ_TPU` step means "any TPU (≥1 chip)".
   Identical to Rust for every requirement except a hypothetical `Tpu { min_chips > 1 }`, which the
   flattened repr cannot express in the first place.
2. **Requirement token is `requirement_name(req)`** (e.g. `"gpu"`, `"tpu"`) rather than the Rust
   `{:?}` Debug spelling (`"Gpu"`, `"Tpu { min_chips: 1 }"`). The full message is otherwise
   identical: `hardware unavailable: step '<name>' requires <req> but no matching device found`. The
   `hardware.rs` test only asserts the message contains `"hardware unavailable"` + the step name, so
   it ports faithfully; the token casing is cosmetic and the flattened repr can't reproduce Debug.

**Why accepted:** forced by the row-3 decision to store `hardware` as a single `REQ_*` i64 (which
matches ai-hwaccel's own `requirement_satisfied(req, min_chips, …)` ABI). The check's observable
verdict (which steps pass/fail) is identical for every requirement szal can represent. See
`src/engine_hardware.cyr` and `docs/development/issues/2026-06-11-registry-new-collision.md` (the
`registry_new` collision that gated this module, resolved by the bote 2.7.5 re-sync).

---

## 12. base64 decode: no error/UTF-8 signalling (`mcp_tools_encoding.cyr`, szal_base64)

**What:** Rust's `szal_base64` decode path returns `McpErrorCode::Internal` for (a) invalid base64
(`STANDARD.decode` error) and (b) decoded bytes that are not valid UTF-8 (`String::from_utf8`).
bayan's `base64_decode` does neither — it returns a best-effort `{ptr, len}` byte buffer for any
input and never validates UTF-8.

**Divergence:** the two `MCP_INTERNAL` error branches are unreachable in the port — malformed input
yields a (possibly garbage) success result rather than a typed error. The **encode** path and the
**valid round-trip decode** path are byte-exact (`encode("hello world") == "aGVsbG8gd29ybGQ="` and
back), which is what the Rust `base64_encode_decode` test exercises.

**Why accepted:** bayan is the stdlib base64 (no decode-error/validation API to thread through), and
the only Rust test covers the valid round trip, which passes unchanged. Hardening this to reject
malformed input is a roadmap follow-up if a consumer needs it. See `src/mcp_tools_encoding.cyr`.

---

## 13. sha256 file hashing capped at 1 MiB (`mcp_tools_hash.cyr`, szal_sha256)

**What:** Rust's `szal_sha256` with a `file` arg reads the whole file uncapped (`tokio::fs::read`).
The port reads through a single fixed `HASH_FILE_CAP` (1 MiB) buffer via `file_read_all`.

**Divergence:** a file larger than 1 MiB hashes only its first 1 MiB (rather than the full contents).
String hashing (`input`) is unaffected and exact; the Rust `sha256_file` test uses a 12-byte file.

**Why accepted:** a fixed buffer is the simplest correct read in Cyrius (no streaming digest wired
yet) and a 1 MiB cap matches szal's file-tool read-cap security posture. Streaming the digest
(`sha256_init`/`_update` over chunks) to remove the cap is a roadmap follow-up. See
`src/mcp_tools_hash.cyr`.

---

## 14. system_info: hardcoded os/arch + second-precision timestamp (`mcp_tools_system.cyr`)

**What:** Rust's `szal_system_info` reports `std::env::consts::OS` / `ARCH` (compile-time per build
target). The port hardcodes `"linux"` / `"x86_64"`. `szal_timestamp`'s `iso8601` field is
second-precision UTC (`"…Z"`) rather than Rust's sub-second `to_rfc3339()` (timezone-offset form).

**Divergences:**
1. **os/arch are literals** matching what the Rust consts resolve to on szal's 2.0.0 target
   (linux/x86_64). A cross-target build would report the wrong values — there is no stdlib
   uname/arch helper to derive them at runtime yet.
2. **timestamp is second-precision UTC** — the standing `chrono.iso8601` limitation (it cannot
   render or round-trip sub-second / offset RFC3339). `unix_secs`/`unix_ms` are exact.

**Why accepted:** szal 2.0.0 targets linux/x86_64 (the `cyrius build … [x86_64]` target), so the
literals are correct for every supported build; the `system_tools.rs` tests assert only that the
`os`/`arch`/`iso8601` fields are present (not their values). Runtime os/arch detection and sub-second
timestamps are roadmap follow-ups. See `src/mcp_tools_system.cyr`.

---

## 15. json_validate error detail: byte position, not line/column (`mcp_tools_json.cyr`)

**What:** Rust's `szal_json_validate` returns `{valid:false, error, line, column}` for malformed
input (serde's `Error::line()`/`column()`). bayan's parser exposes a message + a single byte
**position** (`_json_err_pos`), not line/column.

**Divergence:** the invalid-JSON result carries `{valid:false, error, position}` (byte offset) instead
of `{error, line, column}`. The valid-JSON result (`{valid, type, size_bytes}`) is exact.

**Why accepted:** bayan is the stdlib JSON parser and its error API is position-based; deriving
line/column would mean re-scanning the input for newlines. The `json_validate_bad` test only asserts
`"valid": false`, which holds. (`szal_json_diff` uses a faithful structural `_json_eq` — order-
independent objects, INT≠FLOAT like serde — so diff parity is exact.) See `src/mcp_tools_json.cyr`.

---

## 16. szal_wc file counting capped at 1 MiB (`mcp_tools_template.cyr`)

**What:** `szal_wc` with a `file` arg reads through a fixed `WC_FILE_CAP` (1 MiB) buffer; Rust reads
the file uncapped (`tokio::fs::read_to_string`). Same shape as the sha256 file cap (§13).

**Divergence:** files over 1 MiB are counted only up to the first 1 MiB. Text-field counting
(lines/words/chars/bytes) is exact — `lines` via `.lines()` semantics (trailing newline adds no empty
line), `words` via whitespace runs, `chars` via UTF-8 leading-byte count, `bytes` via length.

**Why accepted:** same rationale as §13 (fixed buffer + read-cap posture). See `src/mcp_tools_template.cyr`.

---

## 17. base_convert uses i64, not u128 (`mcp_tools_conversion.cyr`, szal_base_convert)

**What:** Rust parses/formats via `u128::from_str_radix`. Cyrius has no 128-bit integer (no bigint
dependency wired), so the port parses and formats with i64.

**Divergence:** values requiring more than 63 bits overflow/wrap rather than converting exactly — the
same `u64→i64` width family as §1, extended to the u128 here. Bases (2/8/10/16), prefix stripping
(0x/0b/0o), digit validation, and lowercase output all match Rust for any value that fits in i64.

**Why accepted:** szal has no bigint dep and 64 bits covers realistic base-conversion inputs; the
`conversion_tools.rs` tests use small values (255, 0xFF, 1010). `szal_byte_format` (f64 via
`fmt_float_buf`, 2-decimal rounding) and `szal_duration_format` are exact. See `src/mcp_tools_conversion.cyr`.

---

## 18. math_eval non-integer formatting (`mcp_tools_math.cyr`, szal_math_eval)

**What:** `szal_math_eval` formats an integer-valued result as i64 (exact, matching Rust's
`val as i64` branch). A non-integer result is formatted with `fmt_float_buf` (10 decimals) +
trailing-zero strip, whereas Rust uses `format!("{val}")` (the shortest round-trip f64 Display).

**Divergence:** fractional results may render with slightly different precision/length than Rust
(e.g. a repeating decimal truncates at 10 places). The integer path is exact. The recursive-descent
evaluator itself (precedence, parens, unary minus, `+ - * / %`, div/mod-by-zero errors, the char
allowlist that blocks injection) is faithful. `^` is in the tool *description* but Rust never
implemented it (not in the char allowlist nor tokenizer) — the port omits it identically.

**Why accepted:** Cyrius has no shortest-round-trip f64 formatter; the `math_tools.rs` tests use
integer results (5, 53), which are exact. See `src/mcp_tools_math.cyr`.

---

### Disposition log

- **2026-06-11 — M1 foundation parity audit** (7 modules vs `rust-old`: error/state/migration/bus/
  flow/step/condition, 12-auditor adversarial sweep + skeptic verify): 5 modules clean;
  **2 real divergences found and FIXED Cyrius-side** (oracle untouched) — (1) `state.cyr`: Rust
  `WorkflowState` serde emits PascalCase (`"RollingBack"`), distinct from Display snake_case → added
  `state_json_name`/`state_from_json`; (2) `bus.cyr`: `duration_ms` `Some(0)` rendered `null` → added
  the `WE_DURATION_SET` presence flag. (Predates §1–8; these were the only non-idiom findings all port.)
- **2026-06-11 — M2 result-parity audit** (`engine_result.cyr` + `step.cyr` StepResult deser):
  9 "confirmed" findings, all resolved as **accepted idioms already covered by §1 (u64→i64) and
  §3 (lenient deser)** — documented here, no behavioral change. The audit's suggested fixes that
  proposed editing `rust-old/` (adding `#[serde(default)]` to the oracle) were rejected: parity is
  achieved by matching Rust, not by mutating the frozen oracle. A flagged `vec_new()` OOM gap in
  `flow_result_new` was left as-is to match the house convention (`flow_new`/`step_new` likewise do
  not guard `vec_new()`; Rust `Vec::new()` never fails, so it is not a parity concern).
- **2026-06-11 — M2 storage parity audit** (`storage.cyr` vs `storage.rs`): 3 lenses
  (WorkflowStorage / ExecutionStore / ExecutionRecord serde), auditors primed with this file's
  accepted-idiom list. **0 findings** — full behavioral + serde parity; the pointer-return (§6)
  and deferred-lock (§7) divergences were correctly classified as accepted, not regressions.
- **2026-06-11 — M2 engine_core parity audit** (`engine_core.cyr` vs `engine/mod.rs`): 3 lenses
  (config+context types / handler ABI + emit / check_condition). **0 findings** — field-for-field
  parity. The (fn_ptr, ctx) callback pairs replacing `Arc<dyn Fn → BoxFuture>` (synchronous; no
  async — port-plan §1.7) and the opaque i64 slots for not-yet-ported modules (hardware/heartbeat/
  queue) were correctly classified as accepted design, not divergences.
- **2026-06-11 — M2 engine_step_exec parity audit** (`engine_step_exec.cyr` vs `engine/step_exec.rs`):
  3 lenses (retry/attempts/backoff / failure error messages / timeout + event-metric set).
  **0 findings** — exact parity incl. max_attempts, per-attempt vs total duration, the RetryExhausted
  vs last-error selection, the exact Display strings, and the dual `step_failed` on a final-attempt
  timeout. The worker-thread timeout (§8) was correctly classified as accepted, not a divergence.
- **2026-06-11 — M2 engine_sequential parity audit** (`engine_sequential.cyr` vs `sequential.rs`):
  single-lens (skip order/reasons/shape + condition-Err-runs + failure cascade). **0 findings** —
  exact skip-check order and reason strings, condition parse-error runs (not skips), failed cascade.
- **2026-06-11 — M2 engine_hierarchical parity audit** (`engine_hierarchical.cyr` vs `hierarchical.rs`):
  single-lens (pre-order results / subtree skip propagation / `parent step failed` vs `prior step
  failed` / condition-Err-runs). **0 findings** — faithful recursive tree walk; plain recursion
  replacing `Box::pin` futures is an accepted idiom with identical observable output.
- **2026-06-11 — M2 engine_parallel parity audit** (`engine_parallel.cyr` vs `parallel.rs`): 2 lenses
  (result order + condition pre-pass / semaphore + join + cancel-timeout). **0 findings** — exact
  result order (pre_skipped-first ++ spawn-order), condition-against-pre_skipped, permit floor of 1,
  spawn-order join, skip reasons. The thread/semaphore/worker-slot/orphan idioms (§8) were correctly
  classified as accepted with identical observable output.
- **2026-06-11 — M2 engine_dag parity audit** (`engine_dag.cyr` vs `dag.rs`): 2 lenses (Kahn
  bookkeeping: in-degree/dependents/unlock + re-queue prevention / wave exec + transitive failure +
  skips). **0 findings** — exact in-degree formula, MAX-sentinel double-run prevention, fixed-batch
  waves, `dependency failed` transitive skips, cancel/timeout-stops-with-locked-steps-unrun. The
  ordinal-arena-over-HashMap idiom (CLAUDE.md) was correctly classified as accepted.
- **2026-06-11 — M2 engine_queue_runner parity audit** (`engine_queue_runner.cyr` vs
  `queue_runner.rs`): single-lens (enqueue/dequeue/complete-fail loop + drain exit). **0 findings** —
  Normal-priority enqueue, complete-vs-fail on Completed, one-result-per-step exit. The dropped
  ResourcePool arg (port-plan §3.2) and item-keyed mq_complete/mq_fail were correctly classified as accepted.
- **2026-06-13 — M2 engine_hardware (row 17) self-parity** (`engine_hardware.cyr` vs `hardware.rs`,
  branch-for-branch, oracle read-only): faithful — `detect`/`with_ttl`/`registry`/`check_requirements`/
  `effective_concurrency`/Debug map 1:1 onto ai-hwaccel `cached_registry_new(300)`/`cached_get`/
  `count_satisfying`/`reg_count_by_family`/`reg_has_accelerator`. Two accepted divergences recorded in
  §11 above (flattened `min_chips`=1; `requirement_name` token casing). Wired into `engine_runner` at
  all three entry points (`_engine_check_hardware` after validate, no-op when `config.hardware==0`),
  mirroring the Rust `#[cfg(feature = "hardware")]` blocks. The gating `registry_new` collision (Q9)
  was resolved by the bote 2.7.5 re-sync (`registry_new`→`tool_registry_new`) — see the issue file.
