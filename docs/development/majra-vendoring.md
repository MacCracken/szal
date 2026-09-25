# Vendored libraries — majra 2.9.1, bote-core 3.3.13, ai-hwaccel 2.4.0 at `src/vendor/`

> **Status at szal 2.2.0 (2026-09-25): all three vendored BYTE-FOR-BYTE from their release tags.**
> `scripts/sync-{majra,bote,ai-hwaccel}.sh` extract `dist/<lib>.cyr` with `git show <tag>:…`, check
> the dist's own `# Version:` line against the tag, and copy it unmodified — the same files hoosh
> commits (`cmp` clean). szal no longer renames anything inside a vendored copy: it renamed its OWN
> colliding symbols instead ([ADR 0002](../adr/0002-szal-owns-its-namespace.md), §10 below), because
> a consumer that vendors upstream majra / bote-core could not link szal's code next to them.
> `scripts/scan-collisions.sh` reports one intersection anywhere in szal's build — `REQ_NONE`, shared
> with ai-hwaccel on purpose and allow-listed only while both values agree.
>
> The file keeps its old name and its history. §3–§8b describe the rename-the-copy era
> (2.0.0–2.1.2); read them as history, not procedure. **Procedure now:** run the sync script, then
> `scripts/scan-collisions.sh --check`, `scripts/consumer-check.sh` (and `../hoosh` if present), and
> the full suite; if a new upstream name collides, rename szal's side (ADR 0002) — never the copy.
>
> _Earlier status (2.1.1):_ the full majra dist is vendored at `src/vendor/majra.cyr` (then 4,840
> lines, collision renames applied via `scripts/sync-majra.sh`). The interim metrics shim
> (`src/vendor/majra_metrics.cyr`) was **retired** and `metrics.cyr` repointed at the full dist.
> **The §3 bigint blocker was a FALSE ALARM:** the core `dist/majra.cyr` references **zero**
> `bigint`/`tls`/`sandhi`/`patra` symbols. The only stdlib addition the full dist needed was
> **`lib/thread.cyr`** (chan_/mutex_).

## 1. Why it's required (what needs full majra)

szal uses majra for: `mq_*` (ManagedQueue → row 18 queue_runner), `fleet_*` (FleetQueue → row 19
distributed), `pubsub_*` (EventBus → row 6 bus's deferred majra path + M3 stream `ProgressHub`),
`ratelimit_*` (M3 mcp_pool), `chb_*` (heartbeat → row 20 runner heartbeat guard), and the 22-slot
metrics vtable (row 10 — shipped now via the shim). All but metrics need the **full** dist.

## 2. The pins

| file | release | lines | sync |
|---|---|---|---|
| `src/vendor/majra.cyr` | majra **2.9.1** `dist/majra.cyr` (the base bundle) | 5,984 | `scripts/sync-majra.sh` |
| `src/vendor/bote-core.cyr` | bote **3.3.13** `dist/bote-core.cyr` (`[lib.core]`) | 2,905 | `scripts/sync-bote.sh` |
| `src/vendor/ai-hwaccel.cyr` | ai-hwaccel **2.4.0** `dist/ai-hwaccel.cyr` | 7,514 | `scripts/sync-ai-hwaccel.sh` |

- The **base** `dist/majra.cyr` is still the right cut. majra also ships `majra-admin.cyr`,
  `majra-signed.cyr` and `majra-backends.cyr`; the base bundle is a strict subset of all three, and
  szal uses nothing from the modules they add.
- Vendored, not `[deps.X]` blocks: a dep block makes `cyrius deps` recurse into each library's own
  git sub-deps (lib/ bloat, collisions), and zero git deps keeps CI at `cyrius lib sync` + `--no-deps`.

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

## 4. Symbol collisions (7 as of majra 2.5.3 / szal 2.1.0; was 9) — HISTORY

> **Superseded at 2.2.0:** every szal-side name in the table below was renamed on szal's side
> (§10), and majra is vendored unmodified.

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

## 5. Resolution — rename in the vendored copy ONLY (user-approved 2026-06-11) — HISTORY

> **Reversed at 2.2.0** ([ADR 0002](../adr/0002-szal-owns-its-namespace.md)): the recipe below
> kept szal's build clean but made szal's code unlinkable next to upstream majra, which is what
> every consumer vendors.

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

### 8b. 2.7.0 re-sync (szal 2.1.1, 2026-08-26)

- [x] `scripts/sync-majra.sh ../majra` → `src/vendor/majra.cyr`, **4,840 lines**, from majra 2.7.0.
- [x] **All 25 majra symbols szal links verified signature-identical** at 2.7.0 (`ratelimit_new/
      _check`, `mq_new/_enqueue/_dequeue/_complete/_fail/_queued_count/_running_count`,
      `queue_item_payload`, `fleet_config_new/_new/_register_node/_node_queue/_submit/_rebalance`,
      `chb_register/_deregister`, `noop_metrics`, the 5 `metrics_workflow_*`, `PRIORITY_NORMAL`,
      `METRICS_VTABLE_SIZE`). Nothing renamed, nothing removed, no arg-order or wire-shape change on
      szal's surface. Several majra struct sizes grew (`TokenBucket` 16→32, `RateLimiter` 48→56,
      `chb_tracker` 24→32, `Subscriber` 16→40) — **safe, because szal does zero offset arithmetic on
      any majra object**; it stores handles and passes them straight back.
- [x] Collision scan re-run with the new **cross-kind** scanner, `scripts/scan-collisions.sh`
      (szal × majra × bote × ai-hwaccel × the provisioned stdlib, all symbol kinds, name-based so
      kinds cross). **Zero new same-kind clashes**; majra × bote and majra × ai-hwaccel are both 0.
- [x] **NEW rename rule 6 — `SYS_GETRANDOM` → `MJ_SYS_GETRANDOM`.** This is the one the old
      same-kind scan could never have caught, and it is the same bug shape as `BYTES_PER_GB` (§5a).
      majra declares `var SYS_GETRANDOM = 318` — an **x86_64-hardcoded** literal. The stdlib declares
      the same name as an **arch-conditional enum constant**: 318 on x86_64-linux and macos, **278 on
      aarch64-linux**, **45 on agnos**. `main.cyr` includes `lib/syscalls.cyr` *before*
      `src/vendor/majra.cyr`, so last-definition-wins gave majra's 318 to the entire program —
      including `lib/patra.cyr:640` and `lib/sigil.cyr`, both of which szal reaches through
      `sql_store.cyr`. It is invisible to cycc twice over: it is cross-kind (`var` vs enum constant),
      and the two values are *identical* on the CI arch, and cycc is silent on both. Renaming leaves
      majra using its own 318 internally and hands the arch-correct constant back to the stdlib.
      **majra's hardcoded 318 is an upstream bug worth reporting** (`dist/majra.cyr:205`;
      `SYS_CLOCK_GETTIME = 228` next to it is x86-only too, but collides with nothing so no scan
      sees it).
- [x] **`MJ_ERR_` kept despite rescuing zero live collisions.** It is dead *today* only because szal
      self-prefixed `SZAL_ERR_*` at 2.1.0 and majra's 20 bare `ERR_*` happen not to overlap the 17
      the stdlib closure owns (`ERR_OK`, `ERR_NOT_FOUND`, `ERR_TIMEOUT`, `ERR_INVALID`, `ERR_BUSY`,
      `ERR_OVERFLOW`, `ERR_PERMISSION`, `ERR_UNKNOWN`, `ERR_CAT_*`). That is a coincidence of naming,
      not a structural guarantee — one future majra `ERR_TIMEOUT` or `ERR_NOT_FOUND` (obvious names
      for a queue engine) would silently retarget stdlib call sites. Cost: 20 identifiers, zero risk.
- [x] Rename safety re-verified on 2.7.0: **zero** pre-existing `MJ_` / `majra_uuid_generate` /
      `majra_step_result_new` tokens upstream (so the sed stays idempotent), and **zero** rewrites
      land inside string literals — majra's JSON wire names (`"workflow_step"`, `"ipc_json"`,
      `"heartbeat"`, …) are untouched.
- [x] **`StepStatus` / `TriggerMode` deliberately NOT renamed.** They are enum *type* names colliding
      with `src/step.cyr:26,35`. Cyrius does not put enum type names in the flat symbol table
      (verified: two same-named integer enums keep all their distinct member values, and the compiler
      is silent), and neither name is used as a type annotation anywhere in szal. They are
      allow-listed in `scripts/scan-collisions.sh` so the next scan does not re-litigate them.
- [x] **Correction to §4:** `TRIGGER_ALL` / `TRIGGER_ANY` are **0/1 on both sides** (majra
      `dist/majra.cyr:4372-4373`, szal `src/step.cyr:27-28`) — name clashes with *identical* values,
      not "different value" as the §4 table says. Only the three `STEP_*` genuinely diverge
      (`STEP_COMPLETED` 2 vs 5, `STEP_FAILED` 3 vs 6, `STEP_SKIPPED` 4 vs 9).
- [x] **Correction to `src/bus.cyr:4-5`:** it says majra "is not vendored", which is stale — it is.
      The rest of that comment is right: szal calls **zero** `pubsub_*` symbols anywhere, so
      `bus.cyr` is not a majra consumer.
- [x] `cyrius build --strict --no-deps src/main.cyr` clean, **0 duplicate symbols**; `./build/szal`
      runs; **46/46 test files, 1,437 assertions, 0 failures**; 5/5 fuzz harnesses; bench harness
      builds; lint clean; fmt clean; `rust-old/` oracle pristine.

### 9. majra 2.7.0 closes parity-notes §9 (flagged, NOT implemented)

`docs/development/parity-notes.md` §9 records szal's one streaming divergence: Rust's
`tokio::broadcast` drops the oldest event and reports `RecvError::Lagged` when a subscriber falls
behind, whereas Cyrius has only a blocking `chan_send`, so szal's `ProgressHub` **blocks the
producer** when any subscriber's channel is full. majra 2.7.0 ships exactly the missing primitive —
a per-subscriber lag policy (`PUBSUB_LAG_BLOCK` / `PUBSUB_LAG_DROP_NEWEST` / `PUBSUB_LAG_DROP_OLDEST`)
plus pubsub unsubscribe. Adopting it would let `src/stream.cyr` match tokio's semantics and retire
the divergence. **Deliberately not done at 2.1.1** — that is a behavioural port change, not a
maintenance bump, and it belongs in its own change with its own tests.

## 10. 2.2.0 re-sync — the renames move to szal's side (2026-09-25)

- [x] **Pins:** majra 2.7.0 → **2.9.1**, bote-core 3.3.7 → **3.3.13**, ai-hwaccel 2.3.19 → **2.4.0**,
      each the latest release. Every fn szal calls is signature-identical (24 majra, 13 bote, 6
      ai-hwaccel, measured over src/, tests, fuzz and benches); the `REQ_*` / `FAMILY_*` value table
      is byte-identical.
- [x] **Sync from the tag.** The old scripts copied the checkout's working tree and labelled it
      with its `VERSION`. At this sync ai-hwaccel's checkout sat one unreleased commit past 2.4.0
      (comments only), and that commit had been vendored as "2.4.0". The scripts now take
      `git show <tag>:dist/<lib>.cyr`, refuse a missing tag, and check the dist's `# Version:` line.
- [x] **No renames in the copies.** szal renamed its own side (ADR 0002): the `EventType` members
      → `SZAL_FLOW_*` / `SZAL_STEP_*`; `SzalTriggerMode` / `SZAL_TRIGGER_*`; `SzalStepStatus` (so the
      two enum TYPE names no longer need the allow-list); `szal_step_result_new`;
      `szal_uuid_generate`; `szal_condition_*` for the compiled-condition family bote's
      `compiled_compile` clashed with. Each rename was made while the vendored copies were still
      renamed, so any missed reference failed as an undefined symbol instead of silently binding
      to the upstream name.
- [x] **`MJ_SYS_GETRANDOM` needed no successor**: majra 2.7.3 moved `uuid_generate` to
      `sys_getrandom`, and the dist declares no `SYS_GETRANDOM` at all.
- [x] **`MJ_ERR_*` needed no successor either**: majra 2.8.0 prefixed its own codes
      `MAJRA_ERR_*` (keeping bare `ERR_*` as aliases until 3.0.0); the scan finds no collision
      between those aliases and szal, bote, ai-hwaccel or the stdlib.
- [x] **Scan:** one intersection in szal's build (`REQ_NONE`, equal values). The scanner also
      gained an intra-szal pass — which found `TOK_LPAREN` / `TOK_RPAREN` declared by both
      `condition.cyr` and `mcp_tools_math.cyr` with different values, a live parser bug (see
      CHANGELOG 2.2.0) — and a `--consumer` mode for `dist/szal-mcp.cyr`.
- [x] `cyrius build --strict --no-deps src/main.cyr` clean; 47/47 test files, 1,494 assertions;
      5/5 fuzz; benchmarks; lint; fmt; `rust-old/` untouched.
- [ ] Still open from §9: majra's `PUBSUB_LAG_*` policy could retire parity-notes §9 — a behavioural
      port change for its own release.
