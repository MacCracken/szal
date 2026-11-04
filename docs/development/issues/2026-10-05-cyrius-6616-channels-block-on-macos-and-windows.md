# cyrius 6.6.16 — `chan_send` / `chan_recv` block on arm64 macOS and Windows too (behaviour change; information)

**Status:** 🟡 open — information for szal's next cyrius pin bump; no szal defect. Recorded by cyrius 6.6.16
(lane thr-1), 2026-10-05.
**Severity:** none expected — the new behaviour is the contract szal's own comments already document.
**Checked against:** szal **2.2.0** (pin `cyrius = "6.6.6"`), read only, not measured.

> **Progress at szal 2.2.1 (2026-11-03, pin `cyrius = "6.6.20"`):** the pin is past 6.6.16. The stale
> `chan_try_send` sentence is fixed in `src/stream.cyr` and parity-notes §9 (comment and note only; the
> hub still blocks). On Linux x86_64, `tests/szal_stream.tcyr`, `szal_engine_parallel.tcyr` and
> `szal_engine_parallel_stress.tcyr` pass, with the rest of the 47 suites. **Still open:** nothing has run
> them on arm64 macOS or Windows — CI is `ubuntu-latest` only — and that run is this issue's closing
> condition.

> **At szal 2.3.0 (2026-11-03, pin `cyrius = "6.7.9"`):** unchanged — the Linux suites still pass;
> macOS / Windows still not run.

~~⛔ Nothing to do until cyrius 6.6.16 is tagged and out and szal's pin moves to it.~~ (done at 2.2.1)

## What changed in cyrius

Through cyrius 6.6.15 only Linux had a locked, blocking channel. On arm64 macOS (whose `thread_create`
starts real pthreads) and on Windows (whose threads are real `CreateThread` threads) the channel was the
serial ring: no lock, `chan_recv` answered 0 at once when empty, and `chan_send` returned -1 when full.
From cyrius 6.6.16 arm64 macOS and Windows have the Linux contract — `chan_recv` blocks until a value
arrives (0 once closed and empty), `chan_send` blocks while full (-1 once closed) — and every thread peer
exports `CHAN_BLOCKING` (1 on Linux, arm64 macOS and Windows; 0 on x86 macOS, agnos and cx). 6.6.16 also
fixed a lost-wake-up deadlock in the Linux channel itself (senders and receivers parked on one futex word
with wake-one; a saturated producer could sleep for ever with room in the ring), and makes
`THREADS_CONCURRENT` read 1 on Windows.

## What it means for szal

- **`src/stream.cyr` `_hub_publish`** fans each event out with a blocking `chan_send` on every subscriber
  channel. Its header (:12-14) already documents that contract ("Cyrius `chan_send` BLOCKS when a
  subscriber's channel is full … a wedged slow subscriber back-pressures the producer"), but on arm64
  macOS and Windows a full subscriber channel used to DROP the event (`chan_send` returned -1). From 6.6.16
  the publisher blocks there too, as on Linux. Worth running szal's stream tests on macOS and Windows at
  the bump.
- **The same header says "Cyrius has no non-blocking `chan_try_send`"** — it has had one since cyrius
  6.4.84 (`chan_try_send` / `chan_try_recv`), which a subscriber that must never block the publisher could
  use. The comment is stale on any pin szal supports.
- **`src/engine_parallel.cyr` `run_parallel`'s `max_concurrency` permit semaphore** is a `chan_new(N)`
  channel pre-filled with N tokens (acquire = `chan_recv`, release = `chan_send`, :53-56, :80-83). Before
  6.6.16 an empty permit channel on arm64 macOS or Windows answered 0 at once, so acquire never waited and
  the cap limited nothing there; on arm64 macOS the ring was also updated without a lock. From 6.6.16 the
  cap holds on both targets with no change to szal. (From reading the source; not measured.)

## Closes when

szal's pin reaches ≥ 6.6.16 and its stream / parallel tests pass on macOS and Windows; fix the stale
`chan_try_send` comment then.
