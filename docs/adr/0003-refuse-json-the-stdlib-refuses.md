# ADR 0003 — Tool arguments the stdlib refuses are a tool error, not `{}` and not Rust's last-wins

- **Status**: Accepted
- **Date**: 2026-11-03
- **Release**: 2.3.1

## Context

cyrius 6.7.18 ships bayan 1.11.1, whose JSON parsers refuse a document with a **duplicate object
key** or a **malformed number** (`01`). bayan's reason is a security one: when two readers disagree
on a repeated key (`{"role":"user","role":"admin"}`), a component that validates a document and
one that acts on it can see different values.

szal's MCP tools parse their arguments with bayan. Measured at 2.3.1 through bote 3.8.4's dispatcher
(the path hoosh uses), with the 2.3.0 handlers:

- **bote reads the FIRST of two duplicate keys** — in the JSON-RPC envelope (`params.name` given
  twice dispatches the first) and in the inputSchema check it runs before the handler (3.4.8+). It
  then hands the handler the raw `arguments` text, duplicates intact. (Same on bote 3.4.10.)
- **szal's handlers read arguments bayan refuses as `{}`.** None of the 45 argument-reading handlers
  checked the parse. A tool with optional arguments therefore ran on its defaults and reported
  success: `szal_uuid {count:3,count:3}` answered one UUID, `szal_random_token {length:4,length:4}`
  a 32-byte token, `szal_uuid {count:3,count:"x"}` one UUID after bote had validated `3`. A tool with
  a required argument answered `missing required field: …`, which misnames the problem.
- **Rust** (the parity oracle) reads arguments as a `serde_json::Value`, which keeps the LAST of two
  duplicate keys.

Before 6.7.18 the case could not arise: bote never let malformed JSON through, and bayan accepted
duplicates (answering the first, as bote does).

## Decision

**A handler whose arguments the stdlib refuses returns a Validation error naming bayan's reason and
byte, before reading any argument.** `szal_args_parse(args)` (src/mcp.cyr) parses; on 0 the handler
returns `szal_args_error(args)`: `invalid arguments: duplicate key at byte 11`, `isError: true`,
`_meta.error_code: "validation"`. The reason is read from a parse state of szal's own (bayan's
`_ctx` API), not bayan's shared last-error globals, because bote may run handlers on worker threads.
The nine tools that take no arguments keep ignoring them, as Rust's do.

## Consequences

- **Positive** — no tool acts on arguments it could not read, and none acts on a value bote's schema
  check did not see: with bote reading first-wins, refusing is the only answer that keeps the
  validator and the actor agreeing. The error says what is wrong. `tests/szal_mcp_tools_net.tcyr`
  sweeps all 54 tools (one assertion each, mutation-checked: dropping any one guard names that
  tool), and `tests/szal_consumer_bundle.tcyr` drives the original reproductions through bote.
- **Negative** — a deliberate divergence from Rust: a request whose arguments repeat a key, which
  Rust serves using the last value, is refused. parity-notes §25 records it with the measured
  matrix. It also diverges from bote's own first-wins reading, which szal cannot change (bote-core
  is vendored byte-for-byte, ADR 0002); the bote-side ask is
  `docs/development/issues/2026-11-03-bote-request-reader-first-wins-duplicate-keys.md`.
- **Neutral** — JSON a tool is handed to inspect (`szal_json_validate`, `_path`, `_diff`) and
  step / flow / record JSON were already refused explicitly when they do not parse; nothing changes
  there. For step / flow JSON the refusal matches Rust, whose derived structs reject a duplicate
  field.

## Alternatives considered

- **Restore Rust's last-wins in szal** — rejected. bayan has no option for it, so szal would need a
  JSON parser of its own on the tool boundary; and acting on the last value would act on one bote's
  schema check never saw (`{count:3,count:"x"}`: bote validates `3`), the exact disagreement bayan's
  refusal exists to prevent.
- **Take the first value, as bote does** — rejected: also needs a parser of szal's own, matches
  neither Rust nor the stdlib, and silently drops part of what the client sent.
- **Leave it** — rejected: tools reported success while ignoring the caller's arguments.
- **Make bote refuse duplicates** — right, and asked for (the issue above); it would turn these
  requests into a protocol error before any handler runs. szal's guard stays either way: it is what
  makes a handler fail closed on any argument text bayan refuses.
