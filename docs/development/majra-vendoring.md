# Full majra vendoring — ✅ DONE (2026-06-11), majra 2.4.6 at `src/vendor/majra.cyr`

> **Status: COMPLETE.** The full majra dist is vendored at `src/vendor/majra.cyr` (3,131 lines,
> 9-symbol collision rename applied via `scripts/sync-majra.sh`), included in `main.cyr`, and the
> whole suite is green (726 assertions, 0 duplicate-symbol warnings). The interim metrics shim
> (`src/vendor/majra_metrics.cyr`) has been **retired** and `metrics.cyr` repointed at the full dist.
>
> This doc is kept as the **maintenance record** for re-syncing majra and for the collision
> rationale. The "blockers" below are annotated with how they actually resolved.
>
> **The §3 bigint blocker was a FALSE ALARM:** the core `dist/majra.cyr` references **zero**
> `bigint`/`tls`/`sandhi`/`patra` symbols (those were over-listed in majra's cyml hint, used only
> by its *other* bundles). The only stdlib addition the full dist needed was **`lib/thread.cyr`**
> (chan_/mutex_); everything else was already in `main.cyr`.

## 1. Why it's required (what needs full majra)

szal uses majra for: `mq_*` (ManagedQueue → row 18 queue_runner), `fleet_*` (FleetQueue → row 19
distributed), `pubsub_*` (EventBus → row 6 bus's deferred majra path + M3 stream `ProgressHub`),
`ratelimit_*` (M3 mcp_pool), `chb_*` (heartbeat → row 20 runner heartbeat guard), and the 22-slot
metrics vtable (row 10 — shipped now via the shim). All but metrics need the **full** dist.

## 2. The pin

- **majra 2.4.5**, `dist/majra.cyr` (85,031 bytes), synced from a majra checkout's `dist/`.
- Lands at `src/vendor/majra.cyr` (hoosh vendor pattern — a `[deps.majra]` block would make
  `cyrius deps` recurse into majra's own git sub-deps; see `cyrius.cyml`).

## 3. BLOCKER — `lib/bigint.cyr` is missing from the lib snapshot

`cyrius lib sync` provisioned 89 modules; **`bigint` is not among them**. The full majra dist
references bigint (via its `tls` module), so a `--strict` build fails with an undefined function
until this is resolved. **Resolve one of:**
- re-run `cyrius lib sync` on a toolchain whose snapshot includes `bigint` (preferred — verify the
  6.1.x snapshot actually ships it), OR
- confirm whether the majra subset szal actually links (mq/fleet/pubsub/ratelimit/heartbeat/metrics)
  pulls `tls`/`bigint` at all; if tls is dead code for szal, a stripped vendoring (§5b) avoids it, OR
- vendor `bigint` explicitly (least preferred — off-pattern vs the lib-sync-managed `lib/`).

Do NOT start the full vendoring until this is decided.

## 4. Symbol collisions (9) — majra bundles its own workflow/error surface that overlaps szal

Cyrius duplicate-symbol semantics = **last definition wins** (+ warning for fns; enum-const dupes
can hard-error). Verified clashes between `dist/majra.cyr` and szal's `src/*.cyr`:

| majra symbol | kind | szal owner | clash detail |
|---|---|---|---|
| `ERR_NONE` | enum const (=0) | `SzalErr` ERR_NONE (=0) | same value but duplicate def |
| `ERR_QUEUE` | enum const (=1) | `SzalErr` ERR_QUEUE (=9) | **different value → silent corruption** |
| `STEP_COMPLETED` | enum const | bus `EventType` STEP_COMPLETED (=5) | different value |
| `STEP_FAILED` | enum const | bus `EventType` STEP_FAILED (=6) | different value |
| `STEP_SKIPPED` | enum const | bus `EventType` STEP_SKIPPED (=9) | different value |
| `TRIGGER_ALL` | enum const | step `TriggerMode` | different value |
| `TRIGGER_ANY` | enum const | step `TriggerMode` | different value |
| `uuid_generate` | fn `()` | `src/uuid.cyr` (returns hi,lo) | **fn-name clash** |
| `step_result_new` | fn `(status,output,error_msg)` | `src/step.cyr` `(hi,lo,status,output,duration_ms,attempts)` | **fn-name clash, different arity** |

Full families to rename (all majra-owned, verified — no stdlib `ERR_*`/`STEP_*`/`TRIGGER_*` refs):
majra `ERR_*` (20 consts), `STEP_*` (5: COMPLETED/FAILED/PENDING/RUNNING/SKIPPED), `TRIGGER_*` (2).

## 5. Resolution — rename in the vendored copy ONLY (user-approved 2026-06-11)

Renames apply **only to `src/vendor/majra.cyr`**, never to szal's `src/*.cyr`. Because szal never
passes its own `STEP_*`/`TRIGGER_*`/`ERR_*` values into majra's workflow surface (szal implements
its own engine — port-plan "do NOT wrap majra's dag/workflow"), renaming majra's copies is safe;
blanket-rename-within-the-file keeps majra internally consistent (def + all internal call sites).

### 5a. The sed recipe (this is the body of `scripts/sync-majra.sh`)

```sh
# from a majra checkout: copy dist + apply collision renames into src/vendor/majra.cyr
sed -E \
  -e 's/\bERR_/MJ_ERR_/g' \
  -e 's/\bSTEP_/MJ_STEP_/g' \
  -e 's/\bTRIGGER_/MJ_TRIGGER_/g' \
  -e 's/\buuid_generate\b/majra_uuid_generate/g' \
  -e 's/\bstep_result_new\b/majra_step_result_new/g' \
  "$MAJRA/dist/majra.cyr" > src/vendor/majra.cyr
```

`\bERR_` etc. are word-boundary anchored so they only hit identifier starts (idempotent: `MJ_ERR_`
won't re-match). Verified: the 9 collisions are the *complete* set (full `comm` symbol-intersection
of majra fns/consts/vars vs all szal modules — 234 fns / 57 consts / 29 vars on the majra side).
**Re-run the `comm` check after any majra version bump** — a new majra release may add new clashes.

### 5b. Alternative considered: strip majra's workflow/dag module

majra's `uuid_generate`/`step_result_new`/`STEP_*`/`TRIGGER_*` live in its bundled workflow/dag
module, which is **dead code for szal**. Stripping that module (by `# --- <module>.cyr ---` marker
boundaries) would remove most collisions at the source and cut bloat, but risks removing something
mq/fleet transitively need. Renaming (5a) is lower-risk and was the chosen approach.

## 6. Interim metrics-only shim — RETIRED (was row 10 → superseded same day)

A metrics-only shim (`src/vendor/majra_metrics.cyr`, just majra's self-contained `metrics.cyr`
module) was shipped first to unblock `metrics.cyr` while the full dist was thought blocked. Once
the bigint "blocker" proved false (§intro), the full dist was vendored and the shim **deleted**;
`metrics.cyr` was repointed at `src/vendor/majra.cyr` (identical `METRICS_VTABLE_SIZE`/
`noop_metrics`/`metrics_workflow_*` surface — it was a true drop-in, nothing downstream changed).
`scripts/sync-majra-metrics.sh` was also removed, superseded by `scripts/sync-majra.sh`.

## 7. Stdlib footprint actually needed (much smaller than the cyml hint implied)

The full `dist/majra.cyr` only needed **one** stdlib addition to `main.cyr`: **`lib/thread.cyr`**
(provides `chan_*` + `mutex_*`; everything else — string/fmt/alloc/freelist/vec/str/hashmap/
syscalls/tagged/result/fnptr/io/chrono — was already included). It references **none** of
`bigint`/`tls`/`sandhi`/`patra`/`net`/`fs`/`bench`. Note: `lib/sync.cyr` also defines `mutex_*`,
so include `thread` but **not** `sync` (else last-wins duplicate warnings on `mutex_new/lock/unlock`).

## 8. Acceptance checklist — ✅ all done (2026-06-11)

- [x] `lib/bigint.cyr` — N/A (core dist doesn't reference it; §intro).
- [x] `scripts/sync-majra.sh` written (§5a) + committed; produces `src/vendor/majra.cyr` (3,131 lines).
- [x] Re-ran the 9-collision `comm` check vs majra 2.4.6 — same 9 symbols, no NEW clashes.
- [x] `main.cyr` includes `lib/thread.cyr` + `src/vendor/majra.cyr` in single-pass order.
- [x] `cyrius build --strict` clean — full main (all szal modules + full majra) has **0 undefined
      fns, 0 duplicate-symbol warnings**; `./build/szal` runs; 726 assertions green.
- [x] Deleted `src/vendor/majra_metrics.cyr` + `scripts/sync-majra-metrics.sh`; repointed `metrics.cyr`.
- [ ] **TODO (M5):** CHANGELOG 2.0.0 — note majra 2.4.6 vendored + the `MJ_`/`majra_` rename of its
      bundled workflow surface. Also: `cyrius.cyml` pin says 2.4.5 but the checkout/vendor is 2.4.6 —
      reconcile the pin at release (dist byte-identical, so functionally equal).
