# szal — Claude Code Instructions

> **Core rule**: this file is **preferences, process, and procedures** —
> durable rules that change rarely. Volatile state (current version,
> module line counts, port progress, test counts, consumers) lives in
> [`docs/development/state.md`](docs/development/state.md).
> Do not inline state here.

## Project Identity

**szal** — Cyrius port of a Rust project (13172 lines preserved at `rust-old/`).

- **Type**: Port (Rust → Cyrius)
- **License**: GPL-3.0-only
- **Language**: Cyrius (toolchain pinned in `cyrius.cyml [package].cyrius`)
- **Version**: `VERSION` at the project root is the source of truth — do not inline the number here
- **Standards**: [First-Party Standards](https://github.com/MacCracken/agnosticos/blob/main/docs/development/applications/first-party-standards.md) · [First-Party Documentation](https://github.com/MacCracken/agnosticos/blob/main/docs/development/applications/first-party-documentation.md)

## Goal

_TODO: one-or-two-sentence mission statement. What does szal OWN in the stack? Durable — doesn't change per release._

## Current State

> Volatile state lives in [`docs/development/state.md`](docs/development/state.md) —
> port progress, surface parity, in-flight work. Refreshed every release.

This file (`CLAUDE.md`) is durable rules.

## Scaffolding

Project was scaffolded with `cyrius port`. Original Rust at `rust-old/` is the reference oracle — do not modify it; cross-check the port against it.

## Quick Start

```sh
cyrius deps                              # resolve dependencies
cyrius build src/main.cyr build/szal    # compile
cyrius test                              # run tests/*.tcyr
```

## Key Principles

- **Cross-check against `rust-old/`** — the port's correctness bar is "matches what Rust did". Diverge only with an ADR.
- **Correctness over cleverness** — if the Cyrius behavior diverges silently from Rust, the bugs win
- Test after every change, not after the feature is "done"
- ONE change at a time — never bundle unrelated changes
- Build with `cyrius build`, not raw `cat file | cc5` — the manifest auto-resolves deps
- Source files only need project includes — stdlib auto-resolves from `cyrius.cyml`
- `var buf[N]` = N **bytes**, not N entries
- **szal owns its namespace** ([ADR 0002](docs/adr/0002-szal-owns-its-namespace.md)) — Cyrius has one flat namespace, last definition wins. New public symbols are `szal_`/`SZAL_`-prefixed (or carry their module's own prefix); never declare a name another szal file already declares; on a clash with a vendored library, rename szal's side. `scripts/scan-collisions.sh --check` is the gate — `cyrius build --strict` is blind to most collision kinds

## Rules (Hard Constraints)

- **Do not commit or push** — the user handles all git operations
- **Never use `gh` CLI** — use `curl` to the GitHub API if needed
- Do not modify `rust-old/` — it's the parity oracle
- Do not skip tests before claiming changes work
- Do not modify `lib/` files (vendored stdlib / dep symlinks)
- Do not edit `src/vendor/*.cyr` — they are byte-identical release dists; re-sync with `scripts/sync-{majra,bote,ai-hwaccel}.sh` (from the release tag)
- Do not edit `dist/` by hand — regenerate with `cyrius distlib mcp` (CI fails on any drift)
- Do not hardcode toolchain versions in CI YAML — `cyrius = "X.Y.Z"` in `cyrius.cyml` is the source of truth

## Documentation

- [`docs/adr/`](docs/adr/) — Architecture Decision Records (*why X over Y?*)
- [`docs/architecture/`](docs/architecture/) — Non-obvious constraints
- [`docs/guides/`](docs/guides/) — Task-oriented how-tos
- [`docs/examples/`](docs/examples/) — Runnable examples
- [`docs/development/state.md`](docs/development/state.md) — Live state (start here for a handoff)
- [`docs/development/roadmap.md`](docs/development/roadmap.md) — Milestones through v1.0
- [`docs/development/port-plan.md`](docs/development/port-plan.md) — Authoritative porting brief (per-module API, byte layouts, dep mapping) — read before porting any module
- [`docs/development/parity-notes.md`](docs/development/parity-notes.md) — Accepted Rust→Cyrius divergences + the audit disposition log
- [`docs/development/majra-vendoring.md`](docs/development/majra-vendoring.md) — vendored-library maintenance record (majra / bote-core / ai-hwaccel re-sync, collision history)
- [`docs/development/issues/`](docs/development/issues/) — open issues; a closed issue gets its resolution written at the top and moves to `issues/archive/`

