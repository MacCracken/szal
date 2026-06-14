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
# NO symbol rename is applied: ai-hwaccel's only collisions with szal core are REQ_*/FAMILY_*
# (intentionally SHARED with src/step.cyr — they are the hardware-requirement API) and
# ERR_NONE/BYTES_PER_GB (defined-but-unused in ai-hwaccel — harmless duplicates of szal core under
# the single-pass include order; src/engine_hardware.cyr uses neither). After bumping ai-hwaccel,
# RE-RUN the collision scan and add a rename here only if a NEW *used* symbol clashes:
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
  printf '# src/step.cyr by design; ERR_NONE/BYTES_PER_GB are defined-but-unused here (harmless duplicates\n'
  printf '# of szal core under the single-pass include order). Re-sync: scripts/sync-ai-hwaccel.sh.\n\n'
  cat "$SRC"
} > "$DST"

echo "vendored ai-hwaccel -> $DST ($(wc -l < "$DST") lines, from ai-hwaccel $VER)"
