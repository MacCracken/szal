#!/usr/bin/env bash
# scan-collisions.sh — cross-kind global-symbol collision scanner for szal.
#
# WHY THIS EXISTS. Cyrius resolves fns, top-level `var`s, `const`s and enum CONSTANTS in ONE flat
# namespace, last-definition-wins. `cyrius build --strict` does NOT report most of that:
#
#   fn X    vs fn X                        -> warning
#   enumconst X vs enumconst X, diff value -> warning   (SILENT if the values match)
#   enum/var vs var, different int         -> warning   (but see the index-1024 cap below)
#   enumconst X then `var X = <non-int>`   -> hard error ("shadows an enum constant")
#   enumconst X then `var X[8];`           -> SILENT    (array form skips the shadow check)
#   fn X    vs var X                       -> SILENT  <-- and it MISCOMPILES: `&X` binds to the
#   fn X    vs enumconst X                 -> SILENT  <-- data symbol, so any fn pointer taken on
#   struct X vs struct X / fn X            -> SILENT       that name jumps into .bss. szal
#   enum TYPE name vs enum TYPE name       -> SILENT       dispatches 54 MCP tools via fn pointers.
#
# On top of that, the value-conflict warning is hard-capped at var-table index 1024
# (cycc parse_types.cyr) — szal's var_table is >2000, so roughly half of szal's globals sit in a
# zone where that warning CANNOT fire. A green `--strict` build is therefore much weaker evidence
# than it looks. This scanner is the actual defence.
#
# Two real bugs of exactly this shape have already been caught by hand:
#   * BYTES_PER_GB  — szal enum 2^30 vs ai-hwaccel `var` 10^9 (cross-kind, DIFFERENT values), fixed
#                     at 2.1.0 by prefixing szal's to SZAL_BYTES_PER_GB.
#   * SYS_GETRANDOM — majra `var SYS_GETRANDOM = 318` (x86-hardcoded) vs the stdlib's ARCH-
#                     CONDITIONAL enum constant (318/278/45). Cross-kind, values match on x86_64 so
#                     the compiler is silent, but wrong on aarch64/agnos. Fixed at 2.1.1 by the
#                     MJ_SYS_GETRANDOM rename in scripts/sync-majra.sh.
#
# USAGE
#   scripts/scan-collisions.sh              # report every cross-kind intersection
#   scripts/scan-collisions.sh --check      # exit 1 if anything outside the allow-list collides
#
# The allow-list below is the set of intersections that are INTENTIONAL or verified-benign. Re-run
# this after EVERY vendored-dep bump and every toolchain bump, and justify any addition in
# docs/development/majra-vendoring.md.
set -eu

cd "$(dirname "$0")/.."

# Intentional / verified-benign intersections.
#   REQ_NONE            szal src/step.cyr:48 x ai-hwaccel — the SHARED hardware-requirement API,
#                       both = 0, deliberately the same symbol.
#   StepStatus          enum TYPE names (szal src/step.cyr:26,35 x majra). Cyrius does not put enum
#   TriggerMode         type names in the flat symbol table, and neither is used as a type
#                       annotation in szal. Verified benign; documented so it isn't re-litigated.
ALLOW="REQ_NONE StepStatus TriggerMode"

CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1

python3 - "$CHECK" "$ALLOW" <<'PY'
import re, sys, glob, os, collections

check = sys.argv[1] == "1"
allow = set(sys.argv[2].split())

# Extract every GLOBAL symbol with its kind. Brace-depth 0 only, so locals are ignored.
# Handles BOTH the multi-line and the single-line `enum X { A = 0; B = 1; }` form — szal uses the
# single-line form heavily, and a naive scanner that reads to the next `}` swallows the following
# functions and reports a false "0 collisions".
def symbols(path):
    """Every GLOBAL symbol in `path`, as (kind, name, file:line).

    Only brace-depth 0 declarations count, so function locals are ignored. Enum MEMBERS are
    collected from inside `enum X { ... }` blocks specifically — NOT from every brace-depth > 0
    line, which would register every local assignment in every function body as a symbol.
    Both the multi-line and the single-line `enum X { A = 0; B = 1; }` forms are handled: szal
    uses the single-line form heavily, and a scanner that reads to the next `}` swallows the
    functions that follow and then reports a false "0 collisions".
    """
    out = []
    try:
        src = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return out

    def strip_line(ln):
        """Blank out string/char literals and trailing comments, PER LINE.

        Done as a character walk rather than a regex: a regex like `"(\\.|[^"\\])*"` has a
        character class that matches newlines, so a lone quote inside a COMMENT (e.g.
        `# the "params" check`) makes it swallow everything up to the next quote — deleting real
        `fn` declarations and unbalancing the brace depth. That silently under-reports symbols,
        which for a collision scanner is the worst possible failure mode.
        """
        out_chars = []
        i, n, quote = 0, len(ln), None
        while i < n:
            ch = ln[i]
            if quote:
                if ch == "\\":
                    i += 2
                    continue
                if ch == quote:
                    quote = None
                i += 1
                continue
            if ch in ('"', "'"):
                quote = ch
                i += 1
                continue
            if ch == "#":
                break
            out_chars.append(ch)
            i += 1
        return "".join(out_chars)

    depth = 0
    enum_depth = None          # brace depth at which the current enum block sits, else None
    for lineno, raw in enumerate(src.split("\n"), 1):
        line = strip_line(raw)
        stripped = line.strip()
        opens = line.count("{")
        closes = line.count("}")

        if enum_depth is not None:
            for mm in re.finditer(r"([A-Za-z_]\w*)\s*=", stripped):
                out.append(("enumconst", mm.group(1), f"{path}:{lineno}"))
        elif depth == 0:
            m = re.match(r"fn\s+([A-Za-z_]\w*)\s*\(", stripped)
            if m:
                out.append(("fn", m.group(1), f"{path}:{lineno}"))
            m = re.match(r"(?:var|const)\s+([A-Za-z_]\w*)", stripped)
            if m:
                out.append(("var", m.group(1), f"{path}:{lineno}"))
            m = re.match(r"struct\s+([A-Za-z_]\w*)", stripped)
            if m:
                out.append(("struct", m.group(1), f"{path}:{lineno}"))
            m = re.match(r"enum\s+([A-Za-z_]\w*)\s*\{(.*)$", stripped)
            if m:
                out.append(("enumtype", m.group(1), f"{path}:{lineno}"))
                body = m.group(2)
                for mm in re.finditer(r"([A-Za-z_]\w*)\s*=", body):
                    out.append(("enumconst", mm.group(1), f"{path}:{lineno}"))
                # multi-line form: stay in enum mode until the brace closes
                if opens > closes:
                    enum_depth = depth

        depth += opens - closes
        if depth < 0:
            depth = 0
        if enum_depth is not None and depth <= enum_depth:
            enum_depth = None
    return out

def collect(paths):
    d = collections.defaultdict(list)
    for p in paths:
        for kind, name, loc in symbols(p):
            d[name].append((kind, loc))
    return d

lib = sorted(glob.glob("lib/*.cyr"))
groups = {
    "szal":      sorted(glob.glob("src/*.cyr")),
    "majra":     ["src/vendor/majra.cyr"],
    "bote":      ["src/vendor/bote-core.cyr"],
    "aihwaccel": ["src/vendor/ai-hwaccel.cyr"],
    "stdlib":    lib,
}
syms = {g: collect(ps) for g, ps in groups.items()}

names = list(groups)
rows = []
for i in range(len(names)):
    for j in range(i + 1, len(names)):
        a, b = names[i], names[j]
        for name in sorted(set(syms[a]) & set(syms[b])):
            ka = sorted({k for k, _ in syms[a][name]})
            kb = sorted({k for k, _ in syms[b][name]})
            rows.append((name, a, ka, syms[a][name][0][1], b, kb, syms[b][name][0][1]))

flagged = [r for r in rows if r[0] not in allow]
print(f"symbol sets: " + ", ".join(f"{g}={len(syms[g])}" for g in names))
print(f"pairwise intersections: {len(rows)}  ({len(rows)-len(flagged)} allow-listed, {len(flagged)} flagged)\n")
for name, a, ka, la, b, kb, lb in rows:
    tag = "ALLOW" if name in allow else "FLAG "
    print(f"  [{tag}] {name:<24} {a}:{'/'.join(ka)} @{la}   x   {b}:{'/'.join(kb)} @{lb}")
if not rows:
    print("  (none)")
print()
if check and flagged:
    print(f"FAIL: {len(flagged)} collision(s) outside the allow-list.")
    sys.exit(1)
print("OK" + (" (--check passed)" if check else ""))
PY
