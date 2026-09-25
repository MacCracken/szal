# hoosh cannot consume szal's MCP tools — no dist bundle, and szal's own names collide with upstream majra / bote-core

**Status:** Open.
**Filed:** 2026-09-25 by hoosh (2.7.1, AI inference gateway) while retiring hoosh's `rust-old/`.
**Severity:** Medium. It blocks hoosh's `/v1/tools/list` and `/v1/tools/call` from carrying szal's
tools. hoosh's Rust release mounted them through the szal crate; the Cyrius gateway serves only a
`bote_echo` smoke tool until this lands.
**Affects:** any Cyrius consumer that vendors **upstream** majra and bote-core. hoosh does, and daimon
and sutra, szal's other planned consumers, are likely to as well.
**Repos:** szal `2.1.2` · hoosh `2.7.1`. hoosh vendors bote-core `3.3.13` (`src/vendor/bote-core.cyr`),
majra `2.9.1` (`src/vendor/majra.cyr`) and ai-hwaccel `2.4.0` (`lib/ai-hwaccel.cyr`).

## Summary

The port itself is done. `all_tools()` in `src/mcp_tools_net.cyr` returns the 54 built-in tools, and
`register_tools_with` wires them into a bote dispatcher, which is the same bote API hoosh already uses.
Three things stand between that and a consumer:

1. **No bundle.** The roadmap's M5 item "`dist/szal.cyr` + `dist/szal-core.cyr` committed" is still
   open, and `cyrius.cyml` defers its `[lib]` lists to M5.
2. **Collisions are fixed on the wrong side for a consumer.** szal keeps its own build clean by
   renaming the *vendored* copies: `scripts/sync-bote.sh` turns `compiled_compile` into
   `bote_compiled_compile`, and `scripts/sync-majra.sh` rewrites `STEP_*`, `TRIGGER_*`, `ERR_*`,
   `uuid_generate`, `step_result_new` and `SYS_GETRANDOM`. A consumer's upstream copies carry none of
   those renames. A bundle cannot ship szal's renamed copies either, because the consumer already has
   its own, newer copies of the same libraries.
3. **The registration API assumes szal owns the dispatcher.** `szal_register_tools()` builds a new one.
   hoosh already has a dispatcher (`mcp_init`: `bote_echo`, plus its own audit chain and event bus) and
   needs to add szal's tools to it.

## Measured collisions

Every top-level `fn`, `var` and enum constant in szal `src/*.cyr` (945 names) was compared with
hoosh 2.7.1's whole compile set: `src/`, the vendored bote-core and majra, `lib/ai-hwaccel.cyr`, and the
stdlib modules in hoosh's `cyrius.cyml` (7,730 names). `main` and `r` are excluded as program entry
points.

| Symbol | szal | Consumer side | Effect |
|---|---|---|---|
| `STEP_COMPLETED` | `src/bus.cyr:25` = **5** | majra 2.9.1 = **2** | Different value; last-definition-wins silently changes one side's meaning |
| `STEP_FAILED` | `src/bus.cyr:26` = **6** | majra = **3** | Different value |
| `STEP_SKIPPED` | `src/bus.cyr:29` = **9** | majra = **4** | Different value |
| `TRIGGER_ALL` / `TRIGGER_ANY` | `src/step.cyr:27-28` = 0 / 1 | majra = 0 / 1 | Same values; still a duplicate |
| `step_result_new` | `src/step.cyr:442`, **6 args** | majra, **3 args** | Different arity; a hard error since cyrius 6.6.2 |
| `uuid_generate` | `src/uuid.cyr:67` | majra `:459` | Two implementations, one survives |
| `compiled_compile` | `src/condition.cyr:683`, condition compiler | bote-core 3.3.13 `:2471`, schema compiler | Unrelated functions, one survives |
| `cache_new` | `src/condition.cyr:737`, **0 args** | hoosh `src/lib/cache.cyr:29`, **3 args** | Different arity; hard error |

`stdlib` `result` and `log`, which szal needs and hoosh does not yet list, collide with nothing in
hoosh. This scan is a plain name match. `scripts/scan-collisions.sh` covers more (cross-kind, and var
table index past 1024) and should be re-run against this set.

## What would make it consumable

1. **Rename szal's own symbols, not the vendored copies.** Then szal can vendor upstream majra and
   bote-core unmodified, and so can every consumer. The Consumer contract in `../roadmap.md` already
   asks for this ("prefix everything `szal_`/`flow_`/`step_`"). `step_` is not enough here, because
   majra owns `step_result_new`, so these need `szal_` / `SZAL_`:
   - `SZAL_STEP_COMPLETED`, `SZAL_STEP_FAILED`, `SZAL_STEP_SKIPPED`
   - `SZAL_TRIGGER_ALL`, `SZAL_TRIGGER_ANY`
   - `szal_step_result_new`, `szal_uuid_generate`, `szal_condition_compile`,
     `szal_condition_cache_new`

   The same contract says daimon owns the `mcp_` prefix, and `src/mcp.cyr` exports `mcp_err_name`,
   `mcp_err_retryable`, `mcp_tool_def`, `mcp_tool_new`, `mcp_tool_def_of` and `mcp_tool_handler_of`.
   Generic names such as `register_tools`, `register_tools_with`, `all_tools`, `result_ok`,
   `result_error`, `result_ok_json`, `result_error_typed` and `validate_path` are a collision waiting
   for the next consumer.
2. **A bundle without the vendored libraries.** A `dist/szal-mcp.cyr` (or `szal-core`) in the shape of
   bote's core profile: szal's own modules that `all_tools()` needs, and **not** bote-core, majra,
   ai-hwaccel or the stdlib. The consumer supplies those. Ship a `.deps` file with the minimum versions
   and the stdlib modules the closure needs (at least `result`, `log`, `bayan`, and `patra` if any tool
   reaches `sql_store`). Say whether the tool closure reaches `engine_hardware`, and so ai-hwaccel:
   szal vendors ai-hwaccel 2.3.19 and hoosh has 2.4.0.
3. **Register into the consumer's dispatcher.** Something like `szal_all_tools()` (the vec) plus
   `szal_register_into(registry, dispatcher)`, so a consumer adds the 54 tools next to its own and keeps
   its own audit and event sinks. `szal_register_tools()` can stay as the convenience entry point.

## Acceptance

- A program that includes hoosh's set (upstream bote-core 3.3.13, majra 2.9.1, ai-hwaccel 2.4.0, and
  hoosh's stdlib list plus `result` and `log`) and then `dist/szal-mcp.cyr` builds with
  `cyrius build --strict` and prints no `duplicate fn` warning.
- `scripts/scan-collisions.sh`, pointed at that set, reports nothing.
- `szal_register_into` puts 54 tools into an existing bote dispatcher that already holds another tool,
  and `tools/list` returns 55.

## Consumer side (hoosh)

hoosh vendors the bundle at `src/vendor/szal-mcp.cyr` (re-synced by script), and calls
`szal_register_into` from `mcp_init` next to `bote_echo`. `/v1/tools/*` needs no transport changes.
Until then the gap is recorded in hoosh's `docs/development/rust-old-retirement.md` and its roadmap.
