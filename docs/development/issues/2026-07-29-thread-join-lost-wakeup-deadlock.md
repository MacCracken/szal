# `thread_join` lost-wakeup deadlock — `lib/thread.cyr`

**Status:** 🟡 **OPEN upstream · WORKED AROUND in szal (2026-07-29)** — szal routes every join
through `szal_thread_join` (`src/engine_step_exec.cyr`); no szal code calls `thread_join` directly.
**Filed:** 2026-07-29 during the szal Rust→Cyrius port (M2 engine arc, parallel executor).
**Severity:** High — **permanent process deadlock**, no error, no timeout, no diagnostic.
**Affects:** every Cyrius consumer of `lib/thread.cyr` that calls `thread_join` on a short-lived
worker. Frequency scales with join count; szal hit it ~1 per 2,000 parallel `engine_run` calls
(~1 per 150,000 joins).
**Toolchain:** cyrius `6.5.2` (`lib/thread.cyr:293`); the code is unchanged since the v5.4.10
child-trampoline fix, so every 5.4.10+ release is affected.
**Platform:** Linux (x86_64 + aarch64 — the futex path). Windows/macOS/agnos route to the serial
fallbacks and are unaffected.

## Summary

`thread_join` reads the thread's tid word **twice** — once for the loop condition, once for the
`FUTEX_WAIT` expected-value argument:

```cyrius
fn thread_join(t): i64 {
    if (t == 0) { return 0 - 1; }
    while (load64(t) != 0) {
        syscall(SYS_FUTEX, t, FUTEX_WAIT, load64(t), 0, 0, 0);
    }                        # ^^^^^^^^^^ second, independent load
    munmap_stack(load64(t + 8), load64(t + 16));
    return 0;
}
```

A worker that exits **between** the two loads deadlocks its joiner permanently:

| # | joiner | worker / kernel | `*t` |
|---|---|---|---|
| 1 | `load64(t) != 0` reads the live tid → enters the body | | `tid` |
| 2 | | worker exits; `CLONE_CHILD_CLEARTID` stores 0 and issues `futex_wake(t, 1)` — **no waiter is parked yet, so the wake is discarded** | `0` |
| 3 | argument `load64(t)` reads **0** | | `0` |
| 4 | `FUTEX_WAIT(t, 0)`: kernel compares `*t == val`, `0 == 0` → **parks** | | `0` |

The only wake this word will ever receive fired at step 2. The joiner sleeps forever.

The in-code comment argues the opposite — *"Re-reading directly via `load64(t)` each iteration
(rather than caching into a local) keeps the `FUTEX_WAIT` expected-val in lockstep with the
condition check"* — and the intent is right; it is the *implementation* that breaks it. Two
independent loads are not in lockstep; **one** load feeding both uses is.

## Evidence

`/proc/<pid>/syscall` of a hung szal process (read from the tracee's parent shell, since
`ptrace_scope=1` allows only an ancestor):

```
202 0x7f7702fc4800 0x0 0x0 0x0 0x0 0x0 0x7ffd0ed23060 0x43b629
 ^   ^              ^   ^
 |   uaddr          |   val   = 0   <- impossible expected tid
 |                  futex_op = 0x0 <- FUTEX_WAIT with NO FUTEX_PRIVATE_FLAG
 SYS_FUTEX
```

`FUTEX_WAIT` **without** `FUTEX_PRIVATE_FLAG` (0x80) identifies the call site uniquely:
`mutex_lock` (`lib/sync.cyr:64`), `chan_send` (`lib/thread.cyr:352`) and `chan_recv`
(`lib/thread.cyr:450`) all pass `FUTEX_WAIT | FUTEX_PRIVATE_FLAG`. `thread_join` is the only
non-private waiter — deliberately so, because `CLONE_CHILD_CLEARTID`'s wake is a *shared* futex
wake. And `val = 0` is a tid that can never be observed live: it is exactly the post-exit value.

The hung process had **one** thread left (the joiner) — every worker had already exited, which is
the same conclusion from the other direction.

## Reproduction

szal's parallel executor, ~1 hang per 6 runs of a 400-iteration loop (each iteration =
`engine_run` on a 10-step `FLOW_PARALLEL` flow = 20 joins). Minimised, the shape that matters is
a worker whose exit is tightly correlated with its joiner's arrival — spawn, have the child
publish a result and immediately exit, and join the moment the result is visible:

```cyrius
var ch = chan_new(1);
var t = thread_create(&child, ch);        # child: chan_send(ch, 1); return 0;
var got = 0;
while (got == 0) { got = chan_try_recv(ch); }
thread_join(t);                           # races the child's SYS_EXIT
```

Driven from 24 concurrent driver threads on a 16-core host (oversubscription widens the window —
a joiner preempted between the two loads gives the worker a whole scheduling quantum to exit in),
this hangs within ~10,000 joins. `tests/szal_engine_parallel_stress.tcyr` phase C is exactly this
loop; against the pre-fix tree it caught the deadlock on **12 of 12** runs.

## Suggested upstream fix

Load the tid **once per iteration** and use that value for both the loop test and the
expected-value, so the joiner can only ever park on a tid it has positively observed to be live.
If the worker exits in the window before the syscall, `*t` is 0 ≠ `tid`, the kernel returns
`EAGAIN` instead of parking, and the loop re-loads:

```cyrius
fn thread_join(t): i64 {
    if (t == 0) { return 0 - 1; }
    var tid = load64(t);
    while (tid != 0) {
        syscall(SYS_FUTEX, t, FUTEX_WAIT, tid, 0, 0, 0);
        tid = load64(t);
    }
    munmap_stack(load64(t + 8), load64(t + 16));
    return 0;
}
```

No contract change: still a shared (non-private) `FUTEX_WAIT`, still munmaps the stack after the
worker is confirmed dead, still returns -1 for a 0 handle.

## szal-side workaround

`lib/` is provisioned by `cyrius lib sync` and overwritten on every toolchain bump (and CLAUDE.md
forbids editing it), so the fix cannot live there. szal defines `szal_thread_join` in
`src/engine_step_exec.cyr` — the body above, verbatim — and **all four** szal join sites use it:

| Call site | What it joins |
|---|---|
| `src/engine_step_exec.cyr` `_run_attempt` | the per-attempt handler timeout worker |
| `src/engine_parallel.cyr` `run_parallel` | a `_par_worker` at join-in-spawn-order |
| `src/engine_dag.cyr` `run_dag` | a `_par_worker` at end-of-wave |
| `src/engine_distributed.cyr` `run_distributed` | a `_dist_worker` after the coordinator loop |

`engine_step_exec.cyr` is the first thread consumer in the include order, so the shim is in scope
for the other three in `main.cyr` and in every `tests/szal_*.tcyr`. Regression guard:
`tests/szal_engine_parallel_stress.tcyr`.

**Remove the shim only when the pin moves to a toolchain carrying the upstream fix** — and then
verify with that stress suite, not by inspection.
