# szal Rust → Cyrius — AUTHORITATIVE PORTING BRIEF

Merged from 7 research briefs (lang-core, lang-latest, vidya, stdlib, deps, szal, template),
2026-06-11. **Every inter-brief conflict was re-verified against primary sources on this
machine** (cyrius repo, vidya field notes, majra/bote/ai-hwaccel/patra/szal repos); resolutions
are marked `[VERIFIED]` with the source. This is the single input for the porting engineers.

Facts of record (M0 snapshot — **live toolchain/dep pins now live in `state.md` + `cyrius.cyml`**;
this table is the as-written-at-M0 record):

| Thing | Value | Source |
|---|---|---|
> **This table is the LIVE version snapshot — refreshed at 2.1.0 (2026-07-29).** Everything below
> it (§dep mapping, the `cyrius.cyml` sketch, the per-module rows) is the **M0 design record** and
> deliberately still quotes the M0-era pins; read it for *intent*, and take current pins from here,
> `cyrius.cyml`, and [`state.md`](state.md).

| Thing | Value | Source |
|---|---|---|
| Toolchain | Cyrius **6.1.33** at M0 → now pinned **6.5.35** | `cyrius.cyml`, `state.md` |
| szal (Rust) | **1.2.0**, 13,127–13,172 LOC, 42 src files, AGPL-3.0-only in Cargo.toml | `/home/macro/Repos/szal` |
| szal (Cyrius port) | **2.1.1**, 11643 LOC across 44 `src/*.cyr` (excl. `src/vendor/`) | `VERSION`, `src/` |
| bote (Cyrius) | **3.3.7** (vendored `src/vendor/bote-core.cyr`; 2.7.3 at M0) | `/home/macro/Repos/bote` |
| majra (Cyrius) | **2.7.0** (vendored `src/vendor/majra.cyr`; 2.4.5 at M0) | `/home/macro/Repos/majra`, `majra-vendoring.md` |
| ai-hwaccel (Cyrius) | **2.3.19** (vendored `src/vendor/ai-hwaccel.cyr`; 2.3.9 at M0) | `/home/macro/Repos/ai-hwaccel` |
| patra (SQL) | **1.12.12**, consumed via the cyrius stdlib as `lib/patra.cyr` | `/home/macro/Repos/patra` |
| Stdlib | **101** modules in the 6.5.35 snapshot; `cyrius lib sync` provisions the **55** named in `[deps].stdlib` (json/toml/csv/base64/bigint/u128/matrix/linalg carved out → bayan/ganita) | `ls ~/.cyrius/versions/6.5.35/lib/*.cyr` |
| Git deps | **none** — all three third-party libs are vendored under `src/vendor/`, so no `cyrius deps` / `cyrius.lock` | `cyrius.cyml` |

Primary references for the engineers:
- `/home/macro/Repos/cyrius/docs/guides/cyrius-guide.md` (language), `docs/stdlib-reference.md`, `docs/api-surface.snapshot` (canonical names/arity), `docs/guides/faq.md`
- `/home/macro/Repos/vidya/content/cyrius/**` — field notes (gotchas); runnable exemplars under `content/{error_handling,state_machines,iterators,...}/cyrius.cyr`
- **Best porting model**: `/home/macro/Repos/majra` (finished Rust→Cyrius port, contains a DAG workflow engine — `src/dag.cyr`); `/home/macro/Repos/bote/DEPS-PATTERN.md` (distribution contract); `/home/macro/Repos/hoosh` (vendoring pattern)

---

## 1. Cyrius cheat-sheet for Rust engineers (v6.1.33 nomenclature ONLY)

Cyrius is a sovereign self-hosting systems language. No borrow checker, no ownership, no
type checking, no monomorphized generics, no unwinding, no GC. **Everything is an i64**
(ADR-002): pointers, strings, enum values, fn pointers, bools (1/0). Source is `.cyr`,
comments are `#` (no `//`, no block comments; avoid `->` ASCII art in comments).

### 1.1 Toolchain names (current era — stale names are BUILD-BREAKING)

- Compiler: **`cycc`** (was `cc5` — dead since v6.1.1). Bootstrap: **`cybs`** (was `cyrc` — dead).
- CLI wrapper: **`cyrius`** — `build / run / test / tests / bench / fuzz / soak / smoke /
  fmt / lint / doc [--check] / vet / deny / audit / capacity [--check] / distlib / deps
  [--lock|--verify] / lib sync / init / port / coverage / doctest / repl / lsp`.
- **NEVER `cat file.cyr | cycc`** for project code — always `cyrius build` (resolves deps).
  Use `cyrius build --no-deps` once `cyrius lib sync` has provisioned `./lib/` (6.x model).
- `--strict` makes undefined-fn references hard errors (default: warning + `ud2` patch →
  SIGILL exit 132 at runtime). Use `--strict` in CI.
- Cross targets: `--aarch64 / --win / --agnos / --pie / --target=js`. Exactly one of
  `CYRIUS_TARGET_LINUX/_WIN/_MACOS/_AGNOS` is defined per build.

### 1.2 Quick Rust → Cyrius syntax table

| Rust | Cyrius v6.1.33 |
|---|---|
| `let x = 42;` / `let mut` | `var x = 42;` (8-byte i64; everything mutable) |
| `let buf = [0u8; 256];` | `var buf[256];` — **256 BYTES; STATIC data section even inside a fn** (§1.4) |
| `struct P { x: i64, y: i64 }` | `struct P { x; y; }` (fields = 8-byte slots; per-field `i8/i16/i32` widths pack with NO auto-padding) |
| `enum E { A, B }` | `enum E { A; B; }` → `E.A == 0`; namespaced `E.A` works everywhere |
| `Result<T,E>` / `?` | compiler-generated `Ok(v)`/`Err(e)` + postfix `?`; helpers in `lib/result.cyr` |
| `Option<T>` | `Some(v)`/`None()` + `lib/tagged.cyr` (`is_some/unwrap/unwrap_or`); or `0 = None, ptr = Some` for pointer-shaped values |
| `Vec<i64>` | `lib/vec.cyr`: `vec_new/push/pop/get/set/len/find/remove` (elements always i64 — store pointers) |
| `HashMap` | `lib/hashmap.cyr`: `map_new()` (cstr keys) / `map_new_str()` (Str keys) / `map_u64_new()` (u64 keys); `map_set/get/has/delete/keys`; `map_count` and `map_size` are both present `[VERIFIED lib/hashmap.cyr]` |
| `String`/`&str` | cstr (NUL-terminated literal) vs `Str` fat pointer `{data@0, len@8}` (`lib/str.cyr`) — see §1.5 |
| `match` | `match v { VARIANT => { } _ => { } }` (enum exhaustiveness = warning); `switch (n) { case <int literal>: ... default: }` |
| closure | `\|x\| expr` exists but **captures NOTHING** — named fn + explicit context-struct pointer |
| `fn(i64)->i64` ptr | `var fp = &my_fn;` + `callptr(fp, a, …)` (v6.0.70 builtin, preferred) or `fncall0..8` (`lib/fnptr.cyr`) |
| `if let` / try-catch | do not exist; no try/catch EVER (design decision) |
| `i64::wrapping/saturating/checked_add` | `a +% b` / `a +\| b` / `a +? b` (checked panics → exit 57) |
| `#[must_use]` / `#[deprecated]` | `#must_use` / `#deprecated("msg")` on the line above `fn` |
| `unsafe {}` | `@unsafe { }` (advisory) |
| `#[derive(Serialize)]` | `#derive(Serialize)` → generates `<T>_to_json(p, sb)` / `<T>_from_json_str(cstr)`; deserialize half needs `include "lib/bayan.cyr"` |
| `cargo build/test/bench` | `cyrius build/test/bench` |

### 1.3 Program shape, functions, control flow

Top-level statements ARE the program (after enum init, then global-var init in declaration
order). Canonical entry (use everywhere):

```cyrius
fn main() {
    alloc_init();        # REQUIRED before any alloc/vec/str/map/Ok()/Some() use; idempotent since v6.1.23
    # ...
    return 0;
}
var r = main();
syscall(60, r);          # tests/Linux convention; portable code uses sys_exit(r)
```

- Every fn returns a value; no void. Bare `return;` **works everywhere since v5.10.48**
  `[VERIFIED cyrius CHANGELOG 5.10.48 — it synthesizes return 0]`; convention still prefers
  explicit `return 0;`. (Older field notes saying bare-return-in-if is rejected are STALE.)
- ≤6 params in registers (7+ ride the stack; extern-C fn-pointer calls cap at 6). Design APIs ≤6 args.
- Multi-return: `return (a, b);` → `var q, r = f(...);` (rax:rdx).
- **Do NOT return a struct literal / `&local_struct`** — heap-allocate (`alloc` + stores) and return the pointer. This is the stdlib-wide idiom.
- `if (…) {} elif (…) {} else {}`; `while (…)`; `for (var i = 0; i < n; i += 1)` — all three
  clauses required, no `i++`; `for i in 0..n` (exclusive); `break`/`continue`.
- `switch` cases: integer **literals** only, no fallthrough.
- Mixed `&&`/`||` REQUIRES explicit parens. `^` binds tighter than binary `-`. `>>` is
  **LOGICAL** (zero-fill). `/`/`%` are signed idiv. No unary `~` (`x ^ (0 - 1)`).
- **No negative literals** — write `(0 - 5)` `[VERIFIED faq.md limitation 7; the vidya brief's
  "negative literals are supported" is WRONG]`. Enum initializers take no arithmetic and no
  negatives — negative codes are `var` constants.
- `defer { … }` — LIFO at fn exit; cap **64 defer/secret blocks per fn** `[VERIFIED
  src/frontend/parse.cyr: "max 64"; the "max 8" note is stale]`. For szal rollback use an
  explicit stack of (fn_ptr, ctx) pairs, not defer.
- Scoping is function-level: duplicate `var` name in one fn = compile error. Unique local names per fn.

### 1.4 Memory model — the #1 porting bug class

1. **`var buf[N]` = N BYTES, not N elements** (`var state[25]` for 25 u64 lanes = corruption;
   write `var state[200]`). Pointer tables: `var argv[8*count]`.
2. **`var buf[N]` is STATIC data in the binary, even inside a function** — every call shares
   the same buffer; returned `Str`s dangle on the next call `[VERIFIED vidya
   semantics_runtime.cyml entry var_buf_in_library_functions; the "stack at fn scope" claim in
   the vidya brief is WRONG]`. Per-call buffers must be `alloc(N)`.
3. `N` must be an integer literal **or an enum constant** (since v5.10.48 — `enum Sz { BUF = 16; }
   var buf[BUF];` works; `var SIZE = 16; var buf[SIZE];` still rejected) `[VERIFIED CHANGELOG 5.10.48]`.
4. Primitive API: `store8/16/32/64(addr, v)` / `load8/16/32/64(addr)`; `&x`, `*p`; typed
   pointers `var p: *i64 = &buf;` scale arithmetic by element size. NULL is `0`.
5. Allocators:
   - `lib/alloc.cyr` global **bump** allocator: `alloc_init()` once; `alloc(n)` (returns **0 on
     OOM — check it**; next store segfaults, exit 139); **nothing is ever freed** until
     `alloc_reset()` (wipes everything). Thread-safe (CAS spinlock) since v6.0.64. mmap
     chunk-backed since v6.1.19.
   - Arenas: `arena_new(cap)` / `arena_alloc(a, n)` / `arena_reset(a)`; allocator interface
     `alloc_via(a, n)` / `reset_via(a)`; `_a`-suffixed stdlib variants (`vec_new_a(a)`,
     `str_from_a(cstr, a)`, `bayan_json_v_obj_new_a(a)`, …) thread an allocator through.
   - `lib/freelist.cyr`: `fl_init()` / `fl_alloc(n)` / `fl_free(p)` / `fl_calloc(n)` — O(1)
     free/reuse for churn objects.
   - **szal is a long-lived engine — naive `alloc()` WILL leak.** Directive: per-flow-run arena
     (`reset_via` after the run, if nothing outlives it) for execution scratch; `fl_alloc/fl_free`
     for queued events / per-step results that have churn; global bump only for process-lifetime
     objects (registries, flow definitions).

### 1.5 Strings — cstr vs Str (the #1 "garbage output" bug)

- **cstr** = NUL-terminated pointer (what `"literals"` are). `lib/string.cyr`: `strlen, streq,
  memcpy, memset, memeq, memchr, strchr, println, print_num`.
- **Str** = heap fat pointer `{data@0, len@8}`. `lib/str.cyr`: `str_from(cstr), str_new(data,len),
  str_len/str_data (or s.len/s.data with `: Str` annotation), str_eq/2, str_eq_cstr, str_cat,
  str_sub (SHARES data), str_clone, str_split(s, sep_byte)→vec, str_join, str_trim, str_contains,
  str_starts_with/ends_with, str_index_of, str_from_int, str_to_int, str_from_buf`.
- Builder (the `write!`/`format!` replacement — use for ALL string assembly):
  `str_builder_new()` + `str_builder_add(sb, str)` / `_add_cstr` / `_add_int` / `_add_byte` /
  `_add_json_str` + `str_builder_build(sb) → Str`.
- Gotchas: `println(a_Str)` prints garbage (use `str_println`); `str_data(s)` is **NOT
  NUL-terminated**; cstr-keyed APIs (`map_new()` maps, `toml_get`, process argv `exec_vec`)
  silently fail on Str input. **Project convention: Str everywhere internally; cstr literals
  for fixed keys; convert explicitly at boundaries.**
- Slices: `lib/slice.cyr` `{ptr,len}` view, bounds-checked `s[i]` (violation → exit 134).
  Str / vec-header / slice are byte-identical.

### 1.6 Structs, enums, sum types, errors

- THE data-modeling idiom for anything with identity/lifetime (StepDef, FlowDef, results,
  records): **heap blob + enum layout constants + accessor fns**:

```cyrius
enum StepDefL { STEPDEF_SIZE = 64; STEPDEF_ID_HI = 0; STEPDEF_ID_LO = 8; STEPDEF_NAME = 16; ... }
fn stepdef_new(name) { var s = alloc(STEPDEF_SIZE); if (s == 0) { return 0; } ...; return s; }
fn stepdef_name(s) { return load64(s + STEPDEF_NAME); }
```
  Document the byte layout in a comment above every struct (majra convention). Enum layout
  constants cost zero global-var slots — **the initialized-global cap is ~64** `[VERIFIED
  faq.md; the "1024" figure is wrong]` — put ALL constants in enums.
- Dispatch sugar: `fn Counter_inc(self)` → callable as `c.inc()` (ADR-004 name mangling);
  `: TypeName` annotations enable dot syntax on heap pointers (`s.len` on a `: Str`).
  `#derive(accessors)` generates `Type_field(p)/Type_set_field(p, v)`.
- Tagged unions (paren'd enum variants) heap-allocate: `enum Event { StepDone(id, code); }` →
  `alloc(8 + 8*N)`, tag@+0, payload[i]@+8+8i. **Match on `load64(v)` (the tag), never on `v`
  (the pointer).** Don't mix bare and paren'd variants in one decl you match on. Variant names
  share ONE global namespace — prefix them (`SzalStepFailed`, like stdlib `IoNotFound`).
- **Result is heap-tagged: tag@+0 (0=Ok, 1=Err), payload@+8, 16-byte alloc** `[VERIFIED
  lib/result.cyr header — the "packed bit-63 zero-alloc Result" claim in the vidya brief is
  WRONG]`. Constructors call `alloc()` → heap must be initialized first.
  Helpers: `is_ok, is_err_result, result_unwrap (aborts on Err), result_unwrap_or, err_code_of`.
- `?` operator: `var x = f(a)?;` — Err early-returns the Result pointer from the enclosing fn;
  Ok unwraps payload. Only inside fn bodies; only use in fns that themselves return Ok/Err.
- Stdlib error pattern to mirror: `_r`-suffixed Result-returning fns (`file_open_r`) + per-module
  typed error enums (`IoError { IoNotFound; ... }`). Err payload is ONE i64 → szal's rich error
  variants flatten to **code + per-result error-message Str + global last-error detail buffer**
  (majra `src/error.cyr` pattern; the global buffer is NOT thread-safe — carry error text in the
  per-step result struct on threaded paths).

### 1.7 Concurrency (no async/await — that is v6.3.x FUTURE)

- **OS threads** (`lib/thread.cyr`, Linux clone+futex): `thread_create(fp, arg) → t/0`,
  `thread_join(t)`, `gettid()`. 64 KB stack/thread. No macOS threads; Windows = thread_win.
- Mutex: `mutex_new/lock/unlock` (`lib/thread.cyr`; portable version in `lib/sync.cyr` v6.1.16).
- Channels (bounded MPSC, futex-blocking): `chan_new(cap)` `[VERIFIED — capacity arg required]`,
  `chan_send(ch, v)` (-1 if closed), `chan_recv(ch)` (blocking; 0 if closed),
  `chan_try_recv(ch)` (0 if empty — avoid 0 payloads), `chan_close(ch)`.
- Atomics (`lib/atomic.cyr`): `atomic_load/store/cas/fetch_add/fence`.
- Cancellation (exact CancellationToken match, poll-based): `cancel_token_new()`,
  `cancel_token_signal(tok)`, `cancel_token_check(tok)`. No hierarchy, no await — workers poll.
- Cooperative epoll runtime `lib/async.cyr`: `async_new()/async_new_in(arena_allocator(cap))`,
  `async_spawn(rt, fp, arg)`, `async_run(rt)` (**single-use** — closes epfd; recreate per batch),
  `async_sleep_ms`, `async_await_readable`. Tasks are run-to-completion fn pointers, NOT futures.
- **`async_timeout(fp, arg, ms)` FORKS a child process** — side effects are lost. Wrong for
  szal step timeouts (steps mutate shared state). Use: worker thread + `chan_try_recv` polling +
  `clock_now_ms()` deadline + cancel token.
- Time (`lib/chrono.cyr`): `clock_now_ns/ms` (monotonic), `clock_epoch_ns/secs` (wall),
  `iso8601(epoch)→cstr` (second precision, always Z), `iso8601_now`, `iso8601_parse`,
  `dur_*`, `sleep_ms(ms)` (portable — never raw `syscall(35, …)`).

### 1.8 Modules, includes, manifest

- `include "lib/foo.cyr"` is **textual paste**, include-once by filename, single-pass —
  definitions must precede first use across the concatenated unit (forward fn-to-fn calls are
  fine; the hazard is global initializers and CO-01: call-before-definition of Str/struct-ret
  fns can miscompile — keep non-plain-i64 signatures defined before first call).
- **Absolute and parent-traversal includes are REJECTED since v6.1.33** — all includes
  project-relative.
- No namespaces: flat files + name prefixes (`szal_*`, `_private`). The `mod`/`use` keywords in
  the frozen migration-strategy doc are NOT the current mechanism — ignore that doc.
- 6.x dependency model `[VERIFIED majra cyrius.cyml comment]`: `cyrius lib sync` provisions the
  pinned ~94-file stdlib snapshot into `./lib/`; `cyrius deps` overlays `[deps.<name>]` git/path
  bundles (+ writes `cyrius.lock`, sha256, committed); build with `cyrius build --no-deps`.
  The `[deps] stdlib = [...]` list is a human-readable record + auto-prepend list — do NOT list
  carved names (`json`, `bigint`, `toml`, `base64` are GONE from lib/) or non-resolvable
  internals (`slice`, `ct`).
- Carve renames (v6.1.25/26) — stale names are build-breaking:
  `json_*` → `bayan_json_*`, `toml_*` → `bayan_toml_*`, `csv` → `bayan_csv_*`,
  `base64_encode` → `bayan_base64_encode` (legacy aliases linger but are deprecated),
  `matrix/linalg/advanced-math` → `ganita_*`. JSON/TOML/CSV/base64 all come from ONE opt-in
  `include "lib/bayan.cyr"` (consumers also need `result`/`fnptr`/`io` in scope).
- TLS (if net tools need it): native sovereign TLS is the **no-flag default** (v6.1.21);
  `-D CYRIUS_TLS_NATIVE` is a deprecated no-op; `-D CYRIUS_TLS_LIBSSL` opts out.
- Reserved words that bite Rust ports: `secret stack union defer sizeof default match in shared`
  — rename colliding fields/params (the `secret` param trap produces a confusing error in the
  NEXT include).
- Capacity: 8192 fns, 256 KB idents, 16 MB output (v6.1.27). `cyrius capacity --check` in CI.

### 1.9 Tests / benches / fuzz / docs (locations are ENFORCED — wrong dir = silently ignored)

```
tests/tcyr/*.tcyr    unit (cyrius test / tests)      benches/*.bcyr   (cyrius bench)
tests/scyr/*.scyr    soak (cyrius soak)              fuzz/*.fcyr      (cyrius fuzz)
tests/smcyr/*.smcyr  smoke (cyrius smoke)
```
*(Siblings also use flat `tests/<name>.tcyr` driven by `cyrius test <file>` + a `tests/test.sh`
runner — majra's pattern. Either works; auto-discovery needs `tests/tcyr/`.)*

`.tcyr` shape: includes → `alloc_init();` → `test_group("...")` → `assert/assert_eq/assert_neq/
assert_gt/assert_streq/assert_nonnull(…, "name")` → `var r = assert_summary();` (returns FAIL
count = exit code) → `syscall(60, r);`. Table-driven: `lib/test.cyr::test_each(cases_vec, fp)`.
OOM-injection: `fail_after_n_allocs(n)`.

`.bcyr`: `bench_new(name)` → `bench_run(b, &fp, n)` or batch
`bench_batch_start(b)` / tight loop / `bench_batch_stop(b, batch_size)` `[VERIFIED lib/bench.cyr
— the "bench_batch_start(name, iters)" signature in the vidya brief is WRONG]` →
`bench_report(b)` / `bench_report_all(vec)`; readers `bench_avg_ns/min/max/iterations`.
~240 ns clock overhead/op → batch ≥1000 iters for sub-µs ops.

`.fcyr`: deterministic stress scripts; exit nonzero with a distinct code per failure mode.
Add algebraic property tests, not just crash tests.

Doc comments: `#` lines directly above a fn; `cyrius doc --check` fails on any public
(non-`_`-prefixed) fn without one — CI gate. Module headers:
`# name.cyr — purpose` / `# Usage: include …` / `# Requires: …`.

Lint taboos: camelCase fns, tabs, >120-col lines, global-init forward refs (silently read 0).

---

## 2. Rust-dep → Cyrius mapping table

| szal Rust dep | Cyrius replacement | Concrete API |
|---|---|---|
| `serde`+`serde_json` | `#derive(Serialize)` for simple structs; **bayan value-tree** for `serde_json::Value` | `include "lib/bayan.cyr"`: `bayan_json_v_parse(Str)→json_v/0`, `_v_tag`, `_v_is_obj/arr/str/int/...`, `_v_int/_v_str/_v_bool`, `_v_obj_get(v, cstr_key)`, `_v_arr_len/_v_arr_get`, `_v_pointer` (RFC 6901); build: `_v_obj_new/_v_obj_set/_v_arr_new/_v_arr_push/_v_int_new/_v_str_new(Str)`, `bayan_json_v_build(v)→Str`, `_build_pretty`; `bayan_json_last_error[_pos]`; `_a` arena variants for batch-free trees. Hot paths: build JSON directly with `str_builder_*` (bote `stream.cyr` pattern). NO derive attributes (`default`/`skip_serializing_if`/`rename_all`) — hand-write `*_to_json`/`*_from_json` per struct with explicit missing-field defaults (szal has ~11 serde types) |
| `thiserror` | int error-code enum + Result + detail buffer | `enum SzalErr { SZAL_ERR_NONE = 0; SZAL_ERR_STEP_FAILED = 1; ... }` + `szal_err_name(code)→cstr` + `set_err_msg/get_err_msg` global buffer (majra `src/error.cyr` is the template; buffer is last-error-wins, NOT thread-safe — per-result error Str on threaded paths) |
| `anyhow` | same machinery; `SZAL_ERR_OTHER` + detail string | optional richer: sakshi packed errors `sakshi_err_new(code, category)` / `_with_ctx` |
| `tracing` | **sakshi** (`lib/sakshi.cyr`) + `lib/log.cyr` wrapper | `sakshi_error/warn/info/debug/trace(msg, len)`, `sakshi_set_level`, spans `sakshi_span_enter(name, len)/sakshi_span_exit`, trace ids `sakshi_trace_set/_id`, output `sakshi_set_output_fd/output_file/output_buffer/set_emit_hook`; compile-time gate `#define SAKSHI_LEVEL n`. log.cyr: `log_init(LOG_INFO)`, `log_info_kv(msg, key, val)`. Multi-kv events → pre-format with str_builder. **HARD CONSTRAINT: sakshi is single-threaded** (header says so) — see Open Q5 |
| `tokio` rt/spawn | `lib/thread.cyr` threads (parallel/DAG stages) and/or `lib/async.cyr` epoll loop (I/O) | fan-out/join: ai-hwaccel `src/async_detect.cyr` (share-nothing per-thread arg struct, merge after join, sync fallback when `thread_create`→0). Tiered DAG + retry: majra `src/dag.cyr` |
| `tokio::sync::mpsc` | `chan_new(cap)/chan_send/chan_recv/chan_try_recv/chan_close` | bounded MPSC |
| `tokio::sync::broadcast` | majra pubsub | `pubsub_new/subscribe/subscribe_pattern/subscribe_filtered/publish` — per-subscriber `chan_new(64)`; **lag semantics differ** (blocks/drops vs tokio overwrite-oldest) — Open Q9 |
| `tokio::sync::Semaphore` | none — permit channel | `var sem = chan_new(N);` pre-fill N×`chan_send(sem,1)`; acquire=`chan_recv`, release=`chan_send`. Or bounded worker pool off one job chan (majra `mq_new(name, max_concurrent)` already does bounded concurrency) |
| `tokio_util CancellationToken` | `cancel_token_new/signal/check` (`lib/async.cyr`) | exact poll-based match; no child tokens — one token per flow run or vec of children |
| `tokio::time` sleep/timeout/interval | `sleep_ms(ms)`; deadline loops over `clock_now_ms()`; **NOT `async_timeout` (forks!)** | step timeout = worker thread + supervisor polling chan + deadline + cancel token |
| `tokio::select!` | none — restructure as polling loop | `chan_try_recv` + `cancel_token_check` + deadline checks |
| `tokio::fs` / `std::fs` | `lib/io.cyr` + `lib/fs.cyr` (synchronous) | prefer `_r` Results: `file_open_r/file_read_all_r/file_write_all_r`, `file_exists`, `file_lock/trylock/unlock`; `path_join/basename/dirname`, `dir_list`, `dir_walk[_with_prunes]`, `is_dir`, `find_files`. **No `canonicalize`** — szal's `validate_path` security boundary needs a manual component-walk normalizer (flagged; must not be dropped) |
| `tokio::io` std streams | fds 0/1/2: `sys_read(0, buf, len)`, `print/eprint(msg, len)` | MCP stdio JSON-RPC reference: `/home/macro/Repos/bote/src/transport_stdio.cyr` |
| `tokio::net` | `lib/net.cyr` (Result-returning, errno payloads) + `sandhi_resolve_ipv4/6(host)` DNS | `tcp_socket/sock_connect/bind/listen/accept/send/recv/close` |
| `tokio::process::Command` | `lib/process.cyr` | `run/run_capture/spawn/wait_pid` (≤2 args) or `exec_vec/exec_capture/exec_env` (cstr argv) / `exec_vec_str/...` (Str argv) — **never mix shapes** (silent rc=127); child env empty by default; exit 127 = exec failed; signaled → 128+sig |
| `uuid` v4 | none — copy majra's recipe | `/home/macro/Repos/majra/src/envelope.cyr:31` `uuid_generate()` (getrandom syscall, ret2 hi/lo). Store `{hi, lo}` two i64 fields. RFC-4122 string formatter (~20 lines, none in-tree): generate 16 bytes, patch `b[6]=(b[6]&0x0F)\|0x40; b[8]=(b[8]&0x3F)\|0x80`, hex-print in byte order — verify against szal's Rust test vectors |
| `chrono` | `lib/chrono.cyr` | `Utc::now()`→`clock_epoch_ns/secs`; `Instant`→`clock_now_ns/ms`; RFC3339→`iso8601(epoch)` (SECOND precision only, UTC only); `iso8601_parse`; `epoch_to_date` |
| `sha2` | `lib/sigil.cyr` (opt-in; pulls ct/keccak/random; needs bayan) | `sha256(data, len, out32)`, `sha256_hex(data, len)→cstr`, streaming `sha256_init/update/finalize` (fl_alloc'd ctx), `hex_encode`; also sha384/512, hmac_sha256, hkdf; SHA-NI path |
| `md-5` | **DOES NOT EXIST anywhere in Cyrius** | hand-port RFC 1321 (~100 lines) as `src/md5.cyr` modeled on `lib/sha1.cyr` → `md5(data, len, out16)` + `md5_hex` (see Open Q1 — recommended: port it) |
| `base64` | bayan | `bayan_base64_encode(buf, len)→cstr` (STANDARD, padded), `bayan_base64_decode(enc, len)→pair{ptr@0,len@8}`, `bayan_base64url_*`. **Decode does not strictly validate** — add a pre-validation pass to preserve szal's decode-error reporting |
| `sqlx` (sqlite) | **patra** (stdlib `lib/patra.cyr`, also repo v1.11.0) | `patra_init/open/close`, `patra_exec/query`, `patra_prepare/bind_int/bind_text/exec_prepared/query_prepared`, `patra_begin/commit/rollback`, `patra_result_count/get_int/get_str`, `patra_set_sync_mode/flush`; WAL + flock. szal's whole schema is one table → ports directly. JSONL alternative: `jsonl_append_obj/jsonl_read` |
| `sqlx` (postgres) | majra-backends `pg_*` (wire protocol v3) — costs the 137 KB bundle + sigil/ct/sandhi | recommend DROP for v2.0.0 (Open Q6) |
| dev `criterion` | `lib/bench.cyr` (§1.9) — keep the CSV history discipline | emit `timestamp,version,commit,benchmark,time_ns,unit` rows from bench output |
| dev `proptest` | none — seeded-random table tests via `random_bytes` (`lib/random.cyr`) or drop | port the 4 fuzz targets as `.fcyr` property harnesses |
| dev `tempfile` | `/tmp/<name>-<hex>` + `file_write_all_r`; unlink via `sys_unlinkat` | no auto-cleanup |

---

## 3. AGNOS dependency plan

### 3.1 The dist contract (non-negotiable — bote DEPS-PATTERN.md)

Every AGNOS Cyrius library ships committed, self-contained, include-free bundle(s) under
`dist/`, produced by `cyrius distlib` from `[lib]`/`[lib.<profile>]` module lists (order =
include order = single-pass dependency order). Consumers declare
`[deps.szal] git/tag/modules = ["dist/szal.cyr"]`; `cyrius deps` lands it as **`lib/szal.cyr`**
(basename starts with depname → kept as-is) and the consumer writes `include "lib/szal.cyr"`.
CI gates dist freshness (`cyrius distlib && git diff --exit-code dist/*.cyr`); every tag
commits regenerated bundles.

### 3.2 Per-dep decisions

| Dep | Rust | Cyrius | Bundle | Consumption |
|---|---|---|---|---|
| bote | 0.92 | **2.7.3** (done — dist committed) | `dist/bote-core.cyr` (70 KB, 9 transport-free modules; szal never used bote transports) | **VENDOR at `src/vendor/bote-core.cyr`** (hoosh pattern) — a `[deps.bote]` block makes `cyrius deps` recurse into bote's own `[deps.libro]`/`[deps.majra]` git blocks → lib/ bloat + symbol collisions. Re-sync script `scripts/sync-bote.sh <tag>` |
| majra | 1.0.4 (pubsub,queue,heartbeat,ratelimit + prometheus/barrier/dag/fleet passthroughs) | **2.4.5** | `dist/majra.cyr` core (85 KB) — ALREADY contains pubsub+queue+heartbeat+ratelimit+barrier+dag+fleet+metrics (no feature flags; bundle = feature set) | **VENDOR at `src/vendor/majra.cyr`** (same recursion rationale) |
| ai-hwaccel | 1.2 | **2.3.9** | `dist/ai-hwaccel.cyr` (200 KB, zero sub-deps) | normal `[deps.ai-hwaccel]` git+path+tag block → `lib/ai-hwaccel.cyr` |
| sqlx/sqlite | 0.9 | patra **1.11.0** — in the stdlib snapshot | stdlib `patra` (lib sync provides it) | no dep block needed |
| sqlx/postgres | 0.9 | majra-backends `pg_*` only | `dist/majra-backends.cyr` (137 KB + sigil/ct/sandhi) | recommend DROP in v2.0.0 |

Key majra surface mapping: `PubSub`→`pubsub_*`; `ManagedQueue`→`mq_new(name, max_concurrent)/
mq_enqueue(mq, priority, payload)/mq_dequeue(mq)/mq_complete/mq_fail` — **`ResourcePool` does
not exist in Cyrius majra; szal only ever passed an empty pool → drop the parameter, nothing
lost**; `ConcurrentHeartbeatTracker`→`chb_tracker_new(cfg)/chb_register/chb_heartbeat/
chb_update_statuses`; `RateLimiter`→`ratelimit_new(rate_x1000, burst)/ratelimit_check(rl, key)`
(fixed-point); `FleetQueue`→`fleet_config_new/fleet_new/fleet_register_node/fleet_submit/
fleet_rebalance`; `MajraMetrics`→22-slot fn-pointer vtable (`noop_metrics()`,
`metrics_workflow_run_started/completed/failed`, `metrics_workflow_step_started/finished`,
attach via `mq_with_metrics`) — **`prometheus` feature is GONE upstream**, replaced by the
vtable; szal's `WorkflowMetrics` becomes "hand the consumer this vtable".
majra's own `dag` module overlaps szal's engine — szal `grep` shows NO `majra::dag` usage in
Rust; do NOT wrap it, szal implements its own engine (borrow `_compute_tiers` patterns only).

bote surface mapping (from `src/registry.cyr` / `src/dispatch.cyr` / `src/audit.cyr` /
`src/events.cyr` / `src/schema.cyr`): `ToolSchema`→`schema_new`; `ToolDef`→`tool_def_new(name,
description, input_schema)` + `tool_def_with_*`; `ToolRegistry`→`registry_new/registry_register/
registry_get/registry_contains/registry_len`; `Dispatcher`→`dispatcher_new(reg)/
dispatcher_register_tool(d, def, handler_fp)/dispatcher_dispatch(d, request, claims)/
dispatcher_set_audit/dispatcher_set_events`; sinks→`audit_sink_new(log_fp, ctx)/audit_sink_noop`,
`event_sink_new(publish_fp, ctx)/event_sink_noop`, `tool_call_event_new`. Handler ABI:
`fn my_tool_handler(args_cstr, claims) → result_cstr`. Tool names must match `project_tool`
(≥1 underscore) — szal's 54 `szal_*` names port name-for-name. bote-core omits the majra event
bridge — copy the 6-line `majra_events_publish` adapter from bote's `src/events_majra.cyr`.

ai-hwaccel surface: `cached_registry_new(ttl_secs)/cached_get/cached_invalidate`;
`registry_detect[_no_exec/_with_opts]`; `REQ_NONE..REQ_ANY_ACCELERATOR` constants;
`requirement_satisfied/count_satisfying`; `FAMILY_*`/`family_name`; `reg_count_by_family`.
Near-1:1 for szal's `HardwareContext`.

### 3.3 KNOWN COLLISION (must resolve BEFORE porting)

**`registry_new` is defined by BOTH bote-core (24-byte tool registry) and ai-hwaccel (32-byte
profile registry).** Cyrius duplicate-fn semantics = last definition wins + warning. szal needs
BOTH surfaces, and ai-hwaccel's `registry_detect*` call `registry_new()` internally, so include
order cannot fix it. Options: (a) sed-rename in szal's vendored/synced copy
(`registry_new`→`hw_registry_new`), (b) upstream rename in ai-hwaccel 2.4.x (cleanest —
mihi/hoosh also benefit), (c) verified include-order trick (do NOT assume — test). → Open Q11.
Also: szal must not export bare `mcp_*` symbols (daimon defines its own `mcp_*` family) and
must prefix everything `szal_`/`flow_`/`step_` to stay out of majra's `ratelimit_*` namespace.

### 3.4 SQL store decision input

- Rust schema: `szal_executions(execution_id TEXT PK, flow_name TEXT NOT NULL, data TEXT NOT
  NULL)` — ExecutionRecord JSON in `data`. Ports 1:1 to patra.
- Rust 1.2.0's ordered-async-writer semantics (SpawnSink: in-memory read mirror + single
  ordered writer; a `Running` save never overwrites a later `Completed` save — ADR 0001) is the
  SPEC. Cyrius options: synchronous patra writes (simplest, ordering trivial) or one writer
  thread fed by a chan (preserves fire-and-forget). Recommend synchronous for v2.0.0.
- Bonus available: majra `patra_queue_*` = durable restart-surviving job queue if wanted later.
- Postgres → defer (Open Q6); document in CHANGELOG "Removed/Deferred".

### 3.5 Consumer expectations for `dist/szal.cyr`

- **daimon** (Cyrius 1.2.4): NO szal dep today; single-file monolith with its own `mcp_*` MCP
  types ("no bote dependency"). The port defines the contract fresh. Constraint: no bare
  `mcp_*` exports.
- **sutra**: not on disk, not in the shared-crates registry — planned consumer only. No
  constraint beyond the dist pattern.
- **AgnosAI**: registry lists Rust-era 1.1.0; consumes the Rust crate until its own port. No
  Cyrius contract yet.
- **secureyeoman** (stays Rust): pins `szal = "1.0"` — **the Rust szal repo/tags must remain
  intact** (archive via git tag `1.2.0`, don't break it).
- **samay** (planned scheduler, pre-1.0): future consumer of `dist/szal.cyr`.
- **zugot**: `marketplace/szal.cyml` is the Rust-era recipe — must be rewritten on the Cyrius
  shape (copy `marketplace/majra.cyml`).

### 3.6 Version & pin

Rust 1.2.0 → **Cyrius 2.0.0** (majra 1.0.4→2.0.0 is the exact post-1.0 precedent; vidya same;
bote's 0.x reset doesn't apply). `VERSION` file is the single source of truth;
`cyrius.cyml version = "${file:VERSION}"`. Pin `cyrius = "6.1.33"` (peers at 6.1.18–6.1.31 —
≥6.1.24 is ecosystem-consistent; 6.1.33 is the installed toolchain).

---

## 4. szal module port order (topological, single-pass-safe)

Two genuine Rust-module cycles, broken as follows:
- **storage ↔ engine**: `ExecutionRecord.result: Option<FlowResult>` vs
  `EngineConfig.storage/execution_store`. → port `engine/result` (deps: step only) BEFORE
  `storage`, `storage` before the rest of engine.
- **engine/mod ↔ engine/runner**: `sub_flow_handler` constructs `Engine`. → split: engine core
  types → executors → Engine → `sub_flow_handler` LAST.

| # | Cyrius file | Ports | Key API to preserve | Notes |
|---|---|---|---|---|
| 0 | `src/uuid.cyr` (new) | uuid crate usage | `uuid_generate()` (hi/lo via ret2 — copy majra envelope.cyr:31), `uuid_to_cstr(hi, lo)` RFC-4122 formatter, `uuid_parse` | needed by step/flow ids |
| 0b | `src/md5.cyr` (new, pending Q1) | md-5 crate | `md5(data, len, out16)`, `md5_hex` | RFC 1321, model on lib/sha1.cyr |
| 1 | `src/error.cyr` | error.rs (30 ln) | `enum SzalErr { SZAL_ERR_NONE=0; SZAL_ERR_STEP_FAILED; SZAL_ERR_STEP_TIMEOUT; SZAL_ERR_FLOW_INVALID; SZAL_ERR_RETRY_EXHAUSTED; SZAL_ERR_ROLLBACK_FAILED; SZAL_ERR_CYCLE; SZAL_ERR_MIGRATION; SZAL_ERR_HW_UNAVAILABLE; SZAL_ERR_QUEUE; SZAL_ERR_OTHER; }` + `szal_err_name(code)→cstr` + detail-msg buffer `set_err_msg/get_err_msg/clear_err_msg` | majra src/error.cyr is the template |
| 2 | `src/state.cyr` | state.rs (191 ln) | `enum WorkflowState` (8 states), `state_is_terminal(s)` (Completed/RolledBack/Cancelled), `state_valid_transition(from, to)` — exact edge list: Created→Running; Running→{Paused,Completed,Failed,Cancelled,RollingBack}; Paused→{Running,Cancelled}; Failed→RollingBack; RollingBack→{RolledBack,Failed}; `state_name(s)→cstr` snake_case | pure FSM |
| 3 | `src/step.cyr` | step.rs (330 ln) | layout-documented StepDef heap struct: id{hi,lo}, name Str, description Str, timeout_ms (default 30000), max_retries (0), retry_delay_ms (1000), backoff enum (Fixed/Linear/Exponential — `backoff_delay_ms(strategy, base, attempt)` with saturating math, overflow→i64 max), rollbackable, step_type Str/0, config json_v/0, condition Str/0, depends_on vec of id-pairs, trigger_mode (All/Any), sub_steps vec (recursive), hardware REQ_* i64 (default REQ_NONE); builder fns `step_new(name)`, `step_with_timeout/with_retries/with_backoff/with_rollback/step_depends_on/with_step_type/with_config/with_condition/with_trigger_mode/with_sub_step/with_hardware`; StepStatus enum (6); StepResult struct {step_id, status, output json_v, duration_ms, attempts, error Str/0}; `step_to_json/step_from_json` with serde-default semantics (missing backoff/trigger_mode/sub_steps must default, not fail) | |
| 4 | `src/condition.cyr` | condition.rs (1,270 ln) | tokenizer→recursive-descent parser→AST evaluator. Grammar: `\|\|` < `&&` < cmp(`== != > >= < <=`) < unary `!`/paths/literals/parens; single-quoted strings; dot-paths `[a-zA-Z_][a-zA-Z0-9_-]*`. `resolve_path(path, ctx_json_v)→json_v` (missing→null); `cond_evaluate(expr_str, ctx)→Result<bool, err>` (empty expr → true); `compiled_condition` (compile once, errors on trailing tokens, `evaluate(ctx)`); `condition_cache` (map expr→compiled, caches errors too, `cache_len`); `render_template(tpl, ctx)→Str` ({{dot.path}}; missing→"", string→raw, other→JSON); `build_step_context(results_vec, steps_vec)→json_v` ({"steps":{name:{status,output,error}}}) | largest pure-algorithmic unit; semantics: same-type-only equality, numeric compare as f64, truthiness rules; cache must be mutex-guarded if engine threads share it |
| 5 | `src/flow.cyr` | flow.rs (389 ln) | FlowDef heap struct {id, name, description, mode (Sequential/Parallel/Dag/Hierarchical), steps vec, rollback_on_failure, timeout_ms/0, version (default 1)}; `flow_new(name, mode)`, `flow_add_step`, `flow_with_version/with_rollback/with_timeout`; `flow_validate(f)→Result` — EXACT rules: Dag→DFS cycle check (`SZAL_ERR_CYCLE` w/ flow name) + unknown-dep check; non-Dag→any deps = invalid; Hierarchical→recursively no deps in sub-trees; TriggerMode Any with empty deps = invalid; `flow_to_json/from_json` | |
| 6 | `src/bus.cyr` | bus.rs (404 ln) | WorkflowEvent struct {event_type, flow_name/0, step_name/0, step_id/0, attempt, duration_ms, error/0, timestamp epoch_ns}; EventType enum (11, snake_case names); constructors `event_flow_started/...` (×11); `event_topic(e)→Str` ("szal/flow/{name}/{type}" / "szal/step/{name}/{type}", missing→"unknown"); `event_to_json`; EventBus over vendored majra pubsub (`bus_new/bus_publish/bus_subscribe(pattern)`); `otel_event_sink()` → sakshi/log mapping | timestamp: epoch-ns i64 (Open Q3) |
| 7 | `src/migration.cyr` | migration.rs (322 ln) | migration = {from, to, fn_ptr}; `fn_migration(from, to, fp)`; MigrationRegistry (map from-version→migration): `migration_register` (ABORT on to<=from or dup source — documented programmer-error abort, exit-code path), `latest_version`, `migrate_to(flow, target)` (no-op at target; downgrade/missing-path/overshoot → SZAL_ERR_MIGRATION; stamps flow.version per hop), `migrate_latest` | pure, sync |
| 8 | `src/engine_result.cyr` | engine/result.rs (61 ln) | FlowResult {flow_name, steps vec, total_duration_ms, success, rolled_back}; `flow_result_completed_count/failed_count/skipped_count`; `flow_result_to_json/from_json` | MUST precede storage |
| 9 | `src/storage.cyr` | storage.rs (330 ln) | WorkflowStorage as 3-slot vtable {get_by_name, get_by_id, list} + `in_memory_storage_new/insert/remove` (mutex-guarded map name→FlowDef); ExecutionRecord {execution_id Str, flow_name Str, state, result FlowResult/0, started_at, finished_at/0}; ExecutionStore as 4-slot vtable {save, get, list, remove} (SYNCHRONOUS by design — ADR 0001) + in-memory impl | dyn traits → fn-pointer vtables (lib/trait.cyr pattern or hand-rolled `vtable_new`) |
| 10 | `src/metrics.cyr` | metrics.rs (91 ln) | adopt majra's 22-slot metrics vtable directly; `metric_run_started/completed/failed`, `metric_step_started/finished` thin wrappers; `noop_metrics()` default | "WorkflowMetrics trait" = hand the consumer the vtable |
| 11 | `src/engine_core.cyr` | engine/mod.rs MINUS sub_flow_handler | FlowCtx {name, id}; ExecCtx {handler_fp+ctx, event_sink, flow, metrics_vt, step_type_metrics_fp, progress_sink, condition_cache}; `emit(sink, event)` no-op-if-0; `emit_step_type_metric` (None type→"default"); `check_condition(step, results, steps, cache)`; handler ABI: **StepHandler = `fn(step_ptr, ctx) → Result<json_v, err>` fn pointer + context ptr** (replaces `Arc<dyn Fn → BoxFuture>`); RollbackHandler likewise; StepProgress {step_name, step_id, data json_v} + ProgressSink + ProgressReporter; EngineConfig heap struct {max_concurrency (16), global_timeout_ms/0, storage_vt/0, hardware_ctx/0, metrics_vt/0, heartbeat/0, queue/0, step_type_metrics/0, progress_sink/0, execution_store_vt/0} | all former `Arc<dyn Fn>` extension points become (fn_ptr, ctx_ptr) pairs invoked via `callptr` |
| 12 | `src/engine_step_exec.cyr` | engine/step_exec.rs (180 ln) | `execute_step_with_handler(step, ctx)→StepResult` — EXACT semantics: max_attempts = retries+1; `step_started` emitted ONCE; per-attempt timeout (worker thread + deadline poll, NOT async_timeout); success → status Completed, attempts=attempt, duration=per-attempt; retry → emit `step_retry`, sleep `backoff_delay_ms`; exhausted → error = RetryExhausted-string if max_attempts>1 else last error, duration = TOTAL elapsed, output = null | |
| 13 | `src/engine_sequential.cyr` | engine/sequential.rs (98 ln) | in-order; skip reasons exactly: "cancelled" / "prior step failed" / "flow timeout exceeded" / "condition not met" (condition PARSE error → warn + run anyway); skips carry attempts=0, duration=0 | |
| 14 | `src/engine_parallel.cyr` | engine/parallel.rs (143 ln) | condition pre-pass (pre_skipped FIRST in result order); spawn per step bounded by permit-channel semaphore (max(1, max_concurrency)); join in spawn order; cancelled/timed-out at join → Skipped; worker crash → Failed "task panicked" | thread fan-out per ai-hwaccel async_detect.cyr; no abort() — cooperative cancel via token (semantic delta, Open Q2) |
| 15 | `src/engine_dag.cyr` | engine/dag.rs (234 ln) | Kahn wavefront: step_map, in_degree, dependents (TriggerMode Any → in_degree 1); ready queue seeded with no-dep steps; failure propagates transitively through `failed` set ("dependency failed" skips also enter `failed`); `unlock_dependents` (decrement; at 0 → push ready + set degree to sentinel max to prevent Any re-queue) — shared with distributed | CLAUDE.md "Vec arena over HashMap": use vec-indexed-by-ordinal arenas, id→ordinal map once |
| 16 | `src/engine_hierarchical.cyr` | engine/hierarchical.rs (117 ln) | recursive tree walk (plain recursion — no boxed futures needed); sequential siblings; success+sub_steps → recurse; failure → skip_children "parent step failed" + siblings skip; cancel/timeout skips subtree; flat pre-order results | |
| 17 | `src/engine_hardware.cyr` | engine/hardware.rs (170 ln) | HardwareContext over ai-hwaccel `cached_registry_new(300)`; `hw_check_requirements(steps)` (first unsatisfiable → SZAL_ERR_HW_UNAVAILABLE); `hw_effective_concurrency(steps, base)` (min 1; currently never called by engine — latent API, port anyway) | dep: ai-hwaccel bundle |
| 18 | `src/engine_queue_runner.cyr` | engine/queue_runner.rs (109 ln) | enqueue all (PRIORITY_NORMAL), single worker loop `mq_dequeue(mq)` (**no ResourcePool param — dropped**), execute, `mq_complete/mq_fail`, exit when queued+running == 0 | dep: vendored majra |
| 19 | `src/engine_distributed.cyr` | engine/distributed.rs (431 ln) | same DAG bookkeeping (reuses unlock_dependents); per-fleet-node worker threads looping {cancel check, `mq_dequeue`, execute, complete/fail, report via chan}; coordinator submits ready via `fleet_submit`, unlocks on completion, `fleet_rebalance()` per completion; un-run → Skipped "cancelled"/"flow timeout exceeded"/"not scheduled" | `select!{biased}` → poll loop |
| 20 | `src/engine_runner.cyr` | engine/runner.rs (746 ln) | Engine {config, handler, rollback_handler/0, event_sink/0, condition_cache}; builders `engine_new(config, handler...)` + `engine_with_rollback/storage/event_sink/event_bus/metrics/heartbeat/queue/execution_store/progress_sink/step_type_metrics`; `engine_run(e, flow)` EXACT sequence: validate → hw check → emit flow_started → save Running record (execution_id = flow id string, started_at) → metrics → heartbeat guard (register + 10s heartbeat thread; stop+deregister on exit — RAII → explicit stop call) → timeout = global \|\| flow \|\| max → queue path OR mode dispatch → has_failures → rollback_completed_steps (reverse order, rollbackable+Completed only, emit step_rollback each; no handler → false) → emit flow_rolled_back/flow_failed/flow_completed → build FlowResult → save final record → return; `engine_run_with_cancellation(e, flow, token)` (was_cancelled = signalled && any Skipped/"cancelled"; does NOT persist); `engine_run_distributed(e, flow, fleet)` (Dag-mode only; shares `finalize`) | the heart; ~47 behavioral tests define semantics |
| 21 | `src/engine_subflow.cyr` | sub_flow_handler (deferred from mod.rs) | intercepts step_type=="sub_flow": config.flow_name → storage lookup → run on FRESH default-config Engine with inner handler → FlowResult JSON; else delegate to inner | needs Engine — must be last engine file |
| 22 | `src/stream.cyr` | stream.rs (260 ln) | ProgressHub over vendored majra pubsub (or per-subscriber chans): `hub_new(capacity≥1)/hub_sink()/hub_subscribe()/hub_subscriber_count()`; `SSE_EVENT_NAME="step_progress"`; `progress_to_sse(p)→Str`; `sse_frame(event/0, id/0, data)` (one `data:` line per data line + terminating blank) | lag semantics: Open Q9 |
| 23 | `src/sql_store.cyr` | sql_store.rs (491 ln) | patra-backed ExecutionStore: `patra_store_open(path)` → `CREATE TABLE IF NOT EXISTS szal_executions(execution_id TEXT PRIMARY KEY, flow_name TEXT NOT NULL, data TEXT NOT NULL)`; save = upsert(record JSON via prepared stmt + binds); get/list(flow_name filter)/remove; `engine_sink()` returns ExecutionStore vtable; ordering spec: Running must never overwrite later Completed (synchronous writes satisfy trivially) | postgres dropped (Q6); `macro_rules!` → just write the one store |
| 24 | `src/mcp.cyr` | mcp/mod.rs (245 ln) | Tool = (def_fn, call_fn) pair; `szal_register_tools()`/`register_tools_with(audit, events)` → bote `dispatcher_new` + 54 `dispatcher_register_tool` calls (handlers are sync fns — no block_on bridge needed); `result_ok(text)` / `result_ok_json(v)` / `result_error(msg)` / `result_error_typed(code, msg)` (+_meta.error_code/retryable); McpErrorCode enum (6; Timeout/IoError/Internal retryable); **`validate_path(path)`** — manual canonicalize/normalize + CWD confinement (SECURITY boundary — no fs canonicalize exists; component-walk implementation required, traversal tests must pass); `tool_def(name, desc, props_json, required_vec)` | dep: vendored bote-core |
| 25 | `src/mcp_pool.cyr`, `src/mcp_tenant.cyr` | mcp/pool.rs, mcp/tenant.rs | NetworkPool: 3 majra `ratelimit_new` buckets (HTTP 10/s b50, DNS 100/s b200, Port 50/s b100 — fixed-point ×1000) + `check_http/dns/port(key)`; lazy global via `var _pool = 0; fn pool() { if (_pool == 0) { _pool = ...; } ... }`; TenantQuota/TenantCtx/TenantRegistry (mutex map), `check_tenant_quota` (unknown → permissive Ok), `check_tenant_tool_access` | |
| 26 | `src/mcp_tools_*.cyr` (15 files) | mcp/tools/* (54 tools, ~4,300 ln) | order-free among themselves; suggested: system → encoding (uuid/base64) → hash (sha256/md5/random_token) → template → conversion → math (own recursive-descent eval) → json (bayan) → file (ALL through validate_path; 1 MiB read cap, 10k dir entries, depth 20) → process (no shell; reject `..`/`/` in cmd; 30s timeout) → git (validate_git_ref rejects leading `-`; log cap 100) → net (curl shell-out; **`is_safe_url` SSRF guard: metadata endpoints + localhost + RFC1918**; rate-limit checks) → state_tools → step_tools → flow_tools → engine_tools | mechanical but high-volume; the security checks are the part that must NOT be dropped |
| 27 | `src/main.cyr` | lib.rs + entry | header comment, project includes in the exact order above, smoke `fn main()` | also the `[lib] modules` order |
| 28 | tests/benches/fuzz | 294 tests, 15 benches, 4 fuzz targets | see §5 | port suites in lockstep with each module |

### Feature-gate handling strategy

Rust has 8 features; `prometheus`/`barrier`/`dag` introduce **zero szal code** (pure majra
passthroughs — they vanish; majra-core bundle has barrier/dag built in, prometheus is gone
upstream). Strategy:

1. **Option-typed config fields stay always-present** (they're already `Option` in Rust → `0`
   pointers in Cyrius). No `#ifdef` needed for majra/hardware fields in EngineConfig/ExecCtx.
2. **Dist profiles replace features** (`[features]`/`optional = true` manifest keys are v6.3.x
   FUTURE — do not write them):
   - `[lib]` (default, `dist/szal.cyr`): full surface — engine + storage + stream + patra store
     + MCP + vendored bote-core/majra + hardware.
   - `[lib.core]` (`dist/szal-core.cyr`, recommended): engine-only — no MCP/bote, no
     ai-hwaccel, no majra (bus events without EventBus; queue_runner/distributed/hardware
     excluded). Add a bote-style core-only drift-guard smoke test.
3. `#ifdef` only for genuine target splits (`CYRIUS_TARGET_AGNOS` exit syscall etc.), not for
   feature emulation.

---

## 5. Project structure plan

```
szal/
  VERSION                      # 2.0.0 — single source of truth
  cyrius.cyml                  # below
  cyrius.lock                  # COMMITTED (sha256 of resolved deps)
  DEPS-PATTERN.md              # copy/adapt bote's
  src/
    main.cyr                   # entry: provenance header, project includes (order = §4), fn main
    vendor/bote-core.cyr       # synced pin: bote 2.7.3 dist/bote-core.cyr (69,597 B)
    vendor/majra.cyr           # synced pin: majra 2.4.5 dist/majra.cyr (85,031 B)
    uuid.cyr  md5.cyr  error.cyr  state.cyr  step.cyr  condition.cyr  flow.cyr
    bus.cyr  migration.cyr  engine_result.cyr  storage.cyr  metrics.cyr
    engine_core.cyr  engine_step_exec.cyr  engine_sequential.cyr  engine_parallel.cyr
    engine_dag.cyr  engine_hierarchical.cyr  engine_hardware.cyr
    engine_queue_runner.cyr  engine_distributed.cyr  engine_runner.cyr  engine_subflow.cyr
    stream.cyr  sql_store.cyr  mcp.cyr  mcp_pool.cyr  mcp_tenant.cyr
    mcp_tools_{system,encoding,hash,template,conversion,math,json,file,process,git,net,state,step,flow,engine}.cyr
  tests/
    test.sh                    # majra-style sh runner
    szal_core.tcyr szal_condition.tcyr szal_engine.tcyr szal_mcp.tcyr ...   # split per fn-table cap
    szal_core_only_smoke.tcyr  # core-profile drift guard (bote pattern)
  benches/bench_all.bcyr       # majra convention; emits CSV-compatible lines
  fuzz/
    fuzz_step_deser.fcyr fuzz_flow_deser.fcyr fuzz_flow_validate.fcyr fuzz_state_transitions.fcyr
  dist/szal.cyr  dist/szal-core.cyr     # committed, regenerated every tag
  scripts/version-bump.sh  bench-history.sh  sync-bote.sh  sync-majra.sh
  docs/ ...                    # §6
  lib/    (gitignored — cyrius lib sync + cyrius deps)
  build/  (gitignored)
  rust-old/                    # cyrius port archive; parity oracle; retired in a 2.0.x patch
```

### cyrius.cyml skeleton

```toml
[package]
name = "szal"
version = "${file:VERSION}"
description = "szal — workflow engine: step/flow execution with branching, retry, rollback, DAG and parallel stages"
license = "GPL-3.0-only"          # pending Open Q10 (Rust Cargo.toml says AGPL-3.0-only)
repository = "https://github.com/MacCracken/szal"
language = "cyrius"
cyrius = "6.1.33"

[build]
entry = "src/main.cyr"
output = "build/szal"

# Order mirrors src/main.cyr include order — single-pass forward-reference resolution.
[lib]
modules = [
    "src/vendor/bote-core.cyr", "src/vendor/majra.cyr",
    "src/uuid.cyr", "src/md5.cyr", "src/error.cyr", "src/state.cyr", "src/step.cyr",
    "src/condition.cyr", "src/flow.cyr", "src/bus.cyr", "src/migration.cyr",
    "src/engine_result.cyr", "src/storage.cyr", "src/metrics.cyr",
    "src/engine_core.cyr", "src/engine_step_exec.cyr", "src/engine_sequential.cyr",
    "src/engine_parallel.cyr", "src/engine_dag.cyr", "src/engine_hierarchical.cyr",
    "src/engine_hardware.cyr", "src/engine_queue_runner.cyr", "src/engine_distributed.cyr",
    "src/engine_runner.cyr", "src/engine_subflow.cyr",
    "src/stream.cyr", "src/sql_store.cyr",
    "src/mcp.cyr", "src/mcp_pool.cyr", "src/mcp_tenant.cyr",
    "src/mcp_tools_system.cyr", "...all 15 tool files...",
]

[lib.core]
modules = [
    "src/uuid.cyr", "src/error.cyr", "src/state.cyr", "src/step.cyr", "src/condition.cyr",
    "src/flow.cyr", "src/bus.cyr", "src/migration.cyr", "src/engine_result.cyr",
    "src/storage.cyr", "src/engine_core.cyr", "src/engine_step_exec.cyr",
    "src/engine_sequential.cyr", "src/engine_parallel.cyr", "src/engine_dag.cyr",
    "src/engine_hierarchical.cyr", "src/engine_runner.cyr", "src/engine_subflow.cyr",
    "src/stream.cyr",
]

[deps]
# Human-readable stdlib record (cyrius 6.x: `cyrius lib sync` provisions ./lib/).
# NO "json"/"base64"/"toml" here — carved into bayan (opt-in include). NO "slice"/"ct"
# (not separately resolvable — lib sync provides them).
stdlib = ["string", "fmt", "alloc", "freelist", "vec", "str", "syscalls", "io", "fs",
          "args", "assert", "test", "hashmap", "tagged", "result", "fnptr", "chrono",
          "thread", "sync", "async", "atomic", "process", "net", "random", "log",
          "sakshi", "patra", "sigil", "bayan", "bench"]

# ai-hwaccel has no git sub-deps → safe as a normal dep block.
[deps.ai-hwaccel]
git = "https://github.com/MacCracken/ai-hwaccel"
path = "../ai-hwaccel"
tag = "2.3.9"
modules = ["dist/ai-hwaccel.cyr"]     # → lib/ai-hwaccel.cyr

# bote-core + majra are VENDORED at src/vendor/ (hoosh pattern): a [deps.bote] block would
# make `cyrius deps` recurse into bote's own [deps.libro]/[deps.majra] git blocks (lib/ bloat
# + last-definition-wins symbol collisions). Pins: bote 2.7.3 dist/bote-core.cyr,
# majra 2.4.5 dist/majra.cyr — re-synced by scripts/sync-bote.sh / sync-majra.sh.
```

Build liturgy (README/CI): `cyrius lib sync && cyrius deps` →
`cyrius build --no-deps src/main.cyr build/szal` → `cyrius test tests/szal_core.tcyr` (etc.)
→ `cyrius bench` → `cyrius distlib && cyrius distlib core` → `cyrius audit`. CI: `--strict`,
`CYRIUS_DCE=1`, manifest-completeness gate (every src include appears in `[lib]`), capacity
gate (≥95% fn_table/idents fails), dist-freshness gate, version-consistency gate.

---

## 6. Docs rewrite checklist

| File | Action |
|---|---|
| `README.md` | Rewrite (bote/majra models): drop "for Rust" + crates.io badge; add "Written in Cyrius" line + port-provenance callout; capability/modules table; cyrius 6.x build liturgy; `[deps.szal]` consumer snippet; Cyrius quick-start code; "Ported from Rust" metrics table; consumers table (daimon/AgnosAI/sutra); verification section |
| `CHANGELOG.md` | Keep ENTIRE Rust history below; insert `## [2.0.0]` crossover entry (template below) + `## Historical (Rust) — preserved at git tag 1.2.0` divider |
| `CLAUDE.md` | Rewrite: Type = Cyrius library + dist bundle; toolchain table → `cyrius lib sync / deps / build --no-deps / test / bench / fmt / lint / audit / distlib`; cleanliness = `cyrius lint` + `cyrius audit`; add vidya's hard DO-NOT "never run cargo/clippy/rustc"; "do not modify lib/ or rust-old/"; keep no-commit/no-gh/zugot-sync/never-skip-benchmarks; key principles re-expressed (`var buf[N]` = bytes; check sentinel returns; Str vs cstr; enums for constants); point volatile state at docs/development/state.md |
| `CONTRIBUTING.md`, `SECURITY.md` | Rewrite build/report instructions for cyrius toolchain |
| `docs/ROADMAP.md` | Move to `docs/development/roadmap.md`; strip sqlx/Redis/Cargo-feature items; carry over 1.3 intents (crash recovery, cross-host fleet, `in`/`contains` condition operators) re-expressed for Cyrius |
| `docs/adr/0001-1.2-persistence-and-distribution.md` | Keep — annotate that decisions (sync ExecutionStore + ordered writer; encoding-not-server streaming; reuse majra fleet) carry over; mechanisms re-expressed in Cyrius |
| NEW `docs/adr/0002-port-from-rust-to-cyrius.md` | Write (vidya ADR-0001 model): Context/Decision/Consequences/Alternatives; scope in/out; rust-old policy |
| `Makefile` | Replace targets with cyrius CLI equivalents (or delete in favor of scripts + CI) |
| `scripts/bench-history.sh` | Rewrite: run `cyrius bench`, parse `name: Xns avg (min=… max=…)` lines → keep CSV columns `timestamp,version,commit,benchmark,time_ns,unit`; preserve old CSV as `bench-history-rust.csv` (vidya pattern). Do NOT copy vidya's own script (stale `cargo bench`) |
| `scripts/version-bump.sh` | Replace (majra's model): writes VERSION, verifies `${file:VERSION}` in cyml, prints next steps (distlib, CHANGELOG, tag, **zugot recipe sync**) |
| `.github/workflows/ci.yml` | Replace with bote's ci.yml + majra's 6.x `lib sync` steps (toolchain installed from the cyml pin grep); jobs in §5 |
| `.github/workflows/release.yml` | Replace with bote's: tag==VERSION gate, dist regen+freshness, smoke, archive (src tar, binary, `szal-${TAG}.cyr`, SHA256SUMS), CHANGELOG-section extraction, gh-release with cyrius.lock |
| `Cargo.toml, Cargo.lock, rust-toolchain.toml, deny.toml, codecov.yml, supply-chain/, fuzz/Cargo.toml` | Into `rust-old/` via `cyrius port`; gone when rust-old retires |
| NEW `DEPS-PATTERN.md` | Write szal's (from bote's, "non-negotiable" framing) |
| NEW `docs/development/state.md` | Volatile facts: version, pins, bundle sizes, test counts, consumer pins — refreshed every release |
| NEW `docs/development/semver.md` | majra model: "Starting with 2.0.0 (Cyrius port) … " + Cyrius breaking-change list (struct field offsets, enum constant meanings, wire formats) |
| NEW `docs/benchmarks-rust-v-cyrius.md` | Head-to-head table (Rust criterion baseline from `benchmarks/history.csv` vs Cyrius bench), Analysis, Where-each-wins. This doc is what makes retiring rust-old/ safe |
| NEW `docs/cyrius-feedback.md` | Language issues found during the port (bote convention) |
| `docs/architecture/overview.md`, `docs/guides/getting-started.md`, `docs/examples/` | Write/port (scaffold emits stubs); port the 5 Rust examples as Cyrius programs under docs/examples/ or programs/ |
| zugot `marketplace/szal.cyml` | Rewrite on majra.cyml's Cyrius shape (groups += "cyrius", `build = ["cyrius"]`, github_release source) — CLAUDE.md's recipe-sync loop step |
| Rustdoc doc-examples | They are the API spec — carry into Cyrius `#` doc comments + `cyrius doctest` blocks (`# >>> … / # === N`) |

### CHANGELOG crossover entry template

```markdown
## [2.0.0] — YYYY-MM-DD

**Full port from Rust to Cyrius.** szal is no longer a Rust crate. All modules re-implemented
from scratch in Cyrius (toolchain 6.1.33). The Rust implementation (1.2.0, 13,172 lines) is
preserved in `rust-old/` and at git tag `1.2.0`. Breaking for anyone importing szal as a Rust
dependency (secureyeoman stays on the 1.x Rust tags).

### Breaking
- Language: Rust → Cyrius; build: Cargo → `cyrius build` / `cyrius distlib`
- Dependencies: 14 Rust crates → Cyrius stdlib + first-party bundles
  (bote-core 2.7.3 vendored, majra 2.4.5 vendored, ai-hwaccel 2.3.9, patra via stdlib)
- API: module-prefixed function APIs (`flow_new`, `engine_run`, `step_with_retries`) over
  offset-addressed heap structs; handlers are (fn-pointer, ctx) pairs, not async closures

### Changed
- Generics/traits → i64 heap pointers + fn-pointer vtables
- async/await (tokio) → threads + channels + cancel tokens; cooperative cancel replaces abort()
- serde → bayan JSON value-tree + hand-written per-struct (de)serializers
- tracing → sakshi spans/levels; uuid → getrandom recipe; chrono → lib/chrono.cyr (epoch-ns)
- sqlx/sqlite → patra; tokio broadcast → majra pubsub (lag semantics documented)

### Added
- per-module list …; N test assertions across M suites; N benchmarks; 4 fuzz harnesses
- `dist/szal.cyr` + `dist/szal-core.cyr` bundles for daimon / AgnosAI / sutra

### Removed (deferred)
- `postgres` execution store (patra-only persistence in 2.0.0)
- `prometheus` passthrough (majra replaced it with a metrics fn-pointer vtable)
- [szal_md5 — ONLY if Q1 decides to drop]

### Performance — Cyrius vs Rust
| Benchmark | Cyrius | Rust | Delta |
…

### Known cyrius-language workarounds applied
…pointers into docs/cyrius-feedback.md
```

Follow-up `## [2.0.x]` entry retires `rust-old/` (bote 1.0.1 pattern), pointing at tag `1.2.0`,
after `docs/benchmarks-rust-v-cyrius.md` exists.

### VERSION strategy recommendation

**1.2.0 (Rust) → 2.0.0 (Cyrius)** — majra is the exact precedent. Plain SemVer from 2.0.0
(documented in docs/development/semver.md). `VERSION` file canonical; cyml reads
`${file:VERSION}`; release workflow asserts tag == VERSION. Rust history stays reachable at
tag `1.2.0` (required: secureyeoman pins the Rust crate). Note szal's Rust `0.D.M` date scheme
is already abandoned (1.2.0 ≠ date-based) — no conflict.

---

## 7. Open questions (deduped across all briefs)

**Answered from evidence (recommendation stands unless overridden):**

1. **MD5 (`szal_md5` tool)** — ANSWERED: hand-port RFC 1321 (~100 lines) as `src/md5.cyr`
   modeled on `lib/sha1.cyr`. Cheap, preserves the full 54-tool MCP surface; dropping it is a
   breaking MCP change for no real savings.
2. **bote dependency readiness** — ANSWERED (briefs conflicted; verified on disk): bote Cyrius
   **2.7.3 is finished** with committed `dist/bote-core.cyr` (the "in progress" status-board
   note is stale). Consume bote-core, vendored.
3. **Timestamps** — ANSWERED: store epoch-ns i64 in state/events; `iso8601()` (second
   precision) for display/`started_at` strings. chrono.cyr cannot round-trip subsecond RFC3339,
   so the Rust wire format cannot be preserved exactly — document in the crossover entry.
4. **UUID representation** — ANSWERED: `{hi, lo}` i64 pair internally (majra precedent);
   RFC-4122 string only at JSON/MCP boundaries via the byte-order-correct formatter (generate
   16 bytes, patch b[6]/b[8] — verify against szal's Rust test vectors).
5. **Rust-archive policy & version** — ANSWERED: 2.0.0; keep `rust-old/` as parity oracle
   through the port; retire in a 2.0.x patch after the benchmark-comparison doc lands.
6. **sqlx features** — ANSWERED (recommend): patra-only ExecutionStore in 2.0.0; postgres
   deferred (would drag majra-backends 137 KB + sigil/ct/sandhi). Synchronous patra writes
   satisfy the ADR-0001 ordering guarantee trivially.
7. **majra ResourcePool** — ANSWERED: does not exist in Cyrius majra; Rust szal only passed an
   empty pool → drop the dequeue parameter; nothing is lost.

**Still needing user/lead decision:**

8. **License** — Rust `Cargo.toml` says `AGPL-3.0-only` `[VERIFIED]`; szal CLAUDE.md and ALL
   Cyrius siblings (majra/bote/vidya) say `GPL-3.0-only`. The port must pick one for
   cyrius.cyml/LICENSE/README badge. Evidence leans GPL-3.0-only (ecosystem-consistent), but
   this is a legal call — **user decision**.
9. **`registry_new` collision (bote-core × ai-hwaccel)** — must be resolved BEFORE engine
   porting starts: (a) sed-rename in szal's synced copy, (b) upstream ai-hwaccel 2.4.x rename
   (cleanest, benefits mihi/hoosh), (c) tested include-order trick. Recommend (b) with (a) as
   interim — needs lead/upstream coordination.
10. **Engine concurrency architecture per mode** — recommend: threads + permit-channel +
    cancel tokens for Parallel/DAG/Distributed (ai-hwaccel/majra-dag precedent); plain
    sequential for Sequential/Hierarchical; sync stdio loop for MCP. Accepts a semantic delta:
    `JoinHandle::abort()` → cooperative cancellation (steps poll the token; a hung step blocks
    its worker thread until flow timeout). Needs lead sign-off because observable timeout/
    cancel behavior changes vs Rust.
11. **Logging under threads** — sakshi is explicitly single-threaded. Options: supervisor-only
    logging / mutex-guard every log call / dedicated logging thread fed by a chan. Recommend
    chan-fed logging thread for engine paths (keeps per-step logs). Decide before
    engine_parallel lands.
12. **ProgressHub / EventBus lag semantics** — tokio broadcast drops oldest for lagging
    receivers; majra pubsub per-subscriber bounded chans block/drop-newest. Which behavior does
    the port SPECIFY? (Affects stream.cyr + bus.cyr contract wording; recommend "drop-newest
    when subscriber chan is full, documented" — but it is a contract change.)
13. **Semaphore fairness** — permit-channel emulation has futex wake order, not tokio's FIFO
    fairness. Acceptable? (Likely yes — no szal test asserts fairness — confirm.)
14. **Dist profile split** — ship `dist/szal-core.cyr` (engine-only) alongside `dist/szal.cyr`
    from day one? Recommend yes (bote precedent; daimon-like consumers want engine-without-MCP).
    Costs a drift-guard smoke test + CI loop.
15. **`validate_path` security semantics** — no `canonicalize` in Cyrius; the manual normalizer
    cannot resolve symlinks the way `tokio::fs::canonicalize` does. Decide: component-walk +
    `sys_readlink` loop (closest parity) vs lexical-only normalization + documented symlink
    caveat. Security-relevant — lead decision.

---

## Appendix: conflict-resolution ledger (what was checked, where)

| Claim in a brief | Verdict | Source checked |
|---|---|---|
| Result is packed bit-63, zero-alloc (vidya brief) | **WRONG** — heap tagged {tag@0, payload@+8}, 16-byte alloc | `/home/macro/Repos/cyrius/lib/result.cyr` header |
| `var buf[N]` at fn scope is stack (vidya brief) | **WRONG** — static data section, shared across calls | vidya `semantics_runtime.cyml` `var_buf_in_library_functions` |
| Negative literals supported (vidya brief) | **WRONG** — `(0 - N)` required | `cyrius/docs/guides/faq.md` limitation 7 |
| Bare `return;` rejected in if-blocks (lang-core brief) | **STALE** — fixed v5.10.48 (synthesizes `return 0;`) | cyrius CHANGELOG [5.10.48] |
| `var buf[IDENT]` literal-only (lang-core brief) | **STALE** — enum constants accepted since v5.10.48; plain `var` consts still rejected | cyrius CHANGELOG [5.10.48] |
| Initialized-globals cap 1024 (vidya brief) | **WRONG** — ~64 | faq.md item 4 |
| defer cap 8/fn (vidya brief) | **WRONG** — 64 defer/secret blocks | `cyrius/src/frontend/parse.cyr` |
| `bench_batch_start(name, iters)` (vidya brief) | **WRONG** — `bench_batch_start(b)` / `bench_batch_stop(b, batch_size)` | `lib/bench.cyr` |
| `map_size` vs `map_count` | both exist | `lib/hashmap.cyr` (lines 173, 312) |
| `chan_new()` no-arg (vidya brief) | **WRONG** — `chan_new(cap)` | `lib/thread.cyr:313` |
| 94 stdlib modules (lang-core/deps) | 88 today | `ls cyrius/lib/*.cyr \| wc -l` |
| `json` in `[deps].stdlib` (deps/template briefs) | **DO NOT** — `lib/json.cyr` deleted (bayan carve) | `ls cyrius/lib/json.cyr` → ENOENT |
| bote Cyrius "in progress" (stdlib brief) | **STALE** — 2.7.3 with committed dist bundles | `/home/macro/Repos/bote/VERSION`, `dist/` |
| `[deps].stdlib` auto-prepend vs record | both: auto-prepend at build; under 6.x lib sync provisions files and the list is a human-readable record; build `--no-deps` | majra cyrius.cyml [deps] comment |
| License AGPL vs GPL | real discrepancy — Open Q8 | szal Cargo.toml line 6 vs CLAUDE.md |
