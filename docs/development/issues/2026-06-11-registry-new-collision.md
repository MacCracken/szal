# `registry_new` symbol collision — bote-core × ai-hwaccel

**Filed:** 2026-06-11 during the szal Rust→Cyrius port (M2 engine arc)
**Severity:** Medium — **blocks szal `engine_hardware` (port-plan §4 row 17)**;
the rest of M2 shipped around it. Tracked as a **P2 blocker-from-completion** in
[`../roadmap.md`](../roadmap.md) (M2 cannot close until this resolves).
**Affects:** any Cyrius consumer that includes BOTH `dist/bote-core.cyr` (or
`dist/bote.cyr`) and `dist/ai-hwaccel.cyr` in one compile unit — szal is the first.
**Repos:** bote `2.7.3` · ai-hwaccel `2.3.9` (issue also filed in both repos' `docs/development/issues/`).

## Summary

Two ecosystem libraries export a **public function with the same name but
different identity**:

| Library | Symbol | Shape | Source |
|---|---|---|---|
| bote-core 2.7.3 | `fn registry_new()` | **24-byte** tool registry `{entries map@0, versions map@8, names vec@16}` | `src/registry.cyr:148` (→ `dist/bote-core.cyr:554`, `dist/bote.cyr:553`) |
| ai-hwaccel 2.3.9 | `fn registry_new()` | **32-byte** profile registry (`REGISTRY_SIZE=32`: `{profiles, warnings, system_io, schema}`) | `src/registry.cyr:20` (→ `dist/ai-hwaccel.cyr:3549`) |

Cyrius include semantics are textual paste + **last-definition-wins (with a
warning)**. A consumer that includes both bundles gets exactly ONE `registry_new`
— whichever is included last — and every caller of the other one silently
allocates/interprets the wrong struct layout (24 vs 32 bytes), corrupting memory.

## Why include order can't fix it

ai-hwaccel's own detection path calls `registry_new()` **internally**:
`registry_detect*` (`src/registry.cyr:270`, `:350`), `src/lazy.cyr:157`,
`src/async_detect.cyr:117`. So even if the consumer never calls ai-hwaccel's
`registry_new` directly, ai-hwaccel's detection API does — and if bote-core's
24-byte `registry_new` won the link, ai-hwaccel's `cached_registry_new` /
`registry_detect` build a 24-byte blob and then write profile fields past its
end. There is no include order that gives both libraries a correct
`registry_new`.

## szal's need

szal needs BOTH surfaces in `dist/szal.cyr`:
- bote-core's `ToolRegistry` (`registry_new`/`registry_register`/`registry_get`)
  for the MCP tool dispatcher (M3 `mcp.cyr`).
- ai-hwaccel's `HardwareContext` over `cached_registry_new(300)` /
  `registry_detect` for `engine_hardware.cyr` (M2 row 17).

These collide, so `engine_hardware` cannot be ported until this is resolved.

## Resolution options

1. **Upstream rename in ai-hwaccel (CLEANEST, preferred).** Rename
   ai-hwaccel's `registry_new` → `hw_registry_new` (and the `reg_*`/
   `registry_detect*` family is already `hw`-ish; only `registry_new` collides
   with bote's tool-registry namespace). ai-hwaccel's registry is hardware-profile
   specific — an `hw_` prefix is the natural namespace. Other consumers that pair
   bote + ai-hwaccel (mihi, hoosh) benefit too. **Owner: ai-hwaccel.** Suggested
   for ai-hwaccel 2.4.0.
2. **Local sed-rename in szal's vendored/synced copy.** If szal vendors
   ai-hwaccel (like the 9-symbol majra rename in
   [`../majra-vendoring.md`](../majra-vendoring.md)), apply
   `registry_new`→`hw_registry_new` in `src/vendor/ai-hwaccel.cyr` via a
   `scripts/sync-ai-hwaccel.sh`. Interim, szal-only; carries re-sync maintenance.
3. **bote rename.** Less natural — `registry_new` is bote's long-standing tool
   registry name and many bote consumers depend on it; renaming there has wider
   blast radius than renaming ai-hwaccel's profile registry. Not recommended.

**Recommended:** option 1 (ai-hwaccel 2.4.0 upstream rename); option 2 as the
interim unblock for szal if 2.4.0 is not imminent.

## Cross-references

- szal port-plan §3.3 (the collision was flagged pre-port as "Open Q9").
- szal roadmap.md M2 (row 17 `engine_hardware`, P2 blocker-from-completion).
- The majra vendoring rename ([`../majra-vendoring.md`](../majra-vendoring.md)) is
  the established precedent for option 2.
