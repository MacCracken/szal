# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.3.1] — 2026-11-03

Toolchain and dependency patch — cyrius 6.7.9 → **6.7.18**; majra 2.12.0 → **2.13.0**, bote-core
3.4.10 → **3.8.4**, ai-hwaccel 2.4.2 → **2.5.6** — each its library's latest release (checked against
GitHub as well as the local tags), all three pinned to 6.7.18, which is also cyrius's latest. Vendored
byte-for-byte from the tags. No source change in szal's code; **1,495 assertions across 47 test
files**, the same count suite for suite as 2.3.0, 5 fuzz harnesses (356,290 properties), 15
benchmarks, 0 failures; `rust-old/` untouched. One behaviour change comes in with the stdlib's bayan
(below, and parity-notes §25).

### Changed — cyrius pin 6.7.9 → 6.7.18
- **Stdlib folds:** bayan 1.5.13 → **1.11.1**, sigil 3.13.11 → 3.13.15, patra 1.16.0 → 1.16.6, sakshi
  2.5.8 → 2.5.10. `cyrius lib sync` still provisions 58 files; the build pulls 7 more, and
  `cyrius.lock` records the 72 (re-provisioned from empty; a fresh checkout regenerates the same lock).
- ⚠ **JSON with a duplicate object key or a malformed number no longer parses** (bayan 1.11.1's
  refusals). Measured through `szal_json_validate`: `{"a":1,"a":2}` was `valid: true` on 6.7.9 and is
  `valid: false, "duplicate key"` on 6.7.18; `{"a":01}` was accepted and is refused; a tool whose
  arguments repeat a key answers `missing required field: …`. Against the Rust oracle (`serde_json`):
  malformed numbers and duplicate step / flow fields now get Rust's answer (refused), while free-form
  JSON with a duplicate key, which Rust accepts keeping the last value (szal ≤ 2.3.0 kept the first),
  now diverges further. Recorded and accepted as parity-notes **§25**; the bundle header says so.
- **Main-build warnings 22 → 21.** sigil 3.13.15 no longer puts its over-budget buffer in static
  storage, so its frame-budget warning is gone and `large static data` drops from 455 KB to 136 KB.
  Left: 20 of the "assigning non-pointer to typed pointer" family, and `large static data`. The
  suites' builds print only those two kinds.
- **No language change reaches szal.** 6.7.10–6.7.18 are mostly typed-struct, vector and generic
  semantics, which szal's untyped-pointer code does not use; the one statement rule that could have
  (FA2-1: a statement that is only `load64(..)` and the like no longer compiles) does not occur, and
  every suite builds. `scripts/scan-collisions.sh`'s table of what the compiler reports is unchanged
  on 6.7.18.
- distlib still answers the same under any listing order (re-checked: four orders). The sidecar keeps
  its 11 leaves; `net` moved from 5th to last in their order.
- Benchmarks: no regression. Min of three interleaved rounds against the 2.3.0 tree on 6.7.9, on a
  box at load ~5: every row within ±3 % or faster, the parse rows ~20 % faster (`step_deserialize`,
  `flow_50_deserialize`). A single run made under heavier load had suggested slowdowns; the A/B did
  not reproduce them.

### Changed — dependencies
- **bote-core 3.4.10 → 3.8.4** (3.4.11 – 3.8.4: MCP 2026-07-28 beside the older revisions, HTTP
  transports, OAuth token checks, output schemas, cancellation and progress — none of it reached by
  szal's tools). Every fn szal calls (19) keeps its signature, and bote's "Breaking" notes reach only
  its HTTP functions and `x-mcp-header` annotations ("the handler ABI is untouched"). bote-core now
  needs cyrius ≥ 6.7.13 (was 6.7.2) and still needs `sakshi` from its consumer; its requires block no
  longer includes bayan, which szal declares itself.
- **majra 2.12.0 → 2.13.0** — the changes are in its PostgreSQL / patra-queue backends, outside the
  base bundle szal vendors; every fn szal calls (24) keeps its signature.
- **ai-hwaccel 2.4.2 → 2.5.6** — the first move since 2.2.1. Every fn szal calls keeps its signature;
  the `REQ_*` / `FAMILY_*` values szal uses are unchanged (2.5 adds `FAMILY_UNKNOWN = 5`).
- `scripts/consumer-check.sh` passes generic and against **hoosh 2.9.4** (its latest release,
  extracted from the tag; it vendors bote-core 3.8.3 and the same majra and ai-hwaccel): nothing
  collides, the bundle adds no duplicate warning, `tools/list` = 55, hoosh adds only `math`.

### Issues
- The 2026-10-05 channels issue stays open (macOS / Windows still not run).

## [2.3.0] — 2026-11-03

Toolchain and dependency refresh onto the cyrius **6.7** language minor — cyrius 6.6.20 →
**6.7.9**; majra 2.9.3 → **2.12.0**, bote-core 3.3.17 → **3.4.10**, ai-hwaccel **2.4.2** (kept: no
ai-hwaccel release pins 6.7.9). Each library is at its last release pinned to 6.7.9 or below,
vendored byte-for-byte from its tag, and the three are the files hoosh 2.9.0 (its 6.7.9 release)
uses. A MINOR release for the language minor and for bote 3.4's dispatcher contract (below); szal's
own source, behaviour and public names are unchanged. **1,495 assertions across 47 test files**, the
same count suite for suite as 2.2.1, 5 fuzz harnesses (356,290 properties), 15 benchmarks, 0
failures; `rust-old/` untouched.

### Changed — cyrius pin 6.6.20 → 6.7.9
- **No source change was needed.** The 6.7 changes that could silently alter a program were
  checked against szal's own code (`src/`, tests, fuzz, benches): no lone `!` (until 6.7.3 the lexer
  dropped it, so `!x` compiled as `x`), no identifier named `trait` / `const` / `true` / `false` /
  `do` / `dyn` (now reserved), and no unary `-` / `~` on a constant followed by an operator (6.7.9
  re-binds `-K + 10`). The main build prints the same 22 warnings as 2.2.1, kind for kind.
- **Stdlib folds:** bayan 1.5.12 → 1.5.13, sigil 3.13.10 → 3.13.11, patra 1.15.2 → 1.16.0, sakshi
  2.5.7 → 2.5.8. `cyrius lib sync` still provisions 58 files; `cyrius.lock` regenerated (64 files +
  the `cyrius 6.7.9` trailer, the same from a fresh checkout).
- ⚠ **Re-provision `lib/` from empty on a pin move:** `rm -rf lib cyrius.lock && cyrius lib sync`.
  Run over the 6.6.20 tree, 6.7.9's `lib sync` left the four files the previous *build* had pulled in
  (`mmap`, `boxed`, `sys`, `slice`) at their 6.6.20 content, hashed them into the new lock, and the
  next `cyrius build` refused: "cyrius.lock and the pinned stdlib snapshot DISAGREE under an
  unchanged pin 6.7.9". CI starts from an empty `lib/` and is not affected. Recorded in
  `cyrius.cyml` and state.md.
- **distlib no longer depends on the filesystem's listing order** (fixed upstream in 6.7.7: the
  snapshot is sorted). Re-checked: `dist/szal-mcp.cyr` / `.deps` are byte-identical under four
  listing orders, with or without `src/mcp_bundle.cyr`'s 2.2.1 seed include. The seed stays — it names
  the honest leaf and keeps the sidecar the same on any distlib; its comment says so. The sidecar is
  unchanged (11 leaves).
- `scripts/scan-collisions.sh`'s table of what `cyrius build --strict` reports, re-measured on
  6.7.9: every row unchanged, and a later same-named enum constant still wins for earlier code.

### Changed — dependencies
- **bote-core 3.3.17 → 3.4.10.** Every fn szal calls (19) keeps its signature. What changes for a
  consumer of `dist/szal-mcp.cyr`, all on bote's side of the dispatcher:
  - from 3.4.8 `dispatcher_register_tool` compiles each tool's schema and checks the definition —
    bote's own consumer audit names szal: "its 73 literal property schemas are all valid, so
    registration accepts every tool" — and **`tools/call` arguments are checked against the
    schema** (types, enums, bounds) before szal's handler runs, so a wrong-typed argument gets bote's
    `isError` result naming it instead of the handler's own validation error;
  - a result with `isError: true` is audited and published as a failure (`bote/tool/failed`); szal's
    handlers already return CallToolResults, so nothing is re-wrapped;
  - bote-core needs cyrius ≥ 6.7.2 and calls `sakshi_*` without including `lib/sakshi.cyr` (its
    requires block omits it): a consumer that vendors it declares `sakshi` itself.
    `tests/szal_consumer_bundle.tcyr`, which stands in for such a consumer, now includes it — the
    one test change in this release. hoosh 2.9.0 already declares it.
- **majra 2.9.3 → 2.12.0.** Every fn szal calls (24) keeps its signature. 2.12.0's metrics hooks pass
  their third and fourth arguments as parameters (they went through unsynchronised globals,
  `_metrics_arg3` / `_metrics_arg4`, now deprecated); szal only calls the hooks and never reads those
  globals. Also new: `ratelimit_evict_to` (unused by szal), and free functions for the lists and
  stats blocks majra returns.
- **ai-hwaccel stays 2.4.2.** It pins 6.6.20 and builds clean on 6.7.9; 2.5.0+ pin 6.7.18, a later
  move. hoosh 2.9.0 makes the same choice.
- `scripts/consumer-check.sh` passes generic and against hoosh 2.9.0 (extracted from its tag):
  nothing collides, the bundle adds no duplicate warning, `tools/list` = 55; hoosh adds only `math`.
- `src/mcp_bundle.cyr` (the bundle's contract header) names the new tested versions, bote-core's
  cyrius and sakshi requirements, and the dispatcher's schema checks.

### Issues
- The 2026-10-05 channels issue stays open: nothing has run the stream / parallel suites on arm64
  macOS or Windows.

## [2.2.1] — 2026-11-03

Toolchain and dependency patch — cyrius 6.6.6 → **6.6.20**; majra 2.9.1 → **2.9.3**, bote-core
3.3.13 → **3.3.17**, ai-hwaccel 2.4.0 → **2.4.2**, each the last release of its library pinned to
cyrius 6.6.20 and vendored byte-for-byte from its tag (the same three files hoosh 2.8.2 commits).
No behaviour change: every suite counts the same assertions as the 2.2.0 tree does on 6.6.6 —
**1,495 across 47 test files** (state.md said 1,494 at 2.2.0; that tree also counts 1,495), 5 fuzz
harnesses (356,290 properties), 15 benchmarks, 0 failures; `rust-old/` untouched.

### Changed — cyrius pin 6.6.6 → 6.6.20
- **`bayan_json_v_obj_get` → `bayan_json_v_obj_get_by_cstr`** at all 144 call sites in `src/`, and
  on 55 lines in `tests/` and the `consumer-check.sh` probe. bayan 1.5.10 (first shipped in cyrius
  6.6.13) deprecated the old name, which does not say what the key is; at this pin it is a one-line
  forwarder to `_by_cstr`, so the rename changes nothing — every key szal passes is a literal or a
  `str_cstr(...)`. Left alone, the main build printed 144 deprecation warnings.
  ⚠ **Consumers:** `dist/szal-mcp.cyr` now needs a cyrius ≥ 6.6.13 stdlib (bayan ≥ 1.5.10).
  ai-hwaccel 2.4.2 already does; `src/mcp_bundle.cyr` states it.
- **Main-build warnings 28 → 22.** The six unreachable `undefined function` warnings are gone:
  `argc` / `argv` because ai-hwaccel 2.4.2's dist includes `lib/args.cyr` itself, and `sys_uname` /
  `shake256` / `_keccak_*` because sigil 3.13.10 brings its own leaves. Left: 20 of the "assigning
  non-pointer to typed pointer" family, sigil's static-storage frame-budget warning, and
  `large static data`.
- **`cyrius.lock` is new, and committed.** From 6.6.20 `cyrius build` writes the hashes of the 64
  provisioned stdlib files plus a `cyrius 6.6.20` trailer (the same file from a clean tree).
  majra, bote, ai-hwaccel and hoosh commit theirs, and `.gitignore` already said to.
- **`dist/szal-mcp.cyr` / `.deps` regenerated by 6.6.20's distlib.** The bundle opens with a
  compile-verified `# Requires` block (one `include` per sidecar leaf, so `include
  "dist/szal-mcp.cyr"` compiles on its own), and the sidecar is now what the compile-verify proves
  rather than an inference: **29 → 11 leaves**. hoosh 2.8.2 has only `math` left to add for it (it
  was `result`, `math` and the over-inferred `trait`). distlib names 15 symbols still undefined
  after the verify: the 13 bote / majra fns the consumer supplies, and `sys_dup2` / `sys_execve` on
  x86_64-agnos only (the process tool is Linux-only).
- **Stdlib folds** at this pin: bayan 1.5.6 → 1.5.12, sigil 3.12.18 → 3.13.10, patra 1.14.3 →
  1.15.2, sakshi 2.5.7. `cyrius lib sync` still provisions 58 files.
- **Toolchain behaviour checked against szal**, none needing a source change:
  - *Enum constants past var index 1024 fold* (6.6.8). `var buf[ENUM]` now works at any index, and
    a conflicting redeclaration warns where it used to print nothing. A later same-named constant
    still wins for every read, earlier code included — at any index, on both compilers (measured).
    szal's literal buffer sizes stay; `scripts/scan-collisions.sh` stays the gate.
  - *A struct redefined with another layout warns* (the FIRST definition wins); silent at 6.6.6.
  - *`lib/hashmap.cyr` counts tombstones and rebuilds in place on a new-key `map_set`.* szal's one
    raw slot walk, `migration_latest_version`, reads a u64 map and inserts nothing; the slot layout
    is unchanged (the new count word sits after slot `cap`, outside the walk).
  - *Channels block on arm64 macOS and Windows too* (6.6.16), and the Linux channel's lost wake-up
    is fixed. See Issues.

### Fixed
- **`dist/szal-mcp.deps` and the bundle's `# Requires` block depended on the host's directory
  order.** 6.6.20's distlib credits a name two stdlib leaves both declare to whichever its snapshot
  listing reaches first: `O_WRONLY` (the `file_open` flags in `mcp_tools_file`) is declared by
  `lib/io.cyr` and by the syscalls peers, and went to `syscalls` on one listing order and to `io`
  on the reverse — the same leaves in a different order, enough to fail CI's byte-for-byte drift
  check on a runner whose filesystem lists the other way. Reproduced with two copies of the 6.6.20
  snapshot populated in opposite orders. `src/mcp_bundle.cyr` now includes `lib/io.cyr`, which
  seeds the verify so the name is in scope before any attribution (majra 2.9.3's fix for the same
  defect); the bundle comes out byte-identical under four listing orders (forward, reverse, two
  shuffles) and matches the committed one.
- **`src/stream.cyr` and parity-notes §9 said the stdlib has no `chan_try_send`.** It has had one
  since cyrius 6.4.84 (`-2` when full). Comment and note only — the hub still blocks; switching it
  to drop is a behavioural change for its own release (state.md "Next").
- **`cyrius doc --check` reported 34 undocumented public fns** — 33 in `src/engine_core.cyr` and
  `main` in `src/main.cyr`, present since at least 2.1.2 and carried as a known gap in state.md.
  cyrdoc counts a fn as documented only when a `#` comment sits on the line directly above it, so a
  shared group comment (`# Accessors.`) covered just the first fn under it. Every `cb_*`,
  `flow_ctx_*`, `exec_ctx_*`, `engine_config_*` and `step_progress_*` accessor and setter, and
  `main`, now has its own one-line doc comment (cyrdoc renders only that line). Comment-only: the
  `build/szal` binary is byte-identical before and after.

### Added
- **CI**: `Format` (`cyrius fmt <file> --check`) and `Doc coverage` (`cyrius doc --check`) steps
  over `src/*.cyr`, after Lint — the two M1 exit gates CI never ran. Doc coverage matches cyrdoc's
  summary line as well as its exit code; both fail if the glob matches nothing.

### Changed — dependencies
- **majra 2.9.1 → 2.9.3, bote-core 3.3.13 → 3.3.17, ai-hwaccel 2.4.0 → 2.4.2** — the releases
  that move each library to cyrius 6.6.20; the next release of each (majra 2.10.0, bote 3.4.0,
  ai-hwaccel 2.5.0) pins 6.7.x. In the vendored files the only code changes are ai-hwaccel's three
  `_pj_*` JSON readers moving to `bayan_json_v_obj_get_by_cstr` (szal does not reach them) and
  bote's `_bote_server_version` string; the rest is each dist's new `# Requires` block and majra's
  comments about the new hashmap. Every fn szal calls is unchanged.
- `scripts/scan-collisions.sh --check`: still the one allow-listed intersection, `REQ_NONE`.
  `scripts/consumer-check.sh` passes generic and against hoosh 2.8.2 (its 6.6.20 release, extracted
  from the tag): nothing collides, the bundle adds no duplicate warning, `tools/list` = 55.
- `scripts/scan-collisions.sh`'s header table re-measured on both compilers (the struct and
  past-1024 rows changed; see above).

### Issues
- [`2026-10-05-cyrius-6616-channels-block-on-macos-and-windows.md`](docs/development/issues/2026-10-05-cyrius-6616-channels-block-on-macos-and-windows.md)
  stays open: the pin is past 6.6.16, the stale comment is fixed, and the stream / parallel suites
  pass on Linux — but its closing condition is a run on macOS and Windows, which CI does not do.

## [2.2.0] — 2026-09-25

Toolchain and dependency refresh — cyrius 6.6.2 → **6.6.6**; majra 2.7.0 → **2.9.1**, bote-core
3.3.7 → **3.3.13**, ai-hwaccel 2.3.19 → **2.4.0**, each at its latest release and now vendored
**byte-for-byte** from its release tag — plus the consumer bundle hoosh filed for
(`dist/szal-mcp.cyr`, `szal_register_into`), and all four open issues closed and archived. A MINOR
release because szal's own public Cyrius names changed (ADR 0002); no Cyrius program called them
yet. Full suite: **1,494 assertions across 47 test files**, 5 fuzz harnesses (356,290 properties),
15 benchmarks, 0 failures; `rust-old/` parity oracle untouched.

### Fixed
- **Parenthesised conditions stopped parsing in every build that carried the math tool** — the
  shipped `szal` binary included. `src/mcp_tools_math.cyr` declared `TOK_LPAREN = 6` /
  `TOK_RPAREN = 7`; `src/condition.cyr` declares the same names as 13 / 14. An enum constant
  registered past var index 1024 is not folded, so the later value won for **every** read: the
  condition lexer emitted `(` as its own `TOK_GT` and `)` as `TOK_GTE`, and `(x)`, `(x) == 1`,
  `!(x)`, `((x == 1))` and `x == (1)` all failed with parse errors (a stray `)` was even reported as
  `>=`). The engine logs a condition parse error and runs the step, so a step guarded by such a
  condition ran regardless. Reproduced on the released 2.1.2 tree at its 6.6.2 pin. None of the
  condition suites or fuzz harnesses include the math tool, and the collision scan only compared
  szal with other libraries, so nothing saw it. The math tokens are now `MEV_TOK_*`;
  `tests/szal_mcp_tools_net.tcyr` checks the parser in main.cyr's include order (15 assertions, 12
  of which fail on the old code); and `scripts/scan-collisions.sh` flags any name two szal files
  both define.
- **`szal_server_info` reported version `2.0.0`** — a hand-kept literal no bump ever updated, so
  2.1.0, 2.1.1 and 2.1.2 all said 2.0.0. It now reports `SZAL_VERSION`, which
  `scripts/version-bump.sh` writes, the CI version check compares with `VERSION`, and
  `tests/szal_mcp_tools_engine.tcyr` compares with `./VERSION` at runtime. (Not
  `CYRIUS_PKG_VERSION`: that resolves per build unit, so inside `dist/szal-mcp.cyr` it would report
  the consumer's version.) Its `description` also diverged from Rust's `CARGO_PKG_DESCRIPTION`
  ("Workflow orchestration engine" vs the crate's "Workflow engine — step/flow execution with
  branching, retry, rollback, and parallel stages"); it now matches, and the test pins it.
  parity-notes §20 is no longer a divergence.
- **`_sub_flow_dispatch` handed its inner handler's Result back as one value** — the compiler's
  pair-return check ("returns a `: stack` pair on another path but a SINGLE value here") has
  flagged it since at least 6.6.2, and 2.1.2 shipped with that warning. The non-`sub_flow` delegate
  path was `return handler_invoke(...)`, a fn-pointer call the compiler
  types as one value, so the payload only survived because rdx happened to outlive two epilogues.
  Latent — current codegen preserves it, and the old form passes the new test too — but now
  explicit (`_sub_flow_delegate` binds both halves and re-wraps). `tests/szal_engine_subflow.tcyr`
  now checks that delegation keeps both the Ok payload and the Err message.
- **`scripts/sync-ai-hwaccel.sh` vendored unreleased code under a release label.** It copied the
  checkout's working tree and labelled it with `VERSION`; at this sync the ai-hwaccel checkout sat
  one commit past 2.4.0 (comments only). All three sync scripts now extract the dist from the
  release tag and check its `# Version:` line.

### Changed — ⚠ szal's own names (ADR 0002)
szal renames its own symbols instead of rewriting the libraries it vendors, so szal and its
consumers compile against the same upstream files. Renamed, each with its whole family:

| was | now |
|---|---|
| `EventType` members `FLOW_*` / `STEP_*` | `SZAL_FLOW_*` / `SZAL_STEP_*` (majra owns `STEP_COMPLETED` / `_FAILED` / `_SKIPPED`) |
| `TriggerMode` / `TRIGGER_ALL` / `TRIGGER_ANY`, `StepStatus` (type name) | `SzalTriggerMode` / `SZAL_TRIGGER_*`, `SzalStepStatus` |
| `step_result_new`, `uuid_generate` | `szal_step_result_new`, `szal_uuid_generate` |
| `compiled_compile` / `_evaluate` / `_source` / `_ast` | `szal_condition_compile` / `_evaluate` / `_source` / `_ast` |
| `cache_new` / `_evaluate` / `_len` / `_is_empty`, `CACHE_SIZE`, `CACHE_ENTRY_SIZE`, `CE_*`, `CACHE_MAP` | `szal_condition_cache_*`, `SZAL_COND_CACHE_*`, `SZAL_CE_*` |
| `result_ok` / `_ok_json` / `_error` / `_error_typed`, `validate_path` | `szal_result_*`, `szal_validate_path` |
| `mcp_err_name` / `_retryable`, `mcp_tool_def` / `_new` / `_def_of` / `_handler_of`, `MCP_*` codes | `szal_mcp_err_*`, `szal_tool_*`, `SZAL_MCP_*` |
| `register_tools[_with](tools, …)`, `all_tools`, `<group>_tools` | `szal_register_tool_vec[_with]`, `szal_all_tools`, `szal_<group>_tools` |
| `pool`, `network_pool_new`, `pool_check_*` | `szal_net_pool`, `szal_network_pool_new`, `szal_pool_check_*` |

The Rust oracle's names remain in comments and parity notes. The scan now reports a single
intersection anywhere in szal's build: `REQ_NONE`, shared with ai-hwaccel on purpose.

### Added
- **`dist/szal-mcp.cyr` + `dist/szal-mcp.deps`** (`[lib.mcp]`, `cyrius distlib mcp`): the 54 MCP
  tools for a consumer that already owns a bote dispatcher — only szal's own modules that
  `szal_all_tools()` reaches, no vendored library or stdlib inside. It needs no ai-hwaccel (the tool
  closure never reaches `engine_hardware`). `src/mcp_bundle.cyr`, a comment-only first module,
  carries the consumer contract (include order, tested versions, entry points, host guards) into
  the vendored file.
- **`szal_register_into(d)`** — add the 54 tools to the consumer's existing dispatcher, under its
  audit and event sinks, via bote's `dispatcher_register_tool`. Also `szal_all_tools()`,
  `szal_register_tool_vec_into`, and `szal_register_tools_with(audit, events)` (Rust's
  `register_tools_with`).
- **`tests/szal_consumer_bundle.tcyr`** (47th suite) — builds the committed bundle with no szal
  `src/` in the compile unit and drives it through bote's JSON-RPC codec: 55 tools after
  registering into a dispatcher holding one, the consumer's event sink hears 54 registrations, the
  path / SSRF / exec guards hold.
- **`scripts/consumer-check.sh [consumer-checkout]`** — the acceptance bar from the hoosh filing.
  Against hoosh 2.7.1: its program plus the bundle builds `--strict`; the only duplicate warning is
  sigil x `lib/sys.cyr` `uname_release`, which hoosh already prints without the bundle; nothing
  collides; `tools/list` returns 55.
- **`scripts/scan-collisions.sh`**: an intra-szal pass; allow-list entries must agree on VALUE
  (a renumbered `REQ_NONE` now fails); a `--consumer FILE…` mode that checks the bundle against a
  consumer's compile set. Positive controls: the old math tokens, a bare `TRIGGER_ALL`, and
  `REQ_NONE = 1` each fail it.
- **CI**: a Consumer bundle step (regenerate the bundle and fail on any drift — `distlib --check`
  cannot target one profile — then the generic consumer scan); `SZAL_VERSION` in the version check;
  the bundle, the new suite and `consumer-check.sh` in the harness manifest.
- **ADR 0002** — szal owns its namespace; vendored libraries stay byte-identical.

### Removed
- **`szal_thread_join`.** cyrius **6.5.8** fixed `lib/thread.cyr`'s lost-wakeup `thread_join`
  the way szal's shim did (one load feeds both the loop test and `FUTEX_WAIT`), so all four join
  sites call the stdlib again. Verified with the stress suite, as the issue required: 12 / 12
  green on the real 6.6.6 lib, and 4 / 4 watchdog trips (`DEADLOCKED`) with the double load put
  back into a copy of `lib/thread.cyr` — so the suite still guards the primitive szal now uses.
- **The `var STEP_I64_MAX` workaround.** cyrius **6.5.36** fixed the bit-62 enum-constant fold, so
  `STEP_I64_MAX` is an `enum` constant again. `tests/szal_step.tcyr` now compares it with inline
  literals; every earlier assertion compared it with itself, which passes when it folds to -1
  (forcing it to -1 fails exactly the two new assertions).
- The vendored-copy renames (`MJ_ERR_*`, `MJ_STEP_*`, `MJ_TRIGGER_*`, `majra_uuid_generate`,
  `majra_step_result_new`, `MJ_SYS_GETRANDOM`, `bote_compiled_compile`).

### Changed — cyrius pin 6.6.2 → 6.6.6
- `lib/` re-provisioned (`rm -rf lib && cyrius lib sync`, 58 files). 46/46 existing suites and all
  5 fuzz harnesses were green on 6.6.6 before any source change.
- The roadmap's 6.6.6 migration note, checked: majra's `ret2` pair returns pass the pair-return
  check, and the only szal site the compiler flags is `_sub_flow_dispatch` — a warning (not the
  error the note predicted) that 6.6.2 already printed, fixed above. `cyrius deps` in this zero-git-dep tree exits 0 and writes **no** `cyrius.lock`,
  so there is no lock to commit; `dist/` profiles did not exist before this release. The
  `O_APPEND` Windows fix stays latent: szal ships no PE target.
- Toolchain behaviour worth knowing, all new in 6.6.x: the wrapper re-execs the pinned version and
  that binary uses its SIBLING `cycc`, so the 2.1.1 trap (a newer `~/.cyrius/current` compiling
  with the wrong cycc) is gone; `cyrius build` re-syncs `lib/` from the pinned snapshot before
  compiling, so a stale `lib/` can no longer shadow the pin — and a deliberate `lib/` mutation is
  silently undone.
- Main-build warnings: 27 at 6.6.2, 29 on the same tree at 6.6.6, 28 now. The two new ones are
  `undefined function 'sys_uname'` (sigil 3.12.18 calls `lib/sys.cyr`, unreachable from szal;
  including `sys` would trade it for sigil's `duplicate fn 'uname_release'`) and sigil's
  static-storage frame-budget note, promoted to a warning in 6.6.5. The pair-return warning is
  fixed.

### Changed — dependencies
- **majra 2.7.0 → 2.9.1.** Every fn szal calls is signature-identical. It picks up 2.7.3's
  `uuid_generate` via `sys_getrandom` (which is what made the `MJ_SYS_GETRANDOM` rename
  unnecessary), 2.8.1's `ratelimit_check` reading the clock under its mutex and `fleet_submit` no
  longer losing a job to a concurrent `fleet_deregister_node`, and 2.8.2's heartbeat trackers owning
  their key copies (szal registers per-run ids). 2.9.0's two wire breaks are in encrypted IPC and
  signed envelopes, which szal does not use. majra's error codes are `MAJRA_ERR_*` now, with bare
  `ERR_*` aliases until 3.0.0; none collide with szal or the stdlib.
- **bote-core 3.3.7 → 3.3.13.** All 13 fns szal calls are identical; `ping` and protocol version
  `2025-06-18` conformance fixes.
- **ai-hwaccel 2.3.19 → 2.4.0.** `REQ_*` / `FAMILY_*` value table byte-identical; detection fixes
  (one physical device, one profile; integrated GPUs through Vulkan; unified memory counted once).

### Issues
All four in `docs/development/issues/` are closed and moved to `docs/development/issues/archive/`,
each with its resolution on top: the `registry_new` collision (re-verified — both sides renamed),
the `thread_join` deadlock (fixed upstream at 6.5.8, shim retired), the bit-62 enum fold (fixed
upstream at 6.5.36, enum restored), and the hoosh consumer bundle (shipped here).

## [2.1.2] — 2026-09-10

**Migrated to the cyrius 6.6.x value form.** 46/46 test files pass.

### Changed — cyrius pin 6.5.35 → **6.6.2**

19 declarations split across 14 `src/` modules, plus 60 sites across 13 test files.

⚠ **7 were the propagation trap** — `if (is_err_result(r) == 1) { return r; }` returns the payload
alone under the value form, so an `Err` reaches the caller as `is_err_result == 0`. Six in
`engine_runner.cyr`, one in `migration.cyr`. All re-wrapped as `return Err(r_v);`.

### Fixed — the step-timeout path lost the handler's payload and aliased success

`_run_attempt` ran the handler on a worker thread and passed the result back through a one-slot
channel, then signalled a timeout with a bare `0`. Both halves broke under the value form:

- `chan_send(chan, r)` after a single-variable bind sent the **tag** and silently dropped the
  payload — every timed-out-capable step lost its output.
- `0` as the timeout sentinel became **indistinguishable from `Ok`**, whose tag is `0` (measured).
  `if (r == 0)` would have read every success as a timeout.

Fixed by restoring the pre-flip contract explicitly: `_run_attempt` returns a **boxed** Result
(`boxed_new`) or `0`, which is exactly the shape the callers already assumed when a Result was a
pointer. `0` is unambiguous again regardless of payload — including a legitimate `Ok(0)`.

### Fixed — `callptr` cannot be multi-value destructured

`var t, v = callptr(...)` is rejected (*"multi-value destructure needs a call on the right-hand
side"*), so `migration.cyr`'s migration-hook call recovers the pair with **`rethi()`**, the
documented intrinsic that reads `rdx` from the last call. Verified on both the Ok and Err arms.

### Fixed — `sigil` was declared without `ct` / `keccak`

The vendored `lib/sigil.cyr` references `ct_*`, `shake256` and `_keccak_*` **30 times and defines
none**, but neither module was in `[deps] stdlib`. Pre-existing — it only surfaced when one call
became reachable and turned a warning into `refusing to emit binary with 1 reachable undefined
function(s)`, failing **43 of 46** test binaries while the main build stayed green.

## [2.1.1] — 2026-08-26

Toolchain and vendored-dependency refresh: Cyrius 6.5.2 → 6.5.35, majra 2.5.3 → 2.7.0, bote-core
3.1.4 → 3.3.7, ai-hwaccel 2.3.15 → 2.3.19 — every dependency now at its latest release, and szal
still has **zero git deps**. The bump surfaced a live Cyrius codegen regression that silently broke
step timeouts, and a cross-arch symbol collision that no previous scan could see. Full suite:
**1,437 assertions across 46 test files, 5 fuzz harnesses, 15 benchmarks, 0 failures.**
`rust-old/` parity oracle untouched.

### Fixed
- **Step timeouts silently inverted under Cyrius ≥ 6.5.31** — `cycc` sign-extends enum-constant
  initialisers from **bit 62**, so `enum StepSat { STEP_I64_MAX = 0x7FFFFFFFFFFFFFFF; }`
  (`src/step.cyr`) folded to **-1**. Because Cyrius `>`/`>=` are signed, that inverted two guards
  at once: `engine_step_exec.cyr`'s `timeout_ms >= STEP_I64_MAX` sent **every** step down the
  synchronous no-timeout path, and `engine_runner.cyr`'s unbounded-flow sentinel made the deadline
  check `elapsed > timeout_ms` **always true**, so every step was skipped as "flow timeout
  exceeded". `STEP_I64_MAX` is now a `var`, whose initialisers are immune to the bad fold; it is
  only ever used as a value, so the kind change is behaviour-preserving. Five suites
  (`engine_runner`, `engine_subflow`, `engine_step_exec`, `engine_hardware`,
  `engine_parallel_stress`) and `fuzz/step_json` failed on this and are green again. Upstream bug
  + minimal repro:
  [`docs/development/issues/archive/2026-08-26-cycc-enum-bit62-sign-extension.md`](docs/development/issues/archive/2026-08-26-cycc-enum-bit62-sign-extension.md).
  **Do not convert `STEP_I64_MAX` back to an enum until cycc is fixed.**

- **`SYS_GETRANDOM` cross-kind collision (latent, non-x86_64)** — vendored majra declares
  `var SYS_GETRANDOM = 318`, an x86_64-hardcoded literal, while the stdlib declares the same name
  as an **arch-conditional enum constant** (318 x86_64-linux/macos, 278 aarch64-linux, 45 agnos).
  `main.cyr` includes `lib/syscalls.cyr` before `src/vendor/majra.cyr`, so last-definition-wins
  handed majra's 318 to the whole program — including `lib/patra.cyr` and `lib/sigil.cyr`, both of
  which szal reaches through `sql_store.cyr`. Harmless on x86_64 (318 == 318), the wrong syscall
  anywhere else. Now renamed `MJ_SYS_GETRANDOM` by `scripts/sync-majra.sh`, so the stdlib's
  arch-correct constant survives. This is a `var`-vs-enum-constant collision whose values *match*
  on the CI arch — a class `cyrius build --strict` reports nothing for.

### Added
- **`scripts/scan-collisions.sh`** — a cross-kind global-symbol collision scanner, with a
  `--check` mode for CI. Cyrius resolves fns, top-level `var`s/`const`s and enum constants in one
  flat namespace (last-definition-wins), but `--strict` is blind to most of that: `fn` vs `var`,
  `fn` vs enum-constant, `struct` vs `struct`, enum-type vs enum-type, and *every* value conflict
  whose prior definition sits past var-table index 1024 — which is over half of szal's globals.
  The `fn` vs data class is not merely unreported but **miscompiles**: `&X` binds to the data
  symbol, so a function pointer taken on that name jumps into `.bss`, and szal dispatches 54 MCP
  tools through function pointers. The scanner is validated against ground truth (its symbol
  counts match `grep` exactly) and against a positive control (it flags `SYS_GETRANDOM` with the
  rename reverted). Current state: **3 intersections, all intentional or verified-benign**
  (`REQ_NONE`, the deliberately shared szal × ai-hwaccel hardware-requirement API; and the
  `StepStatus` / `TriggerMode` enum *type* names, which Cyrius does not place in the flat symbol
  table).

### Changed
- **Cyrius pin 6.5.2 → 6.5.35** (`cyrius.cyml [package].cyrius`). `lib/` re-provisioned from the
  new snapshot (`rm -rf lib && cyrius lib sync`, 55 modules). Static data in the main binary
  dropped **13,414,112 → 806,176 bytes**.
- **majra 2.5.3 → 2.7.0** (`src/vendor/majra.cyr`, 3,289 → 4,840 lines). All 25 symbols szal links
  are signature-identical. ⚠️ **One live behavioural change: rate limiting starts working.**
  majra ≤ 2.5.3 keyed its buckets on the *caller's key pointer*; 2.7.0 stores an owned copy. Every
  szal call site passes a freshly allocated cstr per request (`mcp_tools_net.cyr`'s host/DNS/port
  checks, `mcp_tenant.cyr`'s tenant check), so each call previously got a brand-new full-burst
  bucket and **never refused** — szal's HTTP 10/s·50, DNS 100/s·200, port 50/s·100 and tenant
  100/s·500 limits were no-ops in production. They now enforce, which restores Rust parity. The
  test suite is unaffected (it uses string literals, one pooled address per call site). Also
  fixes an i64-overflow fail-open, fractional-credit starvation, and a `ratelimit_evict_stale`
  leak; `mq_dequeue`/`fleet_rebalance` gained correctness fixes szal benefits from.
- **bote-core 3.1.4 → 3.3.7** (`src/vendor/bote-core.cyr`, 2,612 → 2,881 lines). All 13 symbols
  szal uses are signature-identical; no function removals, no JSON-shape changes. The two
  declared-breaking changes in that range do not reach szal: 3.3.5's `cancel_token_*` prefixing is
  in `stream.cyr` (not in the `[lib.core]` cut szal vendors), and 3.3.0's `Dispatcher` 72 → 88 byte
  growth appends fields szal never offsets into. `compiled_compile` remains the only rename needed.
- **ai-hwaccel 2.3.15 → 2.3.19** (`src/vendor/ai-hwaccel.cyr`, 6,348 → 6,401 lines). Still no
  renames. The `REQ_*` / `FAMILY_*` value table is **byte-identical**, so szal's hardware gating is
  unchanged. 2.3.19 moved its JSON calls to bayan's canonical `bayan_json_v_*` names, which
  **clears the long-standing `json_v_parse_str` undefined-function warning** — down from six such
  warnings to five (`argc`/`argv` from ai-hwaccel's unreachable arg helper, and
  `_keccak_absorb`/`_keccak_f1600`/`shake256`, which are stdlib `lib/sigil.cyr` references to a
  `lib/keccak.cyr` szal does not include; all five remain unreachable from szal).
- `src/*.cyr` reformatted for the 6.5.35 formatter (continuation-line indent — 468 lines across 30
  files, whitespace-only, verified with `diff -w -B`). Same class of change as the 6.5.2 reformat
  at 2.1.0.

## [2.1.0] — 2026-07-29

Toolchain and vendored-dependency refresh for the Cyrius port, plus the first real fuzz and
benchmark coverage the port has had — which immediately paid for itself by surfacing a latent
deadlock in parallel execution. The engine's observable behaviour is unchanged **except** for that
deadlock fix. Full suite: 1,437 assertions across 46 test files, 5 fuzz harnesses, 15 benchmarks.
`rust-old/` parity oracle untouched.

### Fixed
- **Intermittent deadlock in parallel execution** — `run_parallel` hung forever roughly once per
  2,000 parallel `engine_run` calls (~1 per 150,000 worker joins), taking `run_dag` and
  `run_distributed` with it. Root cause is upstream, in `lib/thread.cyr`'s `thread_join`: it loads
  the thread's tid word **twice** — once for the loop condition, once for the `FUTEX_WAIT`
  expected-value — and a worker exiting between the two loads makes the joiner park on a wake that
  already fired. Confirmed from `/proc/<pid>/syscall` on a hung process: `SYS_FUTEX`, op
  `FUTEX_WAIT` with no `FUTEX_PRIVATE_FLAG` (the only non-private waiter szal can reach is
  `thread_join`), expected value `0` (an impossible live tid). `lib/` is re-provisioned by
  `cyrius lib sync`, so the fix is szal-side: the new **`szal_thread_join`**
  (`src/engine_step_exec.cyr`) loads the tid once per iteration, and all four szal join sites —
  `engine_step_exec` / `engine_parallel` / `engine_dag` / `engine_distributed` — now use it. No szal
  code calls `thread_join` directly. Full analysis + suggested upstream patch:
  [`docs/development/issues/archive/2026-07-29-thread-join-lost-wakeup-deadlock.md`](docs/development/issues/archive/2026-07-29-thread-join-lost-wakeup-deadlock.md)

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

### Added
- **Real fuzz coverage — 5 property harnesses under `fuzz/`** (~356k properties/run, ~1.6s), sharing
  a seeded deterministic PRNG prelude (`fuzz/fuzz_util.cyr`) so any failure replays exactly from the
  printed seed: `condition_expr` (tokenizer/parser/evaluator/cache/templates, checked against
  algebraic laws — De Morgan, precedence, double negation, idempotence), `flow_validate` (cycle
  detection **differentially checked against an independent Kahn peel-off**, plus mode/trigger/
  hierarchical rules and flow JSON round-trip), `step_json` (StepDef/StepResult round-trip incl.
  recursive `sub_steps`, enum wire forms, saturating backoff math), `state_json` (transitions and the
  Display-vs-serde split, exhaustive), `hash_uuid` (RFC 1321 / RFC 4122 vectors and round-trips).
  Validated by mutation testing: 15 deliberately-injected bugs, 15 caught. Replaces
  `tests/szal.fcyr`, a stub with no `include` lines that had never compiled
- **Real benchmark coverage — `benches/bench_all.bcyr`**, 15 benchmarks covering the 14 names
  `benchmarks/history.csv` has tracked since v1.0.1. Replaces `tests/szal.bcyr`, a stub that had
  never compiled and called a `bench()` that does not exist in `lib/bench.cyr`; it also sat in the
  wrong directory, which `port-plan.md` §1.9 marks as silently ignored. `scripts/bench-history.sh`
  now builds and runs the Cyrius harness instead of `cargo bench`/criterion, and parses
  machine-readable `BENCHDATA <name> <avg> <min> <max> <iters>` lines rather than `lib/bench.cyr`'s
  `bench_report` text, whose `_fmt_time` renders bare integer microseconds at the pinned toolchain
  ("1us" for 1481ns — a 48% error that has flat-lined this history once before). Adds `--dry-run`
- **`tests/szal_engine_parallel_stress.tcyr`** — parallel-executor liveness stress (46th suite,
  ~10s, 3 assertions). The deadlock above went unnoticed because the existing parallel suites run a
  handful of flows each, three orders of magnitude below the frequency needed to hit it. Three
  phases: 3,000 `engine_run` calls on a 10-step `FLOW_PARALLEL` flow, 500 `run_parallel` calls with
  a contended permit semaphore (2 permits / 12 steps), and 96,000 spawn-signal-join rounds against
  the join primitive from 24 concurrent drivers. Built against the pre-fix tree it deadlocks on
  **12 of 12** runs; post-fix it passed **12 of 12**. Carries its own watchdog thread that fails the
  process (`SYS_EXIT_GROUP`, exit 1) with a diagnostic after 120s, since a deadlock is a hang rather
  than a failed assertion

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
- **CI gates the new harnesses, and can no longer silently skip them.** Every harness is named
  explicitly in a `Verify harness manifest` step rather than only globbed — a glob can prove that
  what it found passes, but never that something is missing, which is exactly how the two stubs hid.
  The suite and fuzz steps get count floors (≥ 40 suites, ≥ 5 harnesses), the fuzz step builds
  `--strict`, its skip-cleanly branch is gone, and a `Benchmarks` step runs `bench-history.sh
  --dry-run`. Each suite is wrapped in `timeout 300` so a threading regression fails the job in
  minutes instead of stalling it until GitHub's 6-hour ceiling

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
