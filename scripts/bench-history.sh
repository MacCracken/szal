#!/usr/bin/env bash
# Usage:
#   ./scripts/bench-history.sh          # run all benchmarks, append to CSV
#   ./scripts/bench-history.sh --show   # show recent history
#
# Runs criterion benchmarks and appends timing results to benchmarks/history.csv.
# The CSV is the proof — never skip benchmarks.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HISTORY_DIR="$ROOT/benchmarks"
HISTORY_FILE="$HISTORY_DIR/history.csv"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
VERSION="$(cat "$ROOT/VERSION" | tr -d '[:space:]')"
COMMIT="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo 'unknown')"

mkdir -p "$HISTORY_DIR"

# ── show mode ────────────────────────────────────────────────────────────────

if [ "${1:-}" = "--show" ]; then
  if [ ! -f "$HISTORY_FILE" ]; then
    echo "No benchmark history yet. Run: ./scripts/bench-history.sh"
    exit 0
  fi
  echo "Recent benchmark history (last 20 entries):"
  echo ""
  head -1 "$HISTORY_FILE"
  tail -20 "$HISTORY_FILE"
  exit 0
fi

# ── run benchmarks ───────────────────────────────────────────────────────────

echo "Running benchmarks (szal v${VERSION} @ ${COMMIT})..."
echo ""

BENCH_OUTPUT=$(cargo bench --no-fail-fast 2>&1) || {
  echo "Benchmark run failed:"
  echo "$BENCH_OUTPUT"
  exit 1
}

# ── write CSV header if new file ─────────────────────────────────────────────

if [ ! -f "$HISTORY_FILE" ]; then
  echo "timestamp,version,commit,benchmark,time_ns,unit" > "$HISTORY_FILE"
  echo "Created $HISTORY_FILE"
fi

# ── parse criterion output ───────────────────────────────────────────────────
# Criterion outputs lines like:
#   bench_name          time:   [1.2345 µs 1.2456 µs 1.2567 µs]
# We extract the middle (estimate) value.

ENTRIES=0
while IFS= read -r line; do
  # Match: "name    time:   [low est high unit]"
  if [[ "$line" =~ ^([a-zA-Z0-9_/]+)[[:space:]]+time:[[:space:]]+\[.*[[:space:]]([0-9.]+)[[:space:]]+(ns|µs|us|ms|s)[[:space:]] ]]; then
    name="${BASH_REMATCH[1]}"
    value="${BASH_REMATCH[2]}"
    unit="${BASH_REMATCH[3]}"

    # Normalize to nanoseconds
    case "$unit" in
      ns)  time_ns="$value" ;;
      µs|us) time_ns=$(echo "$value * 1000" | bc -l) ;;
      ms)  time_ns=$(echo "$value * 1000000" | bc -l) ;;
      s)   time_ns=$(echo "$value * 1000000000" | bc -l) ;;
    esac

    # Strip trailing zeros from bc output
    time_ns=$(printf '%.2f' "$time_ns")

    echo "${TIMESTAMP},${VERSION},${COMMIT},${name},${time_ns},ns" >> "$HISTORY_FILE"
    ENTRIES=$((ENTRIES + 1))
  fi
done <<< "$BENCH_OUTPUT"

echo ""
if [ "$ENTRIES" -gt 0 ]; then
  echo "Recorded ${ENTRIES} benchmark results to ${HISTORY_FILE##"$ROOT"/}"
else
  echo "Warning: No benchmark results parsed from output."
  echo "Raw output saved for debugging:"
  echo "$BENCH_OUTPUT" | head -30
fi

echo ""
echo "View history: ./scripts/bench-history.sh --show"
