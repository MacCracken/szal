#!/usr/bin/env sh
# sync-ai-hwaccel.sh — vendor the ai-hwaccel dist into src/vendor/ai-hwaccel.cyr.
#
# szal needs ai-hwaccel's CachedRegistry + REQ_*/FAMILY_* surface for src/engine_hardware.cyr (row 17,
# hardware-aware scheduling). We VENDOR it (hoosh pattern, like scripts/sync-majra.sh / sync-bote.sh)
# rather than a [deps.ai-hwaccel] block: declared as a git dep, `cyrius build` resolves it on the
# FULL-DEPS path, which on x86_64 mis-resolves szal's `var buf[ENUM_CONST]` idiom (error.cyr fails
# with "array size identifier must be an enum constant"). With ai-hwaccel vendored, szal has ZERO git
# deps -> a clean `cyrius lib sync` + `cyrius build --strict --no-deps` (the project build recipe).
#
# NO symbol rename is applied. As of ai-hwaccel 2.3.19 / szal 2.1.1 its ONLY collision with szal
# core is REQ_NONE — part of the REQ_*/FAMILY_* hardware-requirement API that is intentionally
# SHARED with src/step.cyr. Re-verified at 2.3.19: the REQ_*/FAMILY_* VALUE TABLE is byte-identical
# to 2.3.15 (REQ_NONE..REQ_ANY_ACCELERATOR = 0..6, FAMILY_CPU..FAMILY_AI_ASIC = 0..4) and
# requirement_name()'s string table is unchanged, so szal's hardware gating and its
# `_hw_unavailable_msg` text are unaffected. That value check MATTERS: cycc is last-definition-wins
# and both sides declare REQ_NONE, so silent drift here would silently change szal's gating.
#
# UPSTREAM BREAKAGE FIXED at 2.3.19: `profile_from_json_str` used to call bayan's removed pre-1.3.0
# `json_v_parse_str`; 2.3.16 moved it to `json_v_parse_buf` and 2.3.19 moved the whole family to
# bayan's canonical `bayan_json_v_*` names. szal's build therefore no longer emits
# `warning: undefined function 'json_v_parse_str'` (six such warnings -> five).
# STILL PRESENT, still harmless: an arg-parsing helper calls `argc`/`argv` without szal including
# lib/args.cyr. szal reaches none of it — src/engine_hardware.cyr only calls `cached_registry_new`/
# `cached_get` (plus reg_profiles / count_satisfying / reg_count_by_family / reg_has_accelerator /
# requirement_name). If a future szal change calls into those paths the build will fail to link;
# fix it upstream in ai-hwaccel, not here.
# (The other three undefined-fn warnings — _keccak_absorb / _keccak_f1600 / shake256 — are NOT
# ai-hwaccel's: they are stdlib lib/sigil.cyr references to lib/keccak.cyr, which szal does not
# include.)
#
# 2.3.18 renamed ai-hwaccel's `path_exists` -> `aihw_path_exists`. Inert for szal: szal never calls
# it (its own existence check is lib/io.cyr's `file_exists`), and the stdlib defines no `path_exists`.
#
# After bumping ai-hwaccel, RE-RUN `scripts/scan-collisions.sh` — it covers all symbol kinds AND
# cross-kind, which a same-kind `comm` of fn names cannot. Add a rename here only if a NEW symbol
# clashes; also re-diff the REQ_*/FAMILY_* value table above.
#
# Usage: scripts/sync-ai-hwaccel.sh [path-to-ai-hwaccel-checkout]   (default ../ai-hwaccel)
set -eu

HW="${1:-../ai-hwaccel}"
SRC="$HW/dist/ai-hwaccel.cyr"
DST="src/vendor/ai-hwaccel.cyr"
VER="$(cat "$HW/VERSION" 2>/dev/null || echo '?')"

[ -f "$SRC" ] || { echo "ai-hwaccel dist not found at $SRC" >&2; exit 1; }
mkdir -p src/vendor

{
  printf '# ai-hwaccel.cyr — VENDORED ai-hwaccel dist (do not edit; regenerate with scripts/sync-ai-hwaccel.sh).\n'
  printf '# Source: ai-hwaccel %s dist/ai-hwaccel.cyr. VENDORED (hoosh pattern, like src/vendor/majra.cyr\n' "$VER"
  printf '# + bote-core.cyr) rather than a [deps.ai-hwaccel] block: declaring it as a git dep means\n'
  printf '# `cyrius build` resolves it (full-deps path), which on x86_64 mis-handles szal core (error.cyr\n'
  printf '# enum-array idiom). Vendoring gives szal ZERO git deps -> a clean `cyrius lib sync` + `--no-deps`\n'
  printf '# build (matches the project build recipe). Its REQ_*/FAMILY_* constants are shared with\n'
  printf '# src/step.cyr by design (REQ_NONE is the one intentional cross-library symbol). Its own error\n'
  printf '# codes are HWA_ERR_*-prefixed as of 2.3.15, so there are no duplicate-symbol warnings left.\n'
  printf '# Re-sync: scripts/sync-ai-hwaccel.sh.\n\n'
  cat "$SRC"
} > "$DST"

echo "vendored ai-hwaccel -> $DST ($(wc -l < "$DST") lines, from ai-hwaccel $VER)"
