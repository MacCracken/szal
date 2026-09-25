#!/usr/bin/env sh
# sync-ai-hwaccel.sh — vendor the ai-hwaccel dist into src/vendor/ai-hwaccel.cyr, BYTE-FOR-BYTE as
# released.
#
# szal needs ai-hwaccel's CachedRegistry + REQ_* / FAMILY_* surface for src/engine_hardware.cyr
# (hardware-aware scheduling). It is VENDORED (hoosh pattern, like majra and bote-core) rather than
# a [deps.ai-hwaccel] block, which keeps szal at ZERO git deps: CI is `cyrius lib sync` +
# `cyrius build --strict --no-deps`, with no `cyrius deps` resolve step.
#
# The copy has never needed a rename. Its one intersection with szal is REQ_NONE, which
# src/step.cyr declares too, ON PURPOSE and with the same value (0): engine_hardware.cyr checks a
# step's requirement against ai-hwaccel's REQ_* table, and step.cyr must still build without
# ai-hwaccel (dist/szal-mcp.cyr does). scripts/scan-collisions.sh allow-lists that pair only while
# the two values agree, so a future ai-hwaccel that renumbers REQ_NONE fails the scan.
#
# The file is taken from the release TAG (`git show <tag>:dist/ai-hwaccel.cyr`), never the working
# tree: at the 2.2.0 sync the ai-hwaccel checkout sat one unreleased (comment-only) commit past
# 2.4.0, and the old working-tree copy vendored it under the 2.4.0 label. The dist's own
# `# Version:` line must name the tag.
#
# Harmless, unreachable upstream references (they surface as `warning: undefined function`, not
# errors): an arg-parsing helper calls argc/argv without szal including lib/args.cyr. szal calls
# only cached_registry_new / cached_get / reg_profiles / count_satisfying / reg_count_by_family /
# reg_has_accelerator / requirement_name. If szal ever reaches those paths, fix it upstream.
#
# After a sync, re-run `scripts/scan-collisions.sh --check` and the full suite.
#
# Usage (from the szal root): scripts/sync-ai-hwaccel.sh [ai-hwaccel-checkout] [tag]
#   defaults: ../ai-hwaccel, and the tag named by that checkout's VERSION
set -eu

REPO="${1:-../ai-hwaccel}"
TAG="${2:-$(cat "$REPO/VERSION" 2>/dev/null || true)}"
SRC_PATH="dist/ai-hwaccel.cyr"
DST="src/vendor/ai-hwaccel.cyr"

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
echo "vendored ai-hwaccel $TAG -> $DST ($(wc -l < "$DST") lines, byte-identical to $TAG:$SRC_PATH)"
