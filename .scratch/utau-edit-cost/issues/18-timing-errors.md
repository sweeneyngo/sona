# 18 — Off-time starts and ends under the revised weights

Status: resolved
Type: prototype
Blocked by: —

## What

The user revised the weights (2026-11-24, `spec.md`): an off-time start or end costs 4, a near-miss pitch 3.
Rescored on dev, timing (length + shift + moved) is 45% of GAME lead's 303.0 cost/min and 40% of the
stage's 272.2. Oracles (`scripts/edit-cost-taxonomy/taxo.py`, new weights): fixing every length pair cuts
44% (lead) / 39% (stage); every shift/moved pair 24% / 27%.

1. Taxonomy of the timing errors on both outputs: start vs end, direction, size in cells, context (legato
   boundary, before a rest, after a rest, phrase start), note length, and which source the stage took
   the boundary from. Oracles per slice (for example, only one-cell errors; only ends before a rest).
2. Prototype fixes from audio evidence and nothing that reads the UST: energy offsets (12's release trim
   on the stage, retuned for the new weights), onsets on the lead stem, agreement between GAME and the
   stage's boundaries, or anything the taxonomy points to.

Lyric-based timing (the aligner's mora onsets) belongs to lyrics-first 16; leave it out.

## Acceptance

- Selection rule committed before the Rust sweep.
- A report with the taxonomy, oracles and candidates. Show the missing-note count beside each cost,
  since an off-time note (4) now costs more than deleting and redrawing it (3).
- Default unchanged; anything that ships is opt-in until holdout4.

## Selection rule (declared 2026-11-24, before the Rust sweep)

Dev only (9 songs); no holdout of any kind is read. Revised weights (`spec.md`). Baseline reproduced first:
GAME lead + octave **303.0** cost/min (out of sample, decides), learned stage **272.2** (in-sample, shown).
Candidates read only audio evidence (GAME's raw notes on the lead and vocals, sona's grid notes, the pitch
track, the vocals' or lead stem's energy) and the grid; the UST is read only by the oracles and the scoring.

A candidate is ported (opt-in) only if, on dev GAME lead:

1. the paired song-bootstrap (10 000 resamples) 95% upper bound of Δ cost/min is below 0;
2. clean measures do not fall (point estimate Δ ≥ 0);
3. it does not win by dropping notes: the missing count does not rise, or, if it does, the upper bound of
   Δ cost/min stays below 0 with missing re-weighted from 2 to 4 (a dropped note then costs as much as an
   off-time one);
4. a tuned parameter is chosen leave-one-song-out, and the leave-one-song-out result also has its upper
   bound below 0.

If it raises the stage's cost (point estimate), it is for GAME notes only, as 12's trim is. Among candidates
that pass, the lowest GAME-lead cost/min is ported; passing candidates are then measured together. A port
reproduces the prototype's per-song class counts exactly, leaves the default run byte-identical, and has
behaviour tests through the public interface with a mutant shown to be caught. Negative results are recorded
with their numbers; knobs that earn nothing are deleted.

## Comments

## Answer

Report: `crates/sona-eval/reports/timing-errors.md`. Dev only; no holdout read. Baseline reproduced byte for byte
(GAME lead 303.0, stage 272.2; default run `cmp`-identical after the port).

- **Taxonomy.** Timing is 45.7% of GAME lead's cost and 40.2% of the stage's. Almost all of it is one-cell
  jitter at legato boundaries: one boundary a cell off costs up to 12. Fixing only the one-cell errors is worth
  −93.4 cost/min on GAME lead (stage −93.3), against −134.2 / −105.1 for every start and end. The rest is GAME's
  release: 225 notes run into the UST's short rest (−38.1). The stage has mostly learned that (−7.9).
- **No audio source says which way the jitter goes.** At GAME lead's legato boundaries (6.6% wrong), GAME on the
  vocals, sona's grid, the pitch track, voicing and energy each break more right boundaries than they fix. The one
  informative signal is GAME's own rounding residual: boundaries 0.4–0.5 cell from their cell are wrong 40% of the
  time, and 84% of those lean toward the UST.
- **Passes the rule, ported opt-in: `sona_core::near_ties::settle_near_ties` (`--settle-near-ties`).**
  - The rule: an edge whose raw GAME-lead time leans ≥ 0.35 cell one way moves a cell that way when GAME's run on
    the full vocals has an edge there.
  - GAME lead 303.0 → **297.8 (−5.2 [−8.4, −2.3])**. Clean measures +0.4 [+0.0, +1.0]. Missing +2 (upper bound
    −2.2 with missing at 4). Leave-one-song-out picks 0.35 on 9/9 folds; 7/9 songs improve.
  - It adds the same on top of 12's trim: −5.1 [−8.3, −2.2].
  - Stage: −0.6 [−3.1, +1.5], clean −0.3 (in-sample). That is not a rise, so the rule does not restrict it, but
    it earns nothing there. It is for GAME notes.
  - Port = prototype on 18/18 song rows. 9 behaviour tests; 5 mutants caught.
  - A first version measured −6.8 / −2.6. That included an artefact: overlapping notes that the scorer's trim
    turned into fewer one-cell pieces. The port keeps one voice, and the rule was re-swept and re-chosen
    leave-one-song-out on the valid version.
- **Fails the rule: 12's release trim retuned for the new weights.** Every setting also cuts quiet sung
  (devoiced) moras, turning joined into missing.
  - Leave-one-song-out: −9.5 [−18.7, +2.3], missing +39. 12's own setting is now −8.6 [−19.7, +4.3], missing +55.
  - Never on the stage. The lead stem's energy is no better.
  - So `--trim-releases` no longer meets its own bar under the revised weights. That is the user's call; it was
    left in place.
- **Negative:** GAME-latency correction (−8.2 [−22.9, +1.6], clean −0.4); averaging the two GAME listens (LOSO −2.2
  [−5.2, +1.4]); near-tie moves toward sona's edges (+1.8 … +22.0).
- **Recommendation, not done.**
  - The stage's timing ceiling from a better choice among its own sources is −41.7 (every edge or one-cell edges:
    −105.1 / −93.3, in-sample).
  - A retrain that charges a boundary error as the edit cost does (12 per legato boundary, not one cell of
    cross-entropy), with GAME lead's rounding residual as a feature, is the lever. It needs non-dev training data
    and a holdout4 read.
  - The residual is also a timing doubt mark for 14. Lead boundaries with a residual of 0.3 cell or more are 10.8%
    of boundaries and hold 49% of the wrong ones.
