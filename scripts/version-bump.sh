#!/usr/bin/env bash
# version-bump.sh — bump szal's version.
#
# `VERSION` at the repo root is the SINGLE source of truth: `cyrius.cyml [package].version`
# reads `${file:VERSION}`. The one derived copy is `var SZAL_VERSION` in src/mcp_tools_engine.cyr
# (what szal_server_info reports — CYRIUS_PKG_VERSION would report a consumer's version inside
# dist/szal-mcp.cyr), which this script rewrites and CI cross-checks. (This script used to also
# rewrite Cargo.toml + regenerate Cargo.lock — leftovers from the pre-port Rust project, which
# has no Cargo manifest at the root any more; the port's Rust oracle lives at `rust-old/` and is
# never touched. Those steps were removed at 2.1.0.)
#
# The CI "Verify version consistency" step (.github/workflows/ci.yml) re-checks the invariant
# this script maintains: VERSION == the resolved cyrius.cyml version.
set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 2.1.0"
    exit 1
fi

NEW_VERSION="$1"

# Reject anything that isn't a bare semver triple — a typo here silently propagates into the
# manifest, the CI pin checks and the release tag.
if ! printf '%s' "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "ERROR: '$NEW_VERSION' is not a X.Y.Z version" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

OLD_VERSION="$(tr -d '[:space:]' < "$REPO_ROOT/VERSION")"
echo "Bumping szal ${OLD_VERSION} -> ${NEW_VERSION}..."

echo "$NEW_VERSION" > "$REPO_ROOT/VERSION"
echo "  Updated VERSION"

ENGINE="$REPO_ROOT/src/mcp_tools_engine.cyr"
sed -i -E "s/^var SZAL_VERSION = \"[^\"]*\";/var SZAL_VERSION = \"${NEW_VERSION}\";/" "$ENGINE"
grep -q "^var SZAL_VERSION = \"${NEW_VERSION}\";" "$ENGINE" \
    || { echo "ERROR: could not set SZAL_VERSION in $ENGINE" >&2; exit 1; }
echo "  Updated SZAL_VERSION (src/mcp_tools_engine.cyr)"

# Verify the manifest still resolves to the same number. If [package].version was ever
# hardcoded away from ${file:VERSION}, this catches the drift instead of shipping it.
CYML_RAW="$(grep '^version = ' "$REPO_ROOT/cyrius.cyml" | head -1 | sed 's/version = "\(.*\)"/\1/')"
if [ "$CYML_RAW" = '${file:VERSION}' ]; then
    CYML_VERSION="$NEW_VERSION"
else
    CYML_VERSION="$CYML_RAW"
    echo "  WARNING: cyrius.cyml pins version literally ($CYML_RAW), not \${file:VERSION}"
fi

FILE_VERSION="$(tr -d '[:space:]' < "$REPO_ROOT/VERSION")"
if [ "$FILE_VERSION" != "$NEW_VERSION" ] || [ "$CYML_VERSION" != "$NEW_VERSION" ]; then
    echo "ERROR: Version mismatch after bump (VERSION=$FILE_VERSION cyrius.cyml=$CYML_VERSION)" >&2
    exit 1
fi

echo ""
echo "Version bumped to ${NEW_VERSION}"
echo ""
echo "Still to do by hand (this script owns VERSION + SZAL_VERSION):"
echo "  - cyrius distlib mcp            (dist/szal-mcp.cyr embeds the version; CI fails on drift)"
echo "  - CHANGELOG.md: add the ${NEW_VERSION} section"
echo "  - docs/development/state.md: version + toolchain/vendored pins"
echo ""
echo "Next steps:"
echo "  git add VERSION src/mcp_tools_engine.cyr dist/ CHANGELOG.md docs/development/state.md"
echo "  git commit -m \"bump to ${NEW_VERSION}\""
echo "  git tag ${NEW_VERSION}"
echo "  git push && git push --tags"
