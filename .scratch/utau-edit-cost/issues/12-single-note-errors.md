# 12 — Start, end and pitch errors on single notes

Status: resolved
Type: research
Blocked by: 07

## What

03: errors on single notes (length, shift, pitch) are 19–26% of GAME lead's cost. Each is cheap to fix
in OpenUtau (weight 1), but there are many. Break them down by cause (snapping to the wrong cell, a late
consonant onset, a release, an octave) after 07 changes the note boundaries, since 07 moves them.

## Selection rule (declared 2026-11-24, before measuring)

Work on the default output (07 left the default boundaries unchanged). Dev only for decisions; the spent splits
(holdout, holdout2, holdout3) may be re-read from 03's dumps for diagnosis only, logged in the ledger as a
re-analysis. holdout4 / holdout4-tempo are not touched.

- Baseline to reproduce first: dev 180.2 (stage, in-sample) / 188.7 (GAME lead, the out-of-sample reference) /
  338.4 (pYIN) cost/min.
- For each candidate cause (wrong-cell snap / late consonant onset, release / offset placement, min-length or
  merge rules, octave, semitone from vibrato centre or scoop, the UST author's conventions): measure the cost an
  oracle fix removes and whether evidence available at transcription time predicts it.
- **Port** a mechanism only if dev GAME-lead pooled cost/min drops with a paired song-bootstrap 95% interval
  (10 000 resamples) on the difference that excludes 0, the stage's pooled cost/min does not rise, and neither
  output's pooled clean-measure share falls.
- A port is opt-in (default output byte-identical), reproduces the prototype's per-song class counts exactly,
  has behaviour tests through the public interface and a mutant shown to be caught.
- Leave chopped/joined to 07/11. Negative results are recorded with their numbers.

## Comments

## Answer

Report: `crates/sona-eval/reports/single-note-errors.md`. Dev only; the spent splits were re-analysed from 03's
dumps for diagnosis (ledger at 8fc6ae5). The baseline was reproduced byte for byte (188.7 / 180.2 / 338.4).

- **Breakdown (GAME lead, dev).** Pairs carry 22.3% of the cost (1 155 units): length 717, shift 218, pitch
  203, moved 17. Oracles, as cost/min removed (share of 188.7):

  | cause fixed perfectly | GAME lead | stage | spent splits (lead) |
  |---|---|---|---|
  | **release: note runs through the UST's rest before the next note** | **−9.5 [−11.5, −7.5] (5.1%), clean 16.2 → 23.1%** | −2.0 | −4.3 / −8.1 / −8.5 |
  | every end | −15.0 (8.0%) | −8.0 | −7.7 / −14.6 / −11.9 |
  | starts at legato boundaries, late / early | −8.5 / −5.6 | −9.0 / −5.8 | |
  | every start | −16.3 (8.7%) | −16.6 | −10.4 / −11.5 / −9.5 |
  | octave | −2.2 (amaten has 49 of 57) | −1.0 | −1.3 / −5.5 / −13.3 |
  | ±1 semitone | −3.7 | −3.2 | −3.1 / −2.5 / −2.3 |
  | every pair error | −41.0 (21.7%) | −32.3 | −26.7 / −38.3 / −41.4 |

- **Release.** 263 notes start on time and run into a UST rest; 211 fill it to the next note. The UST's short
  rests are 2/4/6 cells: this is the author's notation, but the energy really falls there (median 0.16 and
  0.28 of the note). The vocals' energy predicts it (cell AUC 0.82 against a right note's last cell). The pYIN
  voicing does not (0.68), because UTAU puts the next consonant at the end of the note before it. The stage
  has mostly learned releases already.
- **Ported (opt-in, GAME notes only):** `sona_core::release::trim_releases`, with `sona-eval-cellstage
  --trim-releases`. It ends a note at the first cell under 0.3 of its loudest when every later cell stays
  under 0.8, dropping at least 2 cells.
  - GAME lead **188.7 → 184.3, −4.4 [−6.5, −2.5]**, clean measures 16.2 → 18.7%. Leave-one-song-out:
    −3.7 [−5.4, −1.9]. Spent splits (diagnosis): −1.2, −2.5 and −3.2, every interval excluding 0.
  - On the stage it fails the rule (+0.4 [−0.3, +1.4], clean −1.1 [−2.2, −0.1]), so it is not applied there.
  - The port reproduces the prototype on 18/18 song × output rows, and the default run is byte-identical.
    8 behaviour tests; 4 mutants caught (no tail check, cut one late, `min_cells` ignored, median level).
- **Negative.**
  - Snapping: no global rounding bias beats the shipped one, and ±10 ms shifts lose. Start errors are
    symmetric jitter (raw onsets a median 0.21 cell past the rounding point).
  - GAME's own phase: −2.4 [−6.5, −0.0], all on atamaro's triplet grid; clean −0.4, so it fails.
  - Pitch-track boundaries: 44 fixes against 663 breaks.
  - The snap rules (minimum length, monophonic trim) set 2 of 747 erroneous boundaries.
  - Releases: pYIN-voicing trims +14 … +29; one-cell energy trims lose; a learned trim gives −0.5 out of fold.
  - Semitones: GAME's pitch is ≥ 0.3 off centre for 60% of ±1 errors (4% of right notes), but the track picks
    the direction too rarely (fixes 23, breaks 11).
  - Octaves: the harmonic ratio breaks more than it fixes.
- **What changes the plan.**
  - On the shipped stage, the remaining single-note cost is rounding jitter and semitone choice. They are
    predictable as *doubtful* (a raw boundary near the rounding point, a pitch off centre) but not in
    direction, so they belong to ticket 14's flags.
  - `trim_releases` could become an opt-in for `sona transcribe --game`; a default needs holdout4.
  - The eval's GAME lead snaps on the vocals' onset phase, while the app's `snap_to_grid` uses GAME's own
    phase. They differ by −2.4 on dev, all of it atamaro.
