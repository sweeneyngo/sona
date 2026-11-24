# 11 — Profile the pYIN front end before parallelising

Status: ready-for-agent
Type: research
Blocked by: —

## Evidence

Audit: `sona transcribe` on a 281 s stem took 155 s on one core of 16; the pYIN front end dominates
(`--no-salience` 147 s, note-HMM 172 s). The difference function (`pyin.rs:213-223`) is O(w·τmax) per
frame; nothing in sona-core is parallel.

## Proposal

Profile (e.g. `perf`/`cargo flamegraph`) one dev stem; if the difference function dominates, try an FFT
autocorrelation and/or frame-parallel processing (rayon). Output must stay **bit-identical** (or within a
stated tolerance proven not to change any note on dev): the default path is byte-identical by project rule.

## How to tell

Wall time on the same stem before/after; `output_invariants` and dev cell accuracy unchanged.

## Comments
