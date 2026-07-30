#!/usr/bin/env bash
# Usage:
#   ./scripts/bench-history.sh            # build + run the benchmarks, append to CSV
#   ./scripts/bench-history.sh --dry-run  # build + run, print results, DON'T touch the CSV
#   ./scripts/bench-history.sh --show     # show recent history
#
# Builds and runs benches/bench_all.bcyr and appends its timings to benchmarks/history.csv.
# The CSV is the proof — never skip benchmarks.
#
# ── Rust criterion -> Cyrius bench ──────────────────────────────────────────────────────────
# This script used to run `cargo bench` and parse criterion's
#     bench_name    time:   [1.2345 µs 1.2456 µs 1.2567 µs]
# output. szal is now a Cyrius port with no cargo, so it runs the Cyrius harness instead.
#
# It does NOT parse lib/bench.cyr's human-readable bench_report line. That format is lossy and
# version-dependent: _fmt_time only renders a microsecond fraction ("1.481us") from cyrius
# 6.2.15 on, and at the version this project pins it prints bare integer microseconds — "1us"
# for 1481ns, a 48% error on every sub-millisecond benchmark. bench.cyr's own comment records
# that this exact rounding "flat-lined the micro-bench history" once already.
#
# So benches/bench_all.bcyr emits a second, machine-readable line per benchmark straight from
# bench_avg_ns/min/max, and that is what gets recorded:
#     BENCHDATA <name> <avg_ns> <min_ns> <max_ns> <iterations>
# Integers only, no unit conversion, no rounding, identical on every toolchain version. The avg
# is what lands in the CSV (criterion's middle value was also the estimate); min/max/iterations
# are parsed too so a future schema can carry them without another harness change.
#
# ── reading the CSV across the port boundary ────────────────────────────────────────────────
# The benchmark NAMES are unchanged, so each series is continuous in name — but rows at version
# 1.0.1-1.2.0 are Rust/criterion measurements and rows from 2.0.0 on are Cyrius. They measure
# the same workload SHAPE, not the same implementation, so do not read the step between them as
# a regression. Compare Cyrius to Cyrius. The `version` column is what separates the two eras.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HISTORY_DIR="$ROOT/benchmarks"
HISTORY_FILE="$HISTORY_DIR/history.csv"
BENCH_SRC="$ROOT/benches/bench_all.bcyr"
BENCH_BIN="$ROOT/build/szal_bench"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"
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

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

# ── build the harness ────────────────────────────────────────────────────────
# --strict --no-deps is the project build recipe (szal vendors every git dep). A missing or
# non-building harness is a hard error: silently recording nothing is how szal ended up with an
# empty benchmark history in the first place.

if [ ! -f "$BENCH_SRC" ]; then
  echo "ERROR: benchmark harness missing: ${BENCH_SRC#"$ROOT"/}" >&2
  exit 1
fi

echo "Building benchmarks (szal v${VERSION} @ ${COMMIT})..."
mkdir -p "$ROOT/build"
if ! CYRIUS_NO_WARN_SHADOW_LIB=1 cyrius build --strict --no-deps "$BENCH_SRC" "$BENCH_BIN"; then
  echo "ERROR: benchmark harness failed to build" >&2
  exit 1
fi

# ── run ──────────────────────────────────────────────────────────────────────
# The harness pre-checks every fixture before timing it and exits non-zero if any check fails,
# so a non-zero status here means the numbers would have been meaningless.

echo ""
echo "Running benchmarks..."
echo ""

if ! BENCH_OUTPUT="$("$BENCH_BIN" 2>&1)"; then
  echo "Benchmark run failed (precheck or crash):" >&2
  echo "$BENCH_OUTPUT" >&2
  exit 1
fi
echo "$BENCH_OUTPUT"

# ── parse the BENCHDATA lines ────────────────────────────────────────────────
# BENCHDATA <name> <avg_ns> <min_ns> <max_ns> <iterations> — already in nanoseconds.

ENTRIES=0
PARSED=""
while IFS= read -r line; do
  if [[ "$line" =~ ^BENCHDATA[[:space:]]+([a-zA-Z0-9_]+)[[:space:]]+([0-9]+)[[:space:]]+([0-9]+)[[:space:]]+([0-9]+)[[:space:]]+([0-9]+)[[:space:]]*$ ]]; then
    name="${BASH_REMATCH[1]}"
    avg_ns="${BASH_REMATCH[2]}"
    iters="${BASH_REMATCH[5]}"

    # A benchmark that recorded zero iterations measured nothing — refuse it rather than write a
    # meaningless 0 into the series.
    if [ "$iters" -eq 0 ]; then
      echo "ERROR: benchmark '$name' reported 0 iterations" >&2
      exit 1
    fi

    # The CSV has carried two decimal places since the criterion era; keep the column format
    # byte-compatible with the existing rows even though the source is now an exact integer.
    time_ns=$(printf '%.2f' "$avg_ns")
    PARSED="${PARSED}${TIMESTAMP},${VERSION},${COMMIT},${name},${time_ns},ns"$'\n'
    ENTRIES=$((ENTRIES + 1))
  fi
done <<< "$BENCH_OUTPUT"

# ── sanity-gate the parse ────────────────────────────────────────────────────
# A silent parse failure (a changed bench_report format, a renamed benchmark) would append zero
# rows and still exit 0, leaving the CSV frozen while CI stayed green. Require that the parse
# found at least as many benchmarks as the harness declares via bench_new.

EXPECTED=$(grep -c 'bench_new("' "$BENCH_SRC" || true)
echo ""
if [ "$ENTRIES" -eq 0 ]; then
  echo "ERROR: no BENCHDATA lines parsed from the harness output." >&2
  echo "       Check _b_emit in the harness against the parse regex in this script." >&2
  exit 1
fi
if [ "$ENTRIES" -lt "$EXPECTED" ]; then
  echo "ERROR: parsed $ENTRIES result(s) but the harness declares $EXPECTED benchmark(s)." >&2
  echo "       Some benchmark did not report — refusing to record a partial run." >&2
  exit 1
fi

if [ "$DRY_RUN" -eq 1 ]; then
  echo "Dry run: parsed ${ENTRIES} result(s); ${HISTORY_FILE#"$ROOT"/} not modified."
  exit 0
fi

# ── append ───────────────────────────────────────────────────────────────────

if [ ! -f "$HISTORY_FILE" ]; then
  echo "timestamp,version,commit,benchmark,time_ns,unit" > "$HISTORY_FILE"
  echo "Created ${HISTORY_FILE#"$ROOT"/}"
fi
printf '%s' "$PARSED" >> "$HISTORY_FILE"

echo "Recorded ${ENTRIES} benchmark results to ${HISTORY_FILE#"$ROOT"/}"
echo ""
echo "View history: ./scripts/bench-history.sh --show"
