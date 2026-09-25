#!/usr/bin/env bash
# scan-collisions.sh — cross-kind global-symbol collision scanner for szal.
#
# WHY THIS EXISTS. Cyrius resolves fns, top-level `var`s, `const`s and enum CONSTANTS in ONE flat
# namespace, last-definition-wins. `cyrius build --strict` does NOT report most of that:
#
#   fn X    vs fn X                        -> warning
#   enumconst X vs enumconst X, diff value -> warning   (SILENT if the values match, and SILENT for
#                                                       constants past var index 1024 — see below)
#   enum/var vs var, different int         -> warning
#   enumconst X then `var X = <non-int>`   -> hard error ("shadows an enum constant")
#   enumconst X then `var X[8];`           -> SILENT    (array form skips the shadow check)
#   fn X    vs var X                       -> SILENT  <-- and it MISCOMPILES: `&X` binds to the
#   fn X    vs enumconst X                 -> SILENT  <-- data symbol, so any fn pointer taken on
#   struct X vs struct X / fn X            -> SILENT       that name jumps into .bss. szal
#   enum TYPE name vs enum TYPE name       -> SILENT       dispatches 54 MCP tools via fn pointers.
#
# And an enum constant registered past var-table index 1024 is not constant-FOLDED at all (cycc
# 6.6.6 PARSE_ENUM_DEF, `vcnt < 1024`): it behaves as a global, so a later same-named constant wins
# for EVERY read, including reads compiled before it. That is how szal <= 2.1.2 shipped a condition
# parser that could not parse `(x)`: src/mcp_tools_math.cyr redeclared src/condition.cyr's
# TOK_LPAREN / TOK_RPAREN (13/14 -> 6/7) — two szal files, so the old szal-vs-everyone-else scan
# never compared them. The INTRA-SZAL pass below exists because of it.
#
# Caught so far by this scanner or its predecessors:
#   * BYTES_PER_GB  — szal enum 2^30 vs ai-hwaccel `var` 10^9 (cross-kind, different values); 2.1.0.
#   * SYS_GETRANDOM — majra `var` 318 (x86-hardcoded) vs the stdlib's arch-conditional enum
#                     constant (318/278/45); 2.1.1. Fixed upstream in majra 2.7.3.
#   * TOK_LPAREN / TOK_RPAREN — szal x szal, different values; 2.2.0 (MEV_TOK_* in the math tool).
#   * CACHE_ENTRY_SIZE — szal enum constant vs hoosh `var` (cross-kind), found by --consumer; 2.2.0.
#
# USAGE
#   scripts/scan-collisions.sh [--check]
#       szal's own build: src/*.cyr x src/vendor/{majra,bote-core,ai-hwaccel}.cyr x lib/*.cyr, plus
#       every name defined in two different szal files.
#   scripts/scan-collisions.sh [--check] [--bundle PATH] --consumer FILE...
#       a CONSUMER's compile set (its sources, its vendored libs, its stdlib) x the szal bundle it
#       would vendor (default dist/szal-mcp.cyr), plus every name the bundle defines twice.
#       scripts/consumer-check.sh assembles FILE... for a real consumer checkout.
#   --check exits 1 if anything outside the allow-list collides.
#
# The allow-list below is the set of intersections that are INTENTIONAL. An allow-listed name still
# fails when its integer values disagree between the two sides. Re-run after EVERY vendored-dep
# bump and every toolchain bump, and justify any addition in docs/development/majra-vendoring.md.
set -eu

cd "$(dirname "$0")/.."

# Intentional intersections (each must also agree on its VALUE).
#   REQ_NONE   szal src/step.cyr x ai-hwaccel — the SHARED hardware-requirement "none" (= 0).
#              step.cyr declares it so it builds without ai-hwaccel (dist/szal-mcp.cyr);
#              engine_hardware.cyr compares a step's requirement against ai-hwaccel's REQ_* table.
ALLOW="REQ_NONE"

python3 - "$ALLOW" "$@" <<'PY'
import re, sys, glob, os, collections

allow = set(sys.argv[1].split())
args = sys.argv[2:]
check, bundle, consumer = False, "dist/szal-mcp.cyr", None
i = 0
while i < len(args):
    a = args[i]
    if a == "--check":
        check = True
    elif a == "--bundle":
        i += 1
        bundle = args[i]
    elif a == "--consumer":
        consumer = args[i + 1:]
        break
    else:
        sys.exit(f"scan-collisions: unknown argument {a!r}")
    i += 1

def int_value(expr):
    """The integer an initializer spells, or None if it is not a plain literal."""
    e = expr.strip().replace(" ", "")
    m = re.fullmatch(r"(-?)(0[xX][0-9a-fA-F]+|\d+)", e)
    if not m:
        return None
    v = int(m.group(2), 0)
    return -v if m.group(1) else v

def strip_line(ln):
    """Blank out string/char literals and trailing comments, PER LINE.

    A character walk rather than a regex: a regex like `"(\\.|[^"\\])*"` has a character class
    that matches newlines, so a lone quote inside a COMMENT (`# the "params" check`) swallows
    everything up to the next quote — deleting real `fn` declarations and unbalancing the brace
    depth. That silently under-reports, the worst failure mode a collision scanner has.
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

def symbols(path):
    """Every GLOBAL symbol in `path` as (kind, name, file:line, int value or None).

    Only brace-depth 0 declarations count, so function locals are ignored. Enum MEMBERS are
    collected from inside `enum X { ... }` blocks specifically, in both the multi-line and the
    single-line `enum X { A = 0; B = 1; }` forms — szal uses the single-line form heavily, and a
    scanner that reads to the next `}` swallows the functions that follow and then reports a false
    "0 collisions".
    """
    out = []
    try:
        src = open(path, encoding="utf-8", errors="replace").read()
    except OSError:
        return out
    member = re.compile(r"([A-Za-z_]\w*)\s*=\s*([^;,}]*)")
    depth = 0
    enum_depth = None          # brace depth at which the current enum block sits, else None
    for lineno, raw in enumerate(src.split("\n"), 1):
        line = strip_line(raw)
        stripped = line.strip()
        opens = line.count("{")
        closes = line.count("}")
        loc = f"{path}:{lineno}"
        if enum_depth is not None:
            for mm in member.finditer(stripped):
                out.append(("enumconst", mm.group(1), loc, int_value(mm.group(2))))
        elif depth == 0:
            m = re.match(r"fn\s+([A-Za-z_]\w*)\s*\(", stripped)
            if m:
                out.append(("fn", m.group(1), loc, None))
            m = re.match(r"(?:var|const)\s+([A-Za-z_]\w*)\s*(=\s*([^;]*))?", stripped)
            if m:
                out.append(("var", m.group(1), loc, int_value(m.group(3)) if m.group(3) else None))
            m = re.match(r"struct\s+([A-Za-z_]\w*)", stripped)
            if m:
                out.append(("struct", m.group(1), loc, None))
            m = re.match(r"enum\s+([A-Za-z_]\w*)[^{]*\{(.*)$", stripped)
            if m:
                out.append(("enumtype", m.group(1), loc, None))
                for mm in member.finditer(m.group(2)):
                    out.append(("enumconst", mm.group(1), loc, int_value(mm.group(2))))
                if opens > closes:   # multi-line form: stay in enum mode until the brace closes
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
        for kind, name, loc, val in symbols(p):
            d[name].append((kind, loc, val))
    return d

if consumer is None:
    groups = {
        "szal":      sorted(glob.glob("src/*.cyr")),
        "majra":     ["src/vendor/majra.cyr"],
        "bote":      ["src/vendor/bote-core.cyr"],
        "aihwaccel": ["src/vendor/ai-hwaccel.cyr"],
        "stdlib":    sorted(glob.glob("lib/*.cyr")),
    }
    owned = "szal"
else:
    if not os.path.exists(bundle):
        sys.exit(f"scan-collisions: bundle {bundle} not found (run `cyrius distlib mcp`)")
    files = [f for f in consumer if os.path.isfile(f)]
    if not files:
        sys.exit("scan-collisions: --consumer needs at least one existing file")
    groups = {"bundle": [bundle], "consumer": files}
    owned = "bundle"
syms = {g: collect(ps) for g, ps in groups.items()}

def values(entries):
    return {v for _, _, v in entries if v is not None}

def kinds(entries):
    return "/".join(sorted({k for k, _, _ in entries}))

rows = []   # (name, verdict, text)
names = list(groups)
for i in range(len(names)):
    for j in range(i + 1, len(names)):
        a, b = names[i], names[j]
        for name in sorted(set(syms[a]) & set(syms[b])):
            ea, eb = syms[a][name], syms[b][name]
            verdict = "FLAG "
            note = ""
            if name in allow:
                va, vb = values(ea), values(eb)
                if va and va == vb and len(va) == 1:
                    verdict = "ALLOW"
                else:
                    note = f"   (allow-listed, but values differ: {sorted(va)} vs {sorted(vb)})"
            rows.append((name, verdict, f"{a}:{kinds(ea)} @{ea[0][1]}   x   {b}:{kinds(eb)} @{eb[0][1]}{note}"))

# Names the szal side defines more than once (two szal files, or twice inside the bundle).
for name, entries in sorted(syms[owned].items()):
    locs = sorted({loc for _, loc, _ in entries})
    if consumer is None:
        files = sorted({loc.rsplit(":", 1)[0] for loc in locs})
        if len(files) < 2:
            continue
    elif len(locs) < 2:
        continue
    vals = sorted(values(entries))
    rows.append((name, "FLAG ", f"{owned} x {owned}: {kinds(entries)} @{' @'.join(locs)}"
                 + (f"   values {vals}" if len(vals) > 1 else "")))

flagged = [r for r in rows if r[1] == "FLAG "]
print("symbol sets: " + ", ".join(f"{g}={len(syms[g])}" for g in names))
print(f"intersections: {len(rows)}  ({len(rows) - len(flagged)} allow-listed, {len(flagged)} flagged)\n")
for name, verdict, text in rows:
    print(f"  [{verdict}] {name:<26} {text}")
if not rows:
    print("  (none)")
print()
if check and flagged:
    print(f"FAIL: {len(flagged)} collision(s) outside the allow-list.")
    sys.exit(1)
print("OK" + (" (--check passed)" if check else ""))
PY
