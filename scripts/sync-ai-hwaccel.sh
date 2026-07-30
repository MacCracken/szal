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
# NO symbol rename is applied. As of ai-hwaccel 2.3.15 / szal 2.1.0 its ONLY collision with szal
# core is REQ_*/FAMILY_* — intentionally SHARED with src/step.cyr, they ARE the hardware-requirement
# API. The former ERR_NONE and BYTES_PER_GB duplicates are gone from both sides: ai-hwaccel 2.3.15
# prefixed its codes HWA_ERR_*, and szal 2.1.0 prefixed its own SZAL_ERR_* / SZAL_BYTES_PER_*.
#
# KNOWN UPSTREAM BREAKAGE (ai-hwaccel 2.3.15, harmless for szal): `profile_from_json_str` still
# calls bayan's pre-1.3.0 `json_v_parse_str`, which no longer exists (renamed `json_v_parse_buf`),
# and an arg-parsing helper calls `argc`/`argv` without szal including lib/args.cyr. All three show
# as `warning: undefined function` and NOT as errors, because szal reaches none of them —
# src/engine_hardware.cyr only calls `cached_registry_new` / `cached_get`. If a future szal change
# calls into those paths the build will fail to link; fix it upstream in ai-hwaccel, not here.
#
# After bumping ai-hwaccel, RE-RUN the collision scan and add a rename here only if a NEW *used*
# symbol clashes:
#   comm -12 \
#     <(grep -oE '^fn [a-zA-Z_][a-zA-Z0-9_]*' ../ai-hwaccel/dist/ai-hwaccel.cyr | awk '{print $2}' | sort -u) \
#     <(grep -hoE '^fn [a-zA-Z_][a-zA-Z0-9_]*' src/*.cyr src/vendor/*.cyr | awk '{print $2}' | sort -u)
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
