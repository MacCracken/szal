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

---

### Disposition log

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
