#!/usr/bin/env sh
# sync-majra.sh — vendor the FULL majra dist into src/vendor/majra.cyr with szal-collision renames.
#
# majra bundles its own workflow/dag/error surface that overlaps szal's. Cyrius duplicate-symbol
# semantics = last-definition-wins, so the colliding majra symbols are renamed (in the vendored
# copy ONLY — never szal's src/*.cyr). Safe because szal never passes its own values into majra's
# workflow surface (szal implements its own engine). Full rationale + the collision set:
# docs/development/majra-vendoring.md.
#
# Re-verified against majra 2.7.0 (szal 2.1.1): no new SAME-KIND clashes, but the 2.1.1 cross-kind
# rescan (scripts/scan-collisions.sh) found one the old fn/const/var scan could not see, and rule 6
# below is new because of it.
#
#   SYS_GETRANDOM -> MJ_SYS_GETRANDOM  (NEW at 2.1.1). majra declares `var SYS_GETRANDOM = 318`
#   (dist/majra.cyr:205) — an x86_64-hardcoded literal. The stdlib declares the SAME NAME as an
#   ARCH-CONDITIONAL enum constant: 318 on x86_64-linux/macos, 278 on aarch64-linux, 45 on agnos.
#   main.cyr includes lib/syscalls.cyr BEFORE src/vendor/majra.cyr, so last-definition-wins makes
#   majra's 318 the value for the WHOLE program — including lib/patra.cyr:640 and lib/sigil.cyr,
#   both of which szal reaches (sql_store.cyr uses patra). Benign only on x86_64, where 318 == 318;
#   on aarch64/agnos it silently issues the wrong syscall. This is a `var` vs enum-constant
#   CROSS-KIND collision with matching values on the CI arch, which is precisely the class cycc
#   reports nothing for (see the doc). Same bug shape as the BYTES_PER_GB divergence caught at 2.1.0.
#   Renaming leaves majra using its own 318 internally (its uuid_generate) and hands the arch-correct
#   constant back to the stdlib. majra's hardcoded 318 is an UPSTREAM bug worth reporting.
#
# The MJ_ERR_ rename rescues ZERO live collisions as of 2.7.0 (szal self-prefixed SZAL_ERR_* at
# 2.1.0, and majra's 20 bare ERR_* happen not to overlap the 17 the stdlib closure owns) — KEEP IT
# anyway as defence-in-depth: that non-overlap is a coincidence of naming, and one future majra
# ERR_TIMEOUT / ERR_NOT_FOUND (obvious names for a queue engine) would silently retarget stdlib call
# sites. It costs 20 identifiers, is word-anchored and idempotent, and rewrites no string literals.
#
# Renames (word-boundary anchored, idempotent):
#   ERR_*     -> MJ_ERR_*       STEP_*    -> MJ_STEP_*       TRIGGER_* -> MJ_TRIGGER_*
#   uuid_generate -> majra_uuid_generate   step_result_new -> majra_step_result_new
#   SYS_GETRANDOM -> MJ_SYS_GETRANDOM
#
# NOT renamed, deliberately: the enum TYPE names StepStatus / TriggerMode also collide with
# src/step.cyr:26,35. Cyrius does not put enum type names in the flat symbol table (verified: two
# same-named integer enums keep all their distinct member values, and the compiler is silent), and
# neither name is used as a type annotation anywhere in szal. Documented so the next scan does not
# re-litigate it.
#
# After bumping majra, RE-RUN `scripts/scan-collisions.sh` — a new release may add clashes, and the
# cross-kind classes are invisible to `cyrius build --strict`.
#
# Usage: scripts/sync-majra.sh [path-to-majra-checkout]   (default ../majra)
set -eu

MAJRA="${1:-../majra}"
SRC="$MAJRA/dist/majra.cyr"
DST="src/vendor/majra.cyr"
VER="$(cat "$MAJRA/VERSION" 2>/dev/null || echo '?')"

[ -f "$SRC" ] || { echo "majra dist not found at $SRC" >&2; exit 1; }
mkdir -p src/vendor

{
  printf '# majra.cyr -- VENDORED full majra dist (do not edit; regenerate with scripts/sync-majra.sh)\n'
  printf '# Source: majra %s dist/majra.cyr, with szal-collision renames applied (MJ_ERR_/MJ_STEP_/\n' "$VER"
  printf '# MJ_TRIGGER_ prefixes + majra_uuid_generate/majra_step_result_new). See\n'
  printf '# docs/development/majra-vendoring.md. Hoosh vendor pattern (no [deps.majra] block).\n\n'
  sed -E \
    -e 's/\bERR_/MJ_ERR_/g' \
    -e 's/\bSTEP_/MJ_STEP_/g' \
    -e 's/\bTRIGGER_/MJ_TRIGGER_/g' \
    -e 's/\buuid_generate\b/majra_uuid_generate/g' \
    -e 's/\bstep_result_new\b/majra_step_result_new/g' \
    -e 's/\bSYS_GETRANDOM\b/MJ_SYS_GETRANDOM/g' \
    "$SRC"
} > "$DST"

echo "vendored FULL majra -> $DST ($(wc -l < "$DST") lines, from majra $VER)"
