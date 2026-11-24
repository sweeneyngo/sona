# 17 — The ffmpeg start-up probe can time out on a busy machine and turn separation off

Status: resolved
Type: task
Blocked by: —

## Evidence

A10: at load average ~40, the 5 s `ffmpeg -version` probe added in A12 (`crates/sona-cli/src/serve.rs`, cached
once at start-up) timed out, so several serve tests failed (including old ones); they pass with fewer test
threads. On a user's busy laptop the same timeout would report ffmpeg as missing and switch vocals and lead
separation off for the whole session, with a misleading "install ffmpeg" note.

## Proposal

Tell "not found / exits non-zero" apart from "slow": a missing binary fails fast (spawn error), so only that
should mean missing. On a timeout, treat ffmpeg as present-but-unverified (or retry lazily on first use) and
never cache a timeout as "missing". Test with a stub ffmpeg that sleeps past the limit: capabilities still
offer separation; a stub that exits 1 or a missing path still reports missing.

## How to tell

The slow-stub test passes; a mutant that caches a timeout as missing fails it; serve tests pass under
`RUST_TEST_THREADS` at the default with the machine loaded.

## Comments

## Answer

Done on branch `audit-a17-ffmpeg-probe` (from 692b950). Recorded in [ADR-0030](../../../docs/adr/0030-stem-cache-cleanup-and-job-failures.md)
(item 5, evidence and mutants).

**Behaviour:**
- Before: `ffmpeg -version` that took over 5 s was the same as no ffmpeg. Separation was off for the whole
  session, and the notes and 409s said "install ffmpeg".
- After: `provenance::tool_run` tells the three outcomes apart (`Printed`, `Failed`, `TimedOut`). Only
  `Failed` (the spawn fails, or it exits non-zero) turns ffmpeg off. A timeout counts as present. It is
  never cached as missing, and `sona serve` prints on stderr that it wasn't verified. A slow ffmpeg is
  still used for real work, where each run has its own job deadline (item 4).
- `tool_stdout` is unchanged on top of it, so the stem-cache key's version probe (20 s limit a tool)
  still reads a timed-out ffmpeg as `unknown`. No lazy re-probe: when the first real use fails, the job
  fails with ffmpeg's own error, as any job does.

**Tests:**
- `serve.rs::an_ffmpeg_that_fails_is_missing_but_a_slow_one_is_not`:
  - a stub that exits 1 is `missing`, and separation is off;
  - a stub whose `-version` sleeps past the limit makes start-up wait about 5 s. After that,
    capabilities offer vocals and lead, there is no ffmpeg note, nothing says "install ffmpeg", and a
    48 kHz WAV is queued (201, not 409).
- The existing `/nonexistent/ffmpeg` test still reports `missing`.
- `provenance::tests::only_a_slow_tool_times_out` checks the three outcomes directly.

**Mutants (each caught):**
- `ffmpeg_runs = matches!(probe, Printed(_))`, which caches a timeout as missing: the new serve test
  fails with "a slow ffmpeg is not a missing one" and separation all false.
- `tool_run` reports a non-zero exit as `TimedOut`: the new serve test (broken stub) fails, and so does
  the unit test.

**Under load** (16 cores, load average 35 to 43, default test threads, `RUST_TEST_THREADS` unset):
- `cargo test -p sona-cli --release --test serve` before the change: 17/17 passed in 11.1 s. The A10
  timeout didn't reproduce at this load.
- After the change, three runs: 18/18 each time, in 12.7 s, 11.4 s and 9.6 s.
- To force the case, the test wrapper's `-version` was made to sleep 6 s (temporary, not committed):
  - with the fix, 18/18 passed in 22.8 s;
  - on the base code, 14 of 18 failed: every test that needs separation or ffmpeg.
  - So the tests that don't check ffmpeg no longer depend on how fast the probe is. The cost is at most
    5 s of start-up per server.

**Gates:**
- `cargo clippy --workspace --all-targets -- -D warnings` is clean.
- `cargo test --workspace --release` passes.
- `verification/serve-jobs/check.sh` passes, with all 7 mutants caught.
- `scripts/ci/check-no-corpus-data.sh` passes.

**Left as is:**
- The probe still runs synchronously before "serving", so a slow ffmpeg delays start-up by up to 5 s.
- An ffmpeg that exits 1 only after more than 5 s (a broken one on a very loaded machine) now counts as
  present. Its first job then fails with ffmpeg's error, not with the up-front 409.
