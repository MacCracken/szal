#!/usr/bin/env sh
# sync-majra-metrics.sh — vendor ONLY majra's self-contained metrics module as the interim shim
# src/vendor/majra_metrics.cyr (the full majra dist is deferred — see docs/development/majra-vendoring.md).
#
# Extracts the `# --- metrics.cyr ---` module (self-contained: fl_alloc + fncall2 only,
# collision-free) from a majra checkout's dist bundle, applies the same MJ_/majra_ collision
# renames the full vendoring will use (no-ops on the metrics section today — defensive against a
# future majra release adding a colliding token), and prepends a provenance header.
#
# Usage: scripts/sync-majra-metrics.sh [path-to-majra-checkout]   (default ../majra)
set -eu

MAJRA="${1:-../majra}"
SRC="$MAJRA/dist/majra.cyr"
DST="src/vendor/majra_metrics.cyr"
VER="$(cat "$MAJRA/VERSION" 2>/dev/null || echo '?')"

[ -f "$SRC" ] || { echo "majra dist not found at $SRC" >&2; exit 1; }
mkdir -p src/vendor

{
  printf '# majra_metrics.cyr -- VENDORED interim shim (do not edit; regenerate with scripts/sync-majra-metrics.sh)\n'
  printf '# Source: majra %s dist/majra.cyr, the self-contained `metrics.cyr` module only.\n' "$VER"
  printf '# The FULL majra dist is a required-but-deferred 2.0.0 deliverable -- see\n'
  printf '# docs/development/majra-vendoring.md. When it lands, delete this file and repoint\n'
  printf '# src/metrics.cyr at src/vendor/majra.cyr (identical METRICS_VTABLE_SIZE/noop_metrics surface).\n'
  printf '# Provides: METRICS_VTABLE_SIZE, noop_metrics(), metrics_workflow_run_started/completed/failed,\n'
  printf '#   metrics_workflow_step_started/finished (+ the queue/pubsub/etc slots szal does not call yet).\n'
  printf '# Requires (stdlib): freelist (fl_alloc), fnptr (fncall2).\n\n'
  # Extract the metrics module: from its marker up to (not including) the next MODULE marker.
  # Module markers are specifically `# --- <name>.cyr ---`; internal `# --- ... ---` comments
  # (e.g. "Metrics dispatch helpers") do NOT end in `.cyr ---`, so they stay inside the module.
  # Then apply the collision renames (word-boundary anchored; idempotent).
  awk '
    /^# --- [a-z_]+\.cyr ---/ {
      if ($0 ~ /metrics\.cyr/) { inmod=1; print; next }
      else if (inmod) { inmod=0 }
    }
    inmod { print }
  ' "$SRC" | sed -E \
      -e 's/\bERR_/MJ_ERR_/g' \
      -e 's/\bSTEP_/MJ_STEP_/g' \
      -e 's/\bTRIGGER_/MJ_TRIGGER_/g' \
      -e 's/\buuid_generate\b/majra_uuid_generate/g' \
      -e 's/\bstep_result_new\b/majra_step_result_new/g'
} > "$DST"

echo "vendored metrics shim -> $DST ($(wc -l < "$DST") lines, from majra $VER)"
