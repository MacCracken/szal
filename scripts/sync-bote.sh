#!/usr/bin/env sh
# sync-bote.sh — vendor bote-core dist into src/vendor/bote-core.cyr with szal-collision renames.
#
# szal's MCP surface (mcp.cyr) needs bote-core's Dispatcher / ToolRegistry / ToolDef / Audit /
# EventSink primitives. We VENDOR (hoosh pattern) rather than a [deps.bote] block: a dep block makes
# `cyrius deps` recurse into bote's own [deps.libro]/[deps.majra] git blocks -> lib/ bloat + symbol
# collisions (bote DEPS-PATTERN.md). bote never used transports in szal, so dist/bote-core.cyr (the
# transport-free 9-module bundle) is the right cut.
#
# Collision renames (vendored copy ONLY -- never szal's src/*.cyr). Verified set at sync time:
#   compiled_compile -> bote_compiled_compile   (collides with szal condition.cyr's condition
#                                                 compiler; bote's is a JSON-schema compiler. The
#                                                 sed renames bote's definition AND its internal
#                                                 callers together, so bote stays self-consistent.)
# bote-core's registry_new is KEPT as-is: szal's only other registry_new is ai-hwaccel's, which is
# NOT vendored (engine_hardware/row 17 is deferred). That cross-library clash is tracked in
# docs/development/issues/2026-06-11-registry-new-collision.md.
#
# After bumping bote, RE-RUN the collision scan:
#   grep -oE '^fn [a-zA-Z_][a-zA-Z0-9_]*' ../bote/dist/bote-core.cyr | awk '{print $2}' | sort -u \
#     | comm -12 - <(grep -hoE '^fn [a-zA-Z_][a-zA-Z0-9_]*' src/*.cyr | awk '{print $2}' | sort -u)
# and add any new clashes to the sed below + this header.
#
# Usage: scripts/sync-bote.sh [path-to-bote-checkout]   (default ../bote)
set -eu

BOTE="${1:-../bote}"
SRC="$BOTE/dist/bote-core.cyr"
DST="src/vendor/bote-core.cyr"
VER="$(cat "$BOTE/VERSION" 2>/dev/null || echo '?')"

[ -f "$SRC" ] || { echo "bote-core dist not found at $SRC" >&2; exit 1; }
mkdir -p src/vendor

{
  printf '# bote-core.cyr -- VENDORED bote-core dist (do not edit; regenerate with scripts/sync-bote.sh)\n'
  printf '# Source: bote %s dist/bote-core.cyr, with the compiled_compile -> bote_compiled_compile\n' "$VER"
  printf '# collision rename applied (vs szal condition.cyr). Hoosh vendor pattern (no [deps.bote]\n'
  printf '# block -- avoids recursive lib/ bloat). See scripts/sync-bote.sh header.\n\n'
  sed -E \
    -e 's/\bcompiled_compile\b/bote_compiled_compile/g' \
    "$SRC"
} > "$DST"

echo "vendored bote-core -> $DST ($(wc -l < "$DST") lines, from bote $VER)"
