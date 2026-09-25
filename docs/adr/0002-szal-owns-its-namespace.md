# ADR 0002 — szal owns its namespace; vendored libraries stay byte-identical

- **Status**: Accepted
- **Date**: 2026-09-25
- **Release**: 2.2.0

## Context

Cyrius resolves every top-level `fn`, `var` and enum constant in one flat namespace, and the
last definition wins — silently for most collision kinds (`scripts/scan-collisions.sh` lists
which). szal vendors three libraries at `src/vendor/`: majra (the full dist), bote-core and
ai-hwaccel. Several names collided between them and szal: majra's `StepStatus` members
`STEP_COMPLETED` / `STEP_FAILED` / `STEP_SKIPPED` (with values different from szal's `EventType`
members of the same names), `TRIGGER_ALL` / `TRIGGER_ANY`, `uuid_generate`, `step_result_new`, and
bote's `compiled_compile`.

Through 2.1.2 szal kept its build clean by rewriting the **vendored copies**: `sync-majra.sh`
sed-renamed six families (`MJ_ERR_*`, `MJ_STEP_*`, `MJ_TRIGGER_*`, `majra_uuid_generate`,
`majra_step_result_new`, `MJ_SYS_GETRANDOM`) and `sync-bote.sh` one (`bote_compiled_compile`).
That works for szal alone. It fails the moment another program links szal's code: hoosh filed
(2026-09-25) that it could not consume szal's MCP tools, because hoosh vendors **upstream** majra
and bote-core, a szal bundle cannot ship szal's rewritten copies alongside them, and szal's own
names then collide with the originals. daimon and sutra are expected to vendor the same way.
szal's bare, generic public names (`result_ok`, `validate_path`, `register_tools`, `all_tools`,
`cache_new`, `pool`, `mcp_*`) were a second class of the same problem — hoosh's `cache_new` already
clashed, and daimon owns the `mcp_` prefix.

## Decision

**szal renames its own symbols; vendored libraries are copied byte-for-byte from their release
tags.**

1. Every name that collides with an upstream library szal vendors, or with a measured consumer,
   is renamed on szal's side, together with the rest of its family so the API stays coherent
   (for example all eleven `EventType` members become `SZAL_FLOW_*` / `SZAL_STEP_*`, not just the
   three that clashed). The public entry points of the consumer bundle — result builders, path
   guard, registration, tool-group accessors, rate-limit pool — are `szal_` / `SZAL_`-prefixed.
2. `scripts/sync-{majra,bote,ai-hwaccel}.sh` extract `dist/<lib>.cyr` from the release **tag**
   (`git show <tag>:…`), check the dist's own `# Version:` line against it, and copy it
   unmodified. A consumer's vendored copy and szal's are the same file (`cmp`).
3. `scripts/scan-collisions.sh` is the gate, not renames alone: cross-kind; every name two szal
   files both define; an allow-list whose entries must also agree on value; and a `--consumer`
   mode that checks `dist/szal-mcp.cyr` against a consumer's whole compile set.

## Consequences

- **Positive** — szal compiles against exactly the libraries its consumers link, so a szal bundle
  (`dist/szal-mcp.cyr`) can drop into their programs; a re-sync is a copy with verifiable
  provenance; the scanner now covers szal x szal, which is how the `TOK_LPAREN` / `TOK_RPAREN`
  redeclaration (a live parser bug in every build carrying the math tool) was found.
- **Negative** — the rename changed szal's public Cyrius names at 2.2.0. No external Cyrius code
  called them yet, which is why it is a MINOR bump, but from here on a rename of these names is a
  breaking change. szal's names now differ from the Rust oracle's (`result_ok` ->
  `szal_result_ok`); the oracle's names survive in comments and parity notes. And szal, not the
  upstream, now owns every future collision: when a vendored library grows a name szal already
  has, the next sync fails the scan and szal renames.
- **Neutral** — most szal internals keep their module prefixes (`step_*`, `flow_*`, `cond_*`,
  `state_*`, and some parser internals such as `tok_kind`); nothing measured collides with them.
  The scanner's `--consumer` mode is how a new consumer finds out whether that is still true for
  its compile set, before it vendors.

## Alternatives considered

- **Keep rewriting the vendored copies** — rejected: it is exactly what stopped hoosh. It also hid
  collisions instead of removing them (szal's `STEP_*` shadowed majra's only because the copy had
  been renamed out of the way).
- **Ask the upstreams to rename** — rejected for the names in question: majra's `StepStatus` /
  `TriggerMode` / `uuid_generate` / `step_result_new` and bote's schema compiler are their own,
  correctly-scoped APIs that predate szal's use. (Upstream renames remain right when the upstream
  name is the generic one — majra's `SYS_GETRANDOM`, now gone, and bote's `registry_new`.)
- **Prefix every szal symbol** — rejected for now: roughly a thousand names, most of which collide
  with nothing, and a large diff against the parity oracle for no measured benefit. Measured
  collisions plus the consumer-facing surface are renamed; the scanner guards the rest.
- **A language-level namespace** — Cyrius has none; `private` is per-file visibility, not a way to
  let two libraries each keep a public name.
