# 21 — Bring the lyric arm (14's fit + 17's S1) into one Rust command, with S1's offsets frozen from dev

Status: in progress
Type: task
Blocked by: 20

## What

20 put C + L in the holdout4 bundle. L is 14's line fit (Rust, `--lyric-placement --lyric-lines
--lyric-line-times`) plus 17's S1 start moves, which today run only in Python (`scripts/mora-split/split.py`,
variant `file-m3+adaptmora+abst3.0:split+start1`, onsets from `scripts/vowel-onsets/sources.py`). Two problems
before the read:

1. **Leak.** `sources.py`'s `debias()` fits S1's per-class offsets leave-one-song-out on the songs being scored,
   from their UST onsets. On holdout4 that reads the labels. Freeze the offsets from the 9 dev songs (all of
   them) into a small local file (`~/.cache/sona/lyric-onsets/offsets.json`; UST-derived, never committed) and
   make every later run read it. Report dev's C+L with frozen offsets: in sample now, so it should be at or
   below 271.8; say by how much.
2. **Port.** `sona_core::lyric_fit::fit_lines_with_onsets` (cut at onsets, move a start up to one cell,
   legato) and a scorer flag `--lyric-onsets DIR` (per-song onset files from the consensus: three CTCs,
   offsets, median, adaptive sigma, lattice prior, snap, abstain on spread > 3 cells). The consensus may stay
   in Python as the onset-file producer for now (the three CTCs need their own runtime; the user decides that
   only if S1 survives holdout4). Acceptance: Rust = Python per song and class on dev's `stage` rows
   (`bundle.py --dump-rows`), with the frozen offsets.

The report: `crates/sona-eval/reports/bundle-on-dev.md` (port plan section).

## Acceptance

- Frozen offsets file written, local only; `sources.py` reads it; no leave-one-song-out on scored songs left in
  the path holdout4 uses (a test or a check that fails if it does).
- `fit_lines_with_onsets` with behaviour tests through the public interface; a mutant caught; default run
  byte-identical; clippy plain and `--features sona-eval/game`; workspace tests.
- One command produces the holdout4 bundle's rows: the onset-file producer, then `sona-eval-cellstage
  --model ~/.cache/sona/cellstage-19C --edit-cost --lyric-placement --lyric-lines … --lyric-line-times …
  --lyric-onsets …`. Rehearsed on dev.

## Comments
