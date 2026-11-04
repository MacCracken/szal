# bote reads the first of two duplicate JSON keys in a request; the stdlib refuses them (upstream ask)

**Status:** 🟡 open — a request for bote, **drafted here, not yet filed upstream**. szal's own side is
fixed at 2.3.1 (ADR 0003): a tool whose arguments bayan refuses returns a Validation error.
**Severity:** low for szal (it now fails closed); medium for the stack — bote's schema check and a
handler can read different values for one argument.
**Measured on:** bote-core 3.4.10 (cyrius 6.7.9) and 3.8.4 (cyrius 6.7.18), vendored byte-for-byte in
szal; requests driven through `codec_process_message` on a `dispatcher_new` dispatcher.

## What happens

bote's request reader (`jsonx`) takes the **first** of two duplicate object keys:

| request | bote 3.4.10 and 3.8.4 |
|---|---|
| `tools/call` with `params` `{"name":"nope","name":"szal_json_validate",…}` | dispatches `nope` → `-32602 tool not found: nope` |
| `szal_uuid` with `arguments` `{"count":"x","count":3}` | inputSchema check fails on the first value: `count: expected integer` |
| `szal_uuid` with `arguments` `{"count":3,"count":"x"}` | inputSchema check passes on `3`; the handler receives the raw text, both pairs |

The stdlib's bayan, from 1.11.1 (cyrius 6.7.18), **refuses** such a document ("duplicate key", the
byte of the second), on the stated ground that a validator and an actor disagreeing on a repeated
key is exploitable (`{"role":"user","role":"admin"}`). And `serde_json`, which most clients' servers
use, keeps the **last** value. Three readers, three answers.

In bote's own dispatcher the inputSchema check reads the first value while the handler is handed the
raw `arguments` text — so a handler that parses with a last-wins reader acts on a value nobody
validated, and one that parses with bayan (szal ≤ 2.3.0, which read a refused parse as `{}`) runs on
its defaults. szal 2.3.1 refuses in the handler; other bote consumers may not.

## The ask

Refuse a request whose JSON repeats a key, before dispatch — a JSON-RPC `-32700` parse error (the
request is not one value under bayan's rules), or `-32602` naming the key — so the envelope, the
schema check and every handler see one document. bayan's `_jk_*` key set (bayan 1.11.1) is a
measured O(n) way to detect it.

## For szal when bote ships it

- Re-sync bote-core, and update `tests/szal_consumer_bundle.tcyr` `t_refused_args`: those requests
  would then fail at the protocol level, not with szal's `invalid arguments: duplicate key …`.
- Keep `szal_args_parse` / `szal_args_error`: they make a handler fail closed on any argument text
  bayan refuses, whatever the dispatcher does.
- parity-notes §25 / ADR 0003: note where the refusal moved.

## Closes when

bote refuses duplicate keys in requests (or decides not to, with its reason recorded here).
