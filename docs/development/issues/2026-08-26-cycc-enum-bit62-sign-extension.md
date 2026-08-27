# cycc sign-extends enum-constant initialisers from bit 62

**Status:** 🔴 OPEN upstream (cyrius). Worked around szal-side at 2.1.1.
**Affects:** cycc **6.5.31 → 6.5.35** (at least). 6.5.2 – 6.5.30 are correct.
**Found:** 2026-08-26, during the szal 2.1.0 → 2.1.1 toolchain bump.
**szal impact:** 5 test suites + 1 fuzz harness failed; the engine silently ran every step
without a timeout *and* skipped every step as "flow timeout exceeded".

## Symptom

An `enum` constant whose initialiser has **bit 62 set** is sign-extended from bit 62 instead of
being taken as a plain 64-bit value. `var` initialisers and inline expression literals are
unaffected — which is why this hid for four minor releases.

| initialiser | cycc 6.5.2 | cycc 6.5.35 |
|---|---|---|
| `enum { K = 0x3FFFFFFFFFFFFFFF }` | `0x3fffffffffffffff` | `0x3fffffffffffffff` ✅ |
| `enum { K = 0x4000000000000000 }` | `0x4000000000000000` | `0xc000000000000000` ❌ |
| `enum { K = 0x7FFFFFFFFFFFFFFE }` | `0x7ffffffffffffffe` | `0xfffffffffffffffe` ❌ |
| `enum { K = 0x7FFFFFFFFFFFFFFF }` | `0x7fffffffffffffff` | `0xffffffffffffffff` (**-1**) ❌ |
| `enum { K = 9223372036854775807 }` | `0x7fffffffffffffff` | `0xffffffffffffffff` ❌ |
| `var K = 0x7FFFFFFFFFFFFFFF` | correct | correct ✅ |
| inline literal `0x7FFFFFFFFFFFFFFF` | correct | correct ✅ |

## Minimal reproduction

```sh
printf 'include "lib/string.cyr"\ninclude "lib/fmt.cyr"\ninclude "lib/syscalls.cyr"\ninclude "lib/io.cyr"\nenum E { M = 0x7FFFFFFFFFFFFFFF; }\nvar a = fmt_int(M);\nvar b = print("\\n", 1);\nvar c = syscall(60, 0);\n' > /tmp/m.cyr
cyrius build --strict --no-deps /tmp/m.cyr /tmp/m && /tmp/m
CYRIUS_HOME=$HOME/.cyrius/versions/6.5.2 $HOME/.cyrius/versions/6.5.2/bin/cyrius \
  build --strict --no-deps /tmp/m.cyr /tmp/m2 && /tmp/m2
```

```
-1                      <- cycc 6.5.35
9223372036854775807     <- cycc 6.5.2
```

## How it broke szal

szal had exactly one enum constant in the affected range:

```
src/step.cyr:52   enum StepSat { STEP_I64_MAX = 0x7FFFFFFFFFFFFFFF; }
```

It is the **"no timeout" sentinel** for the whole engine, and Cyrius `>` / `>=` are **signed**, so
folding it to `-1` inverted two separate guards:

1. `src/engine_step_exec.cyr:96` — `if (timeout_ms >= STEP_I64_MAX)` selects the synchronous
   *no-timeout* path. At `-1` **every** step took it, so no step could ever time out. A timed-out
   step then returned a NULL error pointer, and `tests/szal_engine_step_exec.tcyr:142`
   dereferenced it → SIGSEGV (exit 139), which also masked the 22 assertions after it.
2. `src/engine_runner.cyr:131` — `_engine_timeout()` returns `STEP_I64_MAX` as the *unbounded*
   sentinel. At `-1` the flow-deadline guard `clock_now_ms() - start_ms > timeout_ms`
   (`engine_sequential.cyr:52`, `engine_parallel.cyr:136`, `engine_dag.cyr:137`,
   `engine_hierarchical.cyr:62`) is **always true**, because any elapsed time ≥ 0 > -1. Every step
   was Skipped with reason `"flow timeout exceeded"`.

Observed failures: `szal_engine_runner`, `szal_engine_subflow`, `szal_engine_step_exec`,
`szal_engine_hardware`, `szal_engine_parallel_stress`, and `fuzz/step_json` (whose
`fz_check(d <= STEP_I64_MAX, ...)` at `fuzz/step_json.fcyr:434` fails for any positive `d`).

The narrower per-executor suites (`szal_engine_sequential` / `_parallel` / `_dag` /
`_hierarchical`) kept passing because they call `run_*()` with *literal* timeouts and never reach
`_engine_timeout` — which is why this looked like an include-count or symbol-collision problem at
first. It is neither.

## The trap that made this hard to see

`~/.cyrius/bin/cyrius` resolves `cycc` through `~/.cyrius/current`, **not** through the manifest
pin — and it does so even when you invoke `~/.cyrius/versions/<pin>/bin/cyrius` directly. Only
`CYRIUS_HOME` overrides it. So with `~/.cyrius/current` = `6.5.35` and `cyrius.cyml` pinned to
`6.5.2`, local builds compiled with **6.5.35's cycc against 6.5.2's `lib/` snapshot**, while CI —
which installs exactly the pin — used 6.5.2 throughout and stayed green.

The compiler does warn, but it is buried in szal's warning wall:

```
warning: cyrius.cyml pins 6.5.2 but cycc is 6.5.35 — toolchain drift (snapshot may be stale;
         set CYRIUS_NO_WARN_PIN_DRIFT=1 to silence)
```

**Check that warning first** whenever local and CI results disagree. To reproduce CI exactly:

```sh
CYRIUS_HOME=$HOME/.cyrius/versions/<pin> $HOME/.cyrius/versions/<pin>/bin/cyrius build ...
```

## szal-side workaround (shipped in 2.1.1)

`src/step.cyr` — one line, and it fixes all six failures *under the broken compiler*:

```diff
-enum StepSat { STEP_I64_MAX = 0x7FFFFFFFFFFFFFFF; }
+var STEP_I64_MAX = 0x7FFFFFFFFFFFFFFF;
```

`var` initialisers are immune to the bad fold. `StepSat` was referenced nowhere else, and
`STEP_I64_MAX` is only ever used as a *value* (never as an array size), so the kind change is
behaviour-preserving. **Do not convert it back to an enum until this is fixed upstream.**

A `scripts/scan-collisions.sh`-style guard is not enough here — this is a codegen bug, not a
symbol clash. The durable check is: *no enum constant anywhere in szal's include closure may have
bit 62 set.* As of 2.1.1 the closure — szal `src/`, all three vendored dists, `tests/`, `fuzz/`,
`benches/`, and the whole cyrius 6.5.35 stdlib snapshot — contains **zero** such constants.

## Suggested upstream fix

The fold that materialises an enum member's value is sign-extending from bit 62 rather than 63
(or is round-tripping through a 63-bit signed intermediate). `var` initialisers take a different
path and are correct, so the two should be reconciled onto the `var` path's behaviour. Worth a
cycc regression test over the boundary set `{2^62 - 1, 2^62, 2^63 - 2, 2^63 - 1}` in both hex and
decimal spelling.
