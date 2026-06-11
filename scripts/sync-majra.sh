#!/usr/bin/env sh
# sync-majra.sh — vendor the FULL majra dist into src/vendor/majra.cyr with szal-collision renames.
#
# majra bundles its own workflow/dag/error surface that overlaps szal's. Cyrius duplicate-symbol
# semantics = last-definition-wins, so the colliding majra symbols are renamed (in the vendored
# copy ONLY — never szal's src/*.cyr). Safe because szal never passes its own values into majra's
# workflow surface (szal implements its own engine). Full rationale + the verified 9-symbol
# collision set: docs/development/majra-vendoring.md.
#
# Renames (word-boundary anchored, idempotent):
#   ERR_*     -> MJ_ERR_*       STEP_*    -> MJ_STEP_*       TRIGGER_* -> MJ_TRIGGER_*
#   uuid_generate -> majra_uuid_generate   step_result_new -> majra_step_result_new
#
# After bumping majra, RE-RUN the collision check (see the doc) — a new release may add clashes.
#
# Usage: scripts/sync-majra.sh [path-to-majra-checkout]   (default ../majra)
set -eu

MAJRA="${1:-../majra}"
SRC="$MAJRA/dist/majra.cyr"
DST="src/vendor/majra.cyr"
VER="$(cat "$MAJRA/VERSION" 2>/dev/null || echo '?')"

[ -f "$SRC" ] || { echo "majra dist not found at $SRC" >&2; exit 1; }
mkdir -p src/vendor

{
  printf '# majra.cyr -- VENDORED full majra dist (do not edit; regenerate with scripts/sync-majra.sh)\n'
  printf '# Source: majra %s dist/majra.cyr, with szal-collision renames applied (MJ_ERR_/MJ_STEP_/\n' "$VER"
  printf '# MJ_TRIGGER_ prefixes + majra_uuid_generate/majra_step_result_new). See\n'
  printf '# docs/development/majra-vendoring.md. Hoosh vendor pattern (no [deps.majra] block).\n\n'
  sed -E \
    -e 's/\bERR_/MJ_ERR_/g' \
    -e 's/\bSTEP_/MJ_STEP_/g' \
    -e 's/\bTRIGGER_/MJ_TRIGGER_/g' \
    -e 's/\buuid_generate\b/majra_uuid_generate/g' \
    -e 's/\bstep_result_new\b/majra_step_result_new/g' \
    "$SRC"
} > "$DST"

echo "vendored FULL majra -> $DST ($(wc -l < "$DST") lines, from majra $VER)"
