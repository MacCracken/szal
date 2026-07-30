# Full majra vendoring — ✅ DONE (2026-06-11), majra 2.5.3 at `src/vendor/majra.cyr`

> **Status: COMPLETE.** The full majra dist is vendored at `src/vendor/majra.cyr` (3,289 lines,
> collision rename applied via `scripts/sync-majra.sh`), included in `main.cyr`, and the
> whole suite is green (1,434 assertions, 0 duplicate-symbol warnings). The interim metrics shim
> (`src/vendor/majra_metrics.cyr`) has been **retired** and `metrics.cyr` repointed at the full dist.
>
> **Re-synced 2.4.6 → 2.5.3 at szal 2.1.0 (2026-07-29).** Collision scan re-run over the full
> fn/const/var surface: **no new clashes**, and the set shrank **9 → 7** because szal prefixed its
> own error codes `SZAL_ERR_*` in the same release, so majra's bare `ERR_NONE`/`ERR_QUEUE` stopped
> colliding with szal entirely (see §4). The `MJ_ERR_` rename is kept anyway — it still isolates
> majra's 20 bare `ERR_*` from bote / ai-hwaccel / stdlib.
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

- **majra 2.5.3**, `dist/majra.cyr` (93,035 bytes upstream-dist source; the renamed
  `src/vendor/majra.cyr` is ~93.6 KB with its provenance header), synced from a majra checkout's `dist/`.
  (The old 2.4.5-vs-2.4.6 pin skew noted here is resolved: `cyrius.cyml` and this doc both read 2.5.3.)
- Lands at `src/vendor/majra.cyr` (hoosh vendor pattern — a `[deps.majra]` block would make
  `cyrius deps` recurse into majra's own git sub-deps; see `cyrius.cyml`).

## 3. ~~BLOCKER~~ — `lib/bigint.cyr` (RESOLVED: false alarm)

> **RESOLVED 2026-06-11:** the core `dist/majra.cyr` references **no** `bigint` symbol — this was
> never a real blocker (see the status banner). The options below are kept as the historical record.

`cyrius lib sync` provisioned 89 modules; `bigint` is not among them. It was *thought* the full majra
dist needed bigint (via its `tls` module); in fact the core dist references none of `bigint`/`tls`.
Had it been real, the resolution options were:
- re-run `cyrius lib sync` on a toolchain whose snapshot includes `bigint` (preferred — verify the
  6.1.x snapshot actually ships it), OR
- confirm whether the majra subset szal actually links (mq/fleet/pubsub/ratelimit/heartbeat/metrics)
  pulls `tls`/`bigint` at all; if tls is dead code for szal, a stripped vendoring (§5b) avoids it, OR
- vendor `bigint` explicitly (least preferred — off-pattern vs the lib-sync-managed `lib/`).

Do NOT start the full vendoring until this is decided.

## 4. Symbol collisions (7 as of majra 2.5.3 / szal 2.1.0; was 9)

Cyrius duplicate-symbol semantics = **last definition wins** (+ warning for fns; enum-const dupes
can hard-error). Verified clashes between `dist/majra.cyr` and szal's `src/*.cyr`:

**Retired from this table at szal 2.1.0** — szal renamed *its* side, so these two are no longer
clashes at all (the `MJ_ERR_` rename is still applied, to keep majra's 20 bare `ERR_*` off
bote / ai-hwaccel / stdlib):

| was | why it's gone |
|---|---|
| `ERR_NONE` (=0) vs szal `ERR_NONE` (=0) | szal's is now `SZAL_ERR_NONE` |
| `ERR_QUEUE` (=1) vs szal `ERR_QUEUE` (=9) — **the value-divergence one** | szal's is now `SZAL_ERR_QUEUE` |

The 7 that remain:

| majra symbol | kind | szal owner | clash detail |
|---|---|---|---|
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
won't re-match). Verified: the collisions listed in §4 are the *complete* set (full `comm`
symbol-intersection of majra fns/consts/vars vs all szal modules — 234 fns / 57 consts / 29 vars on
the majra side, unchanged from 2.4.6 to 2.5.3).
**Re-run the `comm` check after any majra version bump** — a new majra release may add new clashes.

The scan is only trustworthy if it covers all three symbol kinds *and* compares them across kinds:
Cyrius resolves fns, enum constants and globals in ONE flat namespace, so a `var X` in one file
really does collide with an `enum { X = .. }` in another. A same-kind-only scan misses those — that
is exactly how ai-hwaccel's `var BYTES_PER_GB = 1000000000` sat against szal's
`enum { BYTES_PER_GB = 1073741824 }` (a **value divergence**, silently resolved by include order)
until the 2.1.0 sweep caught it. Also scan against the **stdlib** modules `main.cyr` includes, not
just szal + the other vendored dists.

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
- [x] `scripts/sync-majra.sh` written (§5a) + committed; produces `src/vendor/majra.cyr`
      (3,131 lines at 2.4.6; **3,289 at 2.5.3**).
- [x] Re-ran the collision `comm` check vs majra 2.4.6 — same 9 symbols, no NEW clashes.
- [x] `main.cyr` includes `lib/thread.cyr` + `src/vendor/majra.cyr` in single-pass order.
- [x] `cyrius build --strict` clean — full main (all szal modules + full majra) has **0 undefined
      fns, 0 duplicate-symbol warnings**; `./build/szal` runs; 882 assertions green.
- [x] Deleted `src/vendor/majra_metrics.cyr` + `scripts/sync-majra-metrics.sh`; repointed `metrics.cyr`.
- [x] **CHANGELOG + pin reconcile — done at 2.1.0** (was "TODO (M5)"): the `cyrius.cyml` 2.4.5-vs-2.4.6
      skew is gone (both now read **2.5.3**), and CHANGELOG 2.1.0 records the vendored bump plus the
      `MJ_`/`majra_` rename of majra's bundled workflow surface.

### 8a. 2.5.3 re-sync (szal 2.1.0, 2026-07-29)

- [x] `scripts/sync-majra.sh ../majra` → `src/vendor/majra.cyr`, 3,289 lines, from majra 2.5.3.
- [x] Collision scan re-run across **all three symbol kinds and cross-kind** (§5a note), over
      szal × majra × bote × ai-hwaccel × the 29 stdlib modules `main.cyr` includes: **no new
      clashes**; majra's own set shrank 9 → 7 (§4). majra's surface is unchanged in size
      (234 fns / 57 consts / 29 vars, same as 2.4.6).
- [x] majra 2.5.3 still ships its `ERR_*`/`STEP_*`/`TRIGGER_*` families **unprefixed**, and contains
      zero pre-existing `MJ_` symbols — so the sed recipe applies cleanly and stays idempotent.
- [x] `cyrius build --strict --no-deps src/main.cyr` clean; `./build/szal` runs; **45/45 test files,
      1,434 assertions, 0 failures**; lint/fmt/doc clean; `rust-old/` oracle pristine.
