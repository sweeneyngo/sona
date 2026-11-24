# 17 — The ffmpeg start-up probe can time out on a busy machine and turn separation off

Status: ready-for-agent
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
