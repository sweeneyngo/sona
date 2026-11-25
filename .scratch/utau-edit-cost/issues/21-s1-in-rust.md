# 21 — Bring the lyric arm (14's fit + 17's S1) into one Rust command, with S1's offsets frozen from dev

Status: resolved
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

## Answer

`crates/sona-eval/reports/bundle-on-dev.md`, section "The bundle as holdout4 will run it (utau-edit-cost 21)".
Dev only (9 songs). Neither holdout4 nor the spent splits were read.

1. **The leak is closed.**
   - `s1_onsets.py freeze` fits the per-class offsets on all 9 dev songs into
     `~/.cache/sona/lyric-onsets/offsets.json` (local only).
   - `produce` writes the onset files from the alignments, the kana and that file. It reads no UST time and
     no dump.
   - `s1_onsets.py check` runs `produce` on kana-only copies of the text files (the output must be
     byte-identical) and with the offsets moved (every song must change). It also refuses offsets fitted
     outside dev. On dev it passes. Two mutants are caught: leave-one-out offsets read from the text, and
     the offsets ignored.
   - `sources.py` now needs `--offsets` or `--loso`, and `--loso` is refused unless every song is dev.
2. **Dev with frozen offsets.** With C's fold models, C + L is **271.7, −20.0 [−41.1, −1.5]** against C. That
   is 0.1 below 271.8: one length error, from 24 of 5 336 onsets changing.
   - On 19C, the holdout4 stage, which is in sample on dev: C 269.5 → C + L **255.7, −13.8 [−43.0, +10.7]**.
     The interval crosses zero.
   - S1's own share holds there (−11.2 [−20.4, −1.8] on top of 14's fit). 14's fit underneath is what
     weakens: −2.5 alone, and +47.7 on my-axiom.
   - Without amaten: −12.5 [−26.5, +3.1] with the fold models, −2.4 [−17.6, +16.2] on 19C.
3. **Rust = Python** on 9/9 songs and all nine classes, with C's fold models:
   - the fit alone, on 17's onset files (271.8);
   - the whole path, with frozen offsets (271.7): the consensus and fit in Rust against `sources.py
     --offsets` + `bundle.py`.

   The default report, and the report with 20's arm flags, are byte-identical to before.
4. **The consensus moved into the scorer.** Its lattice prior needs GAME lead's starts, which only a scorer run
   has. A Python consensus would cost holdout4 a second scored run. The holdout4 command is:

   ```sh
   L14_WORK=W python3 -I crates/sona-eval/scripts/vowel-onsets/s1_onsets.py check --offsets ~/.cache/sona/lyric-onsets/offsets.json
   L14_WORK=W python3 -I crates/sona-eval/scripts/vowel-onsets/s1_onsets.py produce --offsets ~/.cache/sona/lyric-onsets/offsets.json --out ONSETS
   sona-eval-cellstage --model ~/.cache/sona/cellstage-19C --edit-cost --split holdout4 --reason "…" \
     --lyric-placement --lyric-lines LINES --lyric-line-times TIMES --lyric-onsets ONSETS
   ```

   This was rehearsed on dev from fresh directories, and the reports are byte-identical to the runs. C + L is
   the S1 row.
5. **Code.**
   - `sona_core::lyric_fit::{fit_lines_with_onsets, OnsetRule::S1, consensus_onsets, OnsetConsensus::S1,
     lattice_prior}`.
   - 12 behaviour tests, kana-free. Eight mutants were caught, one per rule.
