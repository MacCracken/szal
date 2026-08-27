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
# NOTE (bote 2.7.5+): bote renamed its tool registry ctor registry_new -> tool_registry_new
# upstream. szal's only caller (src/mcp.cyr register_tools_with) was updated to match. This also
# DISSOLVES the old Q9 cross-library collision (bote registry_new x ai-hwaccel registry_new): bote
# no longer owns the registry_new symbol, so overlaying ai-hwaccel for engine_hardware/row 17 no
# longer clashes. See docs/development/issues/2026-06-11-registry-new-collision.md (resolved).
#
# NOTE (bote 3.3.7, szal 2.1.1): re-verified on the 3.1.4 -> 3.3.7 bump (two minors + 7 patches).
# `compiled_compile` is STILL the only szal collision, so the rename below remains the complete set,
# and all 13 bote symbols szal calls are signature-identical. No function was removed from the core
# bundle and no tool-result JSON shape changed; 16 fns were added (the `content_*` family from
# 3.3.6's new content.cyr module, plus dispatcher_server_name/_version/_set_server_info).
#
# The two DECLARED-BREAKING changes in that range do not reach szal:
#   * 3.3.5 renamed cancel_token_new/_cancel/_is_cancelled -> bote_cancel_token_* (they collided
#     with stdlib lib/async.cyr). Those live in bote's stream.cyr, which is NOT in the [lib.core]
#     cut szal vendors — zero `cancel_token` hits in dist/bote-core.cyr at either version.
#   * 3.3.0 grew Dispatcher 72 -> 88 bytes, but the new fields are APPENDED and szal never allocs
#     or offsets a Dispatcher — it only calls dispatcher_new/_set_audit/_set_events/_handle/
#     _registry. Unconfigured serverInfo is byte-identical on the wire to pre-3.3.0.
# Also note dist/bote-core.deps grew 2 leaves -> 9 at 3.3.6 (hashmap bayan + string alloc vec str
# fnptr chrono tagged). No impact: szal has no [deps.bote] block and builds --no-deps, and all 9 are
# already in cyrius.cyml [deps].stdlib.
#
# szal's bote consumers are src/mcp.cyr, src/main.cyr, and the 15 src/mcp_tools_*.cyr (schema_prop_new).
# NOT src/mcp_pool.cyr / src/mcp_tenant.cyr — those use majra ratelimit_* + stdlib only.
#
# After bumping bote, RE-RUN `scripts/scan-collisions.sh` — it covers all symbol kinds AND
# cross-kind, which a same-kind `comm` of fn names cannot (that is how the SYS_GETRANDOM `var` vs
# enum-constant collision hid until 2.1.1).
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
