#!/usr/bin/env sh
# sync-majra.sh — vendor majra's full dist into src/vendor/majra.cyr, BYTE-FOR-BYTE as released.
#
# The copy is UNMODIFIED since szal 2.2.0. szal renamed its OWN colliding symbols instead of
# rewriting majra's (EventType -> SZAL_FLOW_* / SZAL_STEP_*, SzalTriggerMode + SZAL_TRIGGER_*,
# SzalStepStatus, szal_step_result_new, szal_uuid_generate), so szal compiles against exactly the
# majra its consumers vendor — hoosh commits this same file. Through 2.1.2 this script sed-renamed
# MJ_ERR_* / MJ_STEP_* / MJ_TRIGGER_* / majra_uuid_generate / majra_step_result_new /
# MJ_SYS_GETRANDOM into the copy; the history is in docs/development/majra-vendoring.md.
#
# The file is taken from the release TAG (`git show <tag>:dist/majra.cyr`), never the working tree,
# so an unreleased commit cannot be vendored under a release label, and the dist's own `# Version:`
# line must name that same tag.
#
# After a sync, re-run `scripts/scan-collisions.sh --check` (cross-kind and intra-szal collisions,
# which `cyrius build --strict` cannot see) and the full suite.
#
# Usage (from the szal root): scripts/sync-majra.sh [majra-checkout] [tag]
#   defaults: ../majra, and the tag named by that checkout's VERSION
set -eu

REPO="${1:-../majra}"
TAG="${2:-$(cat "$REPO/VERSION" 2>/dev/null || true)}"
SRC_PATH="dist/majra.cyr"
DST="src/vendor/majra.cyr"

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
echo "vendored majra $TAG -> $DST ($(wc -l < "$DST") lines, byte-identical to $TAG:$SRC_PATH)"
