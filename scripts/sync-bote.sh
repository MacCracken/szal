#!/usr/bin/env sh
# sync-bote.sh — vendor bote's transport-free core bundle into src/vendor/bote-core.cyr,
# BYTE-FOR-BYTE as released.
#
# szal's MCP surface (src/mcp.cyr + the 15 src/mcp_tools_*.cyr) needs bote-core's Dispatcher /
# ToolRegistry / ToolDef / schema / audit / event-sink primitives, and no transport, so
# dist/bote-core.cyr ([lib.core]) is the right cut. It is VENDORED (hoosh pattern) rather than a
# [deps.bote] block: a dep block makes `cyrius deps` recurse into bote's own [deps.libro] /
# [deps.majra] git blocks — lib/ bloat plus symbol collisions (bote DEPS-PATTERN.md).
#
# The copy is UNMODIFIED since szal 2.2.0. Its one clash with szal was `compiled_compile` (bote's
# JSON-schema compiler vs szal's condition compiler); szal renamed its own compiled-condition
# family to szal_condition_* instead, so szal now builds against the exact bote-core its consumers
# vendor. Through 2.1.2 this script sed-renamed bote's copy to `bote_compiled_compile`.
#
# The file is taken from the release TAG (`git show <tag>:dist/bote-core.cyr`), never the working
# tree, and the dist's own `# Version:` line must name that same tag.
#
# After a sync, re-run `scripts/scan-collisions.sh --check` and the full suite.
#
# Usage (from the szal root): scripts/sync-bote.sh [bote-checkout] [tag]
#   defaults: ../bote, and the tag named by that checkout's VERSION
set -eu

REPO="${1:-../bote}"
TAG="${2:-$(cat "$REPO/VERSION" 2>/dev/null || true)}"
SRC_PATH="dist/bote-core.cyr"
DST="src/vendor/bote-core.cyr"

[ -n "$TAG" ] || { echo "no tag given and $REPO/VERSION is unreadable" >&2; exit 1; }
git -C "$REPO" rev-parse -q --verify "refs/tags/$TAG" >/dev/null \
  || { echo "tag $TAG not found in $REPO" >&2; exit 1; }
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
git -C "$REPO" show "$TAG:$SRC_PATH" > "$tmp"
ver="$(sed -n 's/^# Version: //p' "$tmp" | head -1)"
[ "$ver" = "$TAG" ] || { echo "$SRC_PATH at tag $TAG says '# Version: $ver'" >&2; exit 1; }

mkdir -p src/vendor
cp "$tmp" "$DST"
echo "vendored bote-core $TAG -> $DST ($(wc -l < "$DST") lines, byte-identical to $TAG:$SRC_PATH)"
