# 01 — Weighted edit-cost metric

Status: ready-for-agent
Type: task
Blocked by: —

## What

Extend `sona_eval::correction` (`crates/sona-eval/src/correction.rs`, port of `web/lib/correction.ts`) into
an edit-cost scorer for transcription vs UST, weighted by the tiers in `../spec.md`:

- extra note 1, wrong pitch (any interval, octave included) 1, wrong length 1, single-note shift by up to
  one cell 1, missing note 2;
- **chopped** (one UST note covered by two or more output notes) 4 per extra piece, **joined** (one output
  note covering two or more UST notes) 4 per hidden note. Neither exists yet; define them on grid cells.
- report cost per minute, per-class counts, and **clean measures** (share of 4/4 measures with zero cost).

Add `--edit-cost` output to `sona-eval-ust-dev` (per song and pooled, song-bootstrap 95% interval), or a
new `sona-eval-edit-cost` binary if that is cleaner. Keep the weights in one table so they can change.

## Acceptance

- Unit tests on synthetic note sets: each class alone gives its exact weight; a known mix gives the sum.
- Break the chopped and joined detectors once each and show a test fails (rules.md: mutants caught).
- `web/lib/correction.ts` and the Rust scorer still agree on shared fixtures for the classes both have.
- Runs on `dev` only. No holdout read in this ticket.
- clippy -D warnings, `cargo test --workspace --release` green.

## Comments
