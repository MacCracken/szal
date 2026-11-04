# bote reads the first of two duplicate JSON keys in a request (tracked in bote)

> ## 📦 CLOSED — archived 2026-11-04 at szal 2.3.2
>
> bote fixed it at **3.8.6** (its issue is archived there with a Resolution) and szal 2.3.2 vendors
> **3.8.9**. Re-verified through `codec_process_message` on the 3.8.9 dist: all three requests below
> — `params.name` twice, `{"count":"x","count":3}`, `{"count":3,"count":"x"}` — are refused with
> `-32600 invalid request: a key named twice, or one that is no text`, and no handler runs. The
> "When bote ships it" steps are done: re-vendored; `tests/szal_consumer_bundle.tcyr` `t_refused_args`
> now expects bote's -32600 for repeated keys and keeps szal's guard covered through the dispatcher
> with what bote still passes and bayan refuses (an integer past i64 under a key the schema does not
> name: `invalid arguments: integer is out of range for i64 at byte 16`); `szal_args_parse` /
> `szal_args_error` stay; parity-notes §25 and ADR 0003 updated. What is below is the note as filed.

**Status (as filed):** 🟡 open — **the issue lives in bote's repo**:
`bote/docs/development/issues/2026-11-03-request-duplicate-keys-read-first-wins.md` (filed 2026-11-03
against bote 3.8.5). This file is szal's side only. szal's own fix shipped at 2.3.1 (ADR 0003): a tool
whose arguments bayan refuses returns a Validation error, so szal fails closed whatever bote does.

## In short

bote's key scanner (`_jx_scan`) takes the FIRST of two duplicate keys — in the JSON-RPC envelope and in
the inputSchema check — and hands the handler the raw `arguments` text. The stdlib's bayan (1.11.1,
cyrius 6.7.18) refuses such a document, and serde-style readers keep the LAST value. Measured through
`codec_process_message` on bote 3.4.10, 3.8.4 and 3.8.5: `szal_uuid {"count":3,"count":"x"}` passes
bote's schema check on `3` and reaches szal's handler. The ask in bote's issue: refuse duplicate keys
in a request before dispatch (`-32700`, or `-32602` naming the key).

## When bote ships it

- Re-vendor bote-core (`scripts/sync-bote.sh ../bote <tag>`).
- `tests/szal_consumer_bundle.tcyr` `t_refused_args`: the duplicate-key requests become a protocol
  error from bote, not szal's `invalid arguments: duplicate key …` — update the expectations.
- Keep `szal_args_parse` / `szal_args_error`: they make a handler fail closed on any argument text bayan
  refuses, whatever the dispatcher does.
- parity-notes §25 and ADR 0003: note that the refusal now happens in bote.

## Closes when

bote's issue closes (fixed, or declined with a reason) and the steps above are done.
