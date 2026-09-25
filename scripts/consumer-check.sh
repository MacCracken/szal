#!/usr/bin/env bash
# consumer-check.sh — prove dist/szal-mcp.cyr drops into a consumer's compile set.
#
#   scripts/consumer-check.sh              generic consumer (what CI runs): the bundle against the
#                                          three upstream dists (src/vendor/ holds the release files
#                                          byte-for-byte) and EVERY module of the pinned stdlib.
#                                          The generic build + behaviour check is the suite
#                                          tests/szal_consumer_bundle.tcyr.
#   scripts/consumer-check.sh ../hoosh     a real consumer checkout (hoosh's layout: cyrius.cyml,
#                                          src/main.cyr, src/lib/*.cyr, src/vendor/*.cyr, lib/).
#
# For a checkout it runs the acceptance bar hoosh's filing set
# (docs/development/issues/archive/2026-09-25-hoosh-consumer-bundle.md):
#   1. scan-collisions.sh --check --consumer <its compile set>   -> nothing flagged;
#   2. its program + the bundle builds with `cyrius build --strict` and prints no duplicate-symbol
#      warning that its program WITHOUT the bundle does not already print (hoosh's own set carries
#      sigil x sys `uname_release`, an upstream sigil duplicate that is not szal's to fix);
#   3. szal_register_into adds the 54 tools to a dispatcher already holding one: tools/list = 55.
# The compile set is the consumer's own modules and entry file, its vendored libs, its declared
# stdlib, and the leaves dist/szal-mcp.deps names that it lacks — reported, since it must add them.
set -eu

cd "$(dirname "$0")/.."
BUNDLE=dist/szal-mcp.cyr
DEPS=dist/szal-mcp.deps
PIN="$(sed -n 's/^cyrius = "\(.*\)"/\1/p' cyrius.cyml | head -1)"
SNAP="$HOME/.cyrius/versions/$PIN/lib"
[ -f "$BUNDLE" ] || { echo "no $BUNDLE — run: cyrius distlib mcp" >&2; exit 1; }
[ -d "$SNAP" ] || { echo "cyrius $PIN is not installed ($SNAP)" >&2; exit 1; }

if [ $# -eq 0 ]; then
    echo "== generic consumer: upstream dists + the whole cyrius $PIN stdlib"
    # shellcheck disable=SC2046
    ./scripts/scan-collisions.sh --check --consumer \
        src/vendor/majra.cyr src/vendor/bote-core.cyr src/vendor/ai-hwaccel.cyr \
        $(find "$SNAP" -name '*.cyr' | sort)
    exit 0
fi

C="$(cd "$1" && pwd)"
[ -f "$C/cyrius.cyml" ] && [ -f "$C/src/main.cyr" ] || { echo "$C is not a consumer checkout" >&2; exit 1; }
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# The consumer's declared stdlib, in its order, then the bundle's leaves it lacks (marked +).
python3 - "$C/cyrius.cyml" "$DEPS" > "$WORK/leaves" <<'PY'
import re, sys
decl = re.findall(r'"([^"]+)"', re.search(r'stdlib\s*=\s*\[([^\]]*)\]', open(sys.argv[1]).read()).group(1))
need = [l.strip() for l in open(sys.argv[2]) if l.strip() and not l.startswith('#')]
for n in decl:
    print(n)
for n in need:
    if n not in decl:
        print("+" + n)
PY
added="$(sed -n 's/^+//p' "$WORK/leaves" | tr '\n' ' ')"
echo "== consumer: $C"
echo "   stdlib leaves it must add for szal-mcp: ${added:-none}"

# Its vendored libs: src/vendor/*.cyr plus any non-stdlib dist it keeps under lib/.
vendored=""
for f in "$C"/src/vendor/*.cyr "$C"/lib/*.cyr; do
    [ -f "$f" ] || continue
    [ -f "$SNAP/$(basename "$f")" ] && continue   # a stdlib file, not a vendored dist
    vendored="$vendored $f"
done
stdfiles=""
while read -r leaf; do stdfiles="$stdfiles $SNAP/${leaf#+}.cyr"; done < "$WORK/leaves"
own="$C/src/main.cyr $(ls "$C"/src/lib/*.cyr 2>/dev/null | tr '\n' ' ')"

echo "== 1. collision scan"
# shellcheck disable=SC2086
./scripts/scan-collisions.sh --check --consumer $own $vendored $stdfiles

echo "== 2. build: the consumer's own program + $BUNDLE"
# Its real entry file, with `main` renamed and the entry tail dropped, so its modules resolve what
# they call in it; the bundle goes in right after its last vendored include, where the consumer
# would put it. Stdlib includes are explicit here (a consumer usually has them auto-prepended from
# its [deps] stdlib; this probe's manifest declares none).
mkdir -p "$WORK/lib" "$WORK/dist" "$WORK/build"
cp -r "$SNAP"/. "$WORK/lib/"
cp -r "$C/lib"/. "$WORK/lib/" 2>/dev/null || true
cp -r "$C/src" "$WORK/src"
cp "$BUNDLE" "$WORK/dist/"
printf '[package]\nname = "consumer_probe"\nversion = "0.0.0"\ncyrius = "%s"\n' "$PIN" > "$WORK/cyrius.cyml"
cat > "$WORK/probe_main.cyr" <<'EOF'
fn _probe_echo(args, claims) {
    return "{\"content\":[{\"type\":\"text\",\"text\":\"probe\"}],\"isError\":false}";
}
fn _probe_parse(s) { return bayan_json_v_parse(s); }
fn main() {
    alloc_init();
    var d = dispatcher_new(tool_registry_new());
    var td = tool_def_new("probe_echo", "Echo", schema_new("object", vec_new(), vec_new()));
    dispatcher_register_tool(d, td, &_probe_echo);
    if (szal_register_into(d) != 0) { println("FAIL: szal_register_into"); return 1; }
    var out = codec_process_message("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}", d, 0);
    var o = _probe_parse(str_from(out));
    var n = bayan_json_v_arr_len(bayan_json_v_obj_get(bayan_json_v_obj_get(o, "result"), "tools"));
    print("   tools/list after szal_register_into: ", 40);
    fmt_int(n);
    print("\n", 1);
    if (n != 55) { return 1; }
    return 0;
}
var r = main();
syscall(SYS_EXIT, r);
EOF
python3 - "$C/src/main.cyr" "$WORK/leaves" "$(basename "$BUNDLE")" "$WORK" <<'PY'
import re, sys
main_src, leaves_file, bundle, work = sys.argv[1:5]
lines = open(main_src).read().split("\n")
tail = next((i for i, l in enumerate(lines) if re.match(r"var\s+\w+\s*=\s*main\(\);", l)), None)
if tail is None:
    sys.exit("consumer-check: no `var <x> = main();` entry line in " + main_src)
body = [re.sub(r"^fn main\(", "fn _consumer_main(", l) for l in lines[:tail]]
inc = [i for i, l in enumerate(body) if l.startswith('include "src/vendor/')] or \
      [i for i, l in enumerate(body) if l.startswith("include ")]
at = inc[-1] + 1 if inc else 0
prelude = [f'include "lib/{l.strip().lstrip("+")}.cyr"' for l in open(leaves_file) if l.strip()]
with open(work + "/base.cyr", "w") as f:
    f.write("\n".join(prelude + body) + "\nfn main() { return 0; }\nvar r = main();\nsyscall(SYS_EXIT, r);\n")
with open(work + "/probe.cyr", "w") as f:
    f.write("\n".join(prelude + body[:at] + [f'include "dist/{bundle}"'] + body[at:]) + "\n"
            + open(work + "/probe_main.cyr").read())
PY
( cd "$WORK" && cyrius build --strict --no-deps base.cyr build/base > base.log 2>&1 ) \
    || { echo "FAIL: the consumer's program does not build on its own"; grep -E 'error' "$WORK/base.log" | head -5; exit 1; }
( cd "$WORK" && cyrius build --strict --no-deps probe.cyr build/probe > probe.log 2>&1 ) \
    || { echo "FAIL: the consumer's program + $BUNDLE does not build"; grep -E 'error' "$WORK/probe.log" | head -5; exit 1; }
dup() { grep -oE "duplicate [a-z]+ '[^']+'" "$1" | sort -u || true; }
new_dups="$(comm -13 <(dup "$WORK/base.log") <(dup "$WORK/probe.log"))"
echo "   duplicate warnings its own program already prints: $(dup "$WORK/base.log" | tr '\n' ' ')"
[ -z "$new_dups" ] || { echo "FAIL: the bundle adds duplicate warnings:"; echo "$new_dups"; exit 1; }
echo "   the bundle adds none"

echo "== 3. register into an existing dispatcher"
"$WORK/build/probe" || { echo "FAIL: expected 55 tools"; exit 1; }
echo "OK: $BUNDLE drops into $C"
