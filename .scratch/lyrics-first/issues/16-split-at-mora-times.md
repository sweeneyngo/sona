# 16 — Split and time notes at the aligner's mora times

Status: resolved
Type: prototype
Blocked by: 14

## What

Under the user's revised weights (off-time start or end 4, `../utau-edit-cost/spec.md`), the line-placed
lyric fit is flat on dev (GAME lead 303.0 → 304.2 cost/min, stage 272.2 → 271.5): `fit_span_moras` splits
a joined note by `TAIL_CELLS` (the extra mora takes the last 2 cells), and a split a cell off costs a
length on one note and a shift and length on the next. 14's CTC aligner gives every mora an onset (78.5%
on its own UST note, 90.9% within one). Use those onsets:

- split joined notes at the aligned mora onsets (rounded to the grid) instead of the tail rule;
- where a fitted note's start is near its mora's aligned onset, try moving the start there (and the
  previous note's end, in legato);
- keep notes with no mora merged as today.

Oracles first: the same fit with the UST's own mora onsets, and with the onsets jittered by the aligner's
measured error. Report new-weight cost/min, clean measures, wrong moras/min and placed share, with paired
song-bootstrap intervals against GAME lead and the stage without lyrics, and against today's aligned fit.

## Acceptance

- Selection rule committed before the measuring.
- A report with the oracles, the candidates, and the missing-note count beside each cost.
- Any Rust stays opt-in; the default is unchanged.

## Comments

### 2026-11-24 — claimed; oracles read, selection rule declared before any aligned candidate is scored

- **Harness:** `crates/sona-eval/scripts/mora-split/split.py` on the new-weight dev dump, 14's aligned lines
  and spaces (`lyrics-lead-energy`) as the spans. It reproduces the Rust reference exactly: GAME lead no
  lyric 303.0 / today's aligned fit 304.2, stage 272.2 / 271.5, every class count equal.
- **Oracles (read; spans aligned, only the mora onsets change; GAME lead cost/min, Δ vs today's aligned fit):**
  split at the UST's onsets 290.6 (−13.7 [−24.6, −5.2]); move each note's start to its UST onset when it is
  within one cell 247.7 (−56.5); both 244.4 (−59.8 [−87.4, −34.9]); within two cells 238.3; any distance
  246.1 (pitch errors jump, 553). With the UST's own line spans too: 241.9. The same with the onsets jittered
  i.i.d. by the aligner's pooled error (57.6% of moras on the right cell, 20.8% one cell early): split +1.4,
  split + start ±1 **+164.7** (468.9). So the lever is the starts, not the splits, and it is worth ~55
  cost/min only if the onsets are right far more often than iid jitter at the aligner's error rate.
- **Candidates (declared now; all onsets from the lyrics CTC on the lead, lines and spaces aligned):**
  `al:split`; `al:start{1,2}` and `al:split+start{1,2}`, each with a constant onset shift of 0, +20 and +40 ms
  (the aligner is early by a median 0.36 cell); and the same start rules gated to moras whose aligner score is
  at or above the song's 25th/50th percentile. Anything added after reading these is reported as added
  later and judged by the same rule.
- **Selection rule.** A candidate *ships opt-in* when all of these hold on dev:
  1. GAME lead: the paired song-bootstrap (10 000 resamples) 95% upper bound of Δ cost/min against the
     no-lyric output (303.0) is **< 0**;
  2. the stage is no worse: its Δ cost/min point estimate against the no-lyric stage (272.2) is ≤ 0;
  3. missing notes not up: missing count ≤ 1.02 × the no-lyric output's on both (lead ≤ 318, stage ≤ 217);
  4. placement kept: wrong moras/min ≤ today's aligned fit + 5 on GAME lead (≤ 98.1).
  Among passers: the lowest GAME lead cost/min; within 1.0 cost/min of it, the one with fewer knobs. Nothing
  becomes a default (dev is in-sample for every knob here; confirmation belongs to holdout4 under the spec's
  rule). If none passes, the result is negative: numbers recorded, knobs deleted.

## Answer

**Negative: no candidate passes the rule, and none beats today's aligned fit.** The lyrics CTC's per-mora
onsets put 57.6% of moras on their UST note's start cell, which is too coarse to time notes on this grid.
`crates/sona-eval/reports/mora-split.md`.

- **Oracle (UST onsets, aligned spans; GAME lead):**
  - split + starts moved within one cell: **244.4** cost/min, against today's aligned fit 304.2 and no lyric
    303.0 (Δ against no lyric −58.6 [−85.5, −34.9]);
  - 19.6% clean measures, 88.7 wrong moras/min, 54.0% placed, 294 missing;
  - stage 219.4 (no lyric 272.2).
  - The splits alone are worth −13.7 against today. The starts are the lever.
- **Precision needed:** with onsets exact on a share *p* of moras, otherwise off by the aligner's own errors,
  split + starts breaks even at p ≈ 90% (305.4) and gains at 95% (265.7). The aligner's 57.6% gives 448.7.
- **Candidates (aligned onsets):**
  - split: 313.2 (+9.0 [+1.6, +21.3] against today);
  - the best start rule (±1 cell, +20 ms, gated to the upper half of the aligner's scores): 343.1 (+38.8);
  - ungated starts: 476.0.
  - The added-later 2-cell-only moves cost +8.8 to +38.9.
  - All 47 fail rules 1 and 2 (lead upper bound against no lyric < 0; stage no worse).
- Where the aligner and the note disagree by one cell, the note is usually right: 600 moves would take a
  correct start off its cell against 174 that would fix one (277 against 200 with +20 ms). The aligner fires
  ~0.36 cell early, plausibly on the consonant; a UST note starts on the vowel.
- No Rust was written (nothing earned a port); the default and every opt-in path are unchanged.
- Next: a vowel-onset source at ~90% cell precision, inside each placed line. It could be a singing phoneme
  aligner such as SOFA, or the aligner's onset used as a prior on sona's own onset candidates. Either needs the
  same precision sweep before a port.
