# 13 — A confidence score that finds the expensive errors

Status: resolved
Type: prototype
Blocked by: 03

## What

03: the shipped confidence score carries no information. 91–99% of the shipped notes score 0.60–0.65,
because the cell stage's notes pass through `grid_segments` with fixed onset, offset, pitch-stability and
voicing inputs. Flagging the lowest 30% catches 30–32% of the cost, the same as random. The stage's own
class probability catches 44–48%, and "longest notes first" catches 60–78% of the joined cost.

Build a score from features already in the dumps (class probability, note length against the beat, notes
per second nearby, GAME/stage agreement) and pick it by the share of edit cost caught in the lowest 30%.
Nothing in the app highlights doubtful notes until a score beats random clearly.

## Acceptance

- Dev curve: share of cost caught vs share of notes flagged, for the old score, each feature and the new
  score, with song-bootstrap intervals. Fit on dev; the spent splits only as logged diagnosis.
- If a score wins, port it to `cell_stage_notes`' confidence and reproduce the curve in Rust.

## Selection rule (committed before any new measurement, 2026-11-24)

- **Metric.** Each song flags its lowest-scored 30% of shipped notes (learned stage; ties broken by a seeded
  random order). Cost caught = attributable edit cost of the flagged notes, 03's attribution (every output note
  gets an equal share of its component's cost; missing notes are unattributable). Pooled over the 9 dev songs:
  sum caught / sum attributable. The full curve (5, 10, 20, 30, 40, 50% flagged) and the joined / chopped
  shares are reported beside it, per-song and globally ranked.
- **Candidates.** Single features from what `cell_stage_notes` already has (class probability, note length,
  notes per second nearby, GAME / sona agreement, pitch-track and energy shape inside the note), and one tiny
  logistic model (at most 4 features) fitted **leave-one-song-out** on dev: each song is scored by a model that
  never saw it. A feature's direction is fixed by what it should mean (low probability, long, busy = doubtful),
  not fitted.
- **Win.** A candidate wins if its catch at 30% beats **both** the stage mean class probability **and** random
  (30% of the cost by construction), with the song-bootstrap 95% interval (10 000 resamples, paired by song)
  on each difference excluding 0. Among winners, the one with the fewest features whose point estimate is
  within 2 points of the best.
- **Then.** A winner is ported as the cell stage's note confidence (notes and cells unchanged) and the Rust
  curve must reproduce the prototype's exactly from a `--dump` re-run. No winner: record the numbers, port
  nothing. The spent splits are diagnosis only (holdout3 is the stage's only out-of-sample split and is
  labelled so); holdout4 is not read.

## Answer

**A score wins and is ported** (ADR-0033): the learned stage's note confidence is now
`1 − σ(2.2738 − 4.4420·p + 0.5818·ln(cells))`, where `p` is the mean top class probability over the note's
cells: the chance the note is neither joined nor chopped, from a logistic model fitted on dev. Flagging each
song's lowest 30% catches **47.5% [43.5, 52.7]** of the dev edit cost (leave-one-song-out), against 30.9% for
the old score, 43.1% for the class probability and 30% for random: **+4.4 [+1.8, +6.0]** over the class
probability and +17.6 [+13.6, +22.7] over random, so it passes the rule above. Only the scalar changed.

**Dev curve** (9 songs, 4 700 stage notes, cost caught when each song flags its lowest x%; 03's attribution):

| signal | 5% | 10% | 20% | 30% | 40% | 50% | joined / chopped at 30% |
|---|---|---|---|---|---|---|---|
| random | 4.9 | 9.9 | 19.9 | 29.9 | 40.0 | 50.0 | 30 / 30 |
| old confidence | 6.2 | 11.5 | 21.5 | 30.9 | 40.6 | 51.5 | 28.8 / 33.6 |
| stage mean class prob | 7.5 | 16.4 | 30.6 | 43.1 | 54.2 | 63.0 | 36.2 / 52.9 |
| stage min class prob | 9.4 | 18.1 | 31.8 | 44.1 | 54.9 | 63.6 | 40.4 / 49.5 |
| length (cells) | 10.7 | 19.4 | 36.9 | 47.2 | 57.0 | 65.3 | 65.5 / 19.7 |
| expected moras (s × GAME-vocals notes/s nearby) | 9.9 | 20.2 | 37.0 | 49.2 | 57.6 | 65.0 | 70.4 / 13.5 |
| notes/s nearby (sona) | 5.7 | 10.9 | 20.2 | 30.0 | 39.4 | 47.8 | 32.1 / 26.8 |
| sona starts inside the note | 9.4 | 17.5 | 30.4 | 42.0 | 49.8 | 57.3 | 54.2 / 20.2 |
| source restarts inside | 10.1 | 18.5 | 32.5 | 43.5 | 51.3 | 58.8 | 55.5 / 21.8 |
| energy dip inside (dB) | 7.2 | 15.2 | 30.2 | 44.5 | 55.0 | 64.4 | 59.1 / 20.8 |
| pitch track spread (st) | 5.0 | 10.8 | 23.0 | 35.1 | 45.9 | 56.6 | 33.1 / 39.0 |
| not GAME lead's pitch | 9.4 | 16.2 | 25.7 | 35.4 | 44.6 | 53.5 | 32.9 / 39.6 |
| rank blend, length + class prob (no fit) | 11.9 | 20.6 | 33.7 | 44.6 | 55.9 | 65.3 | 55.8 / 23.0 |
| **new, LOSO** | 8.8 | 17.9 | 33.2 | **47.5** | 60.0 | 69.5 | 51.0 / 42.3 |
| new, as shipped (all-dev fit; Rust) | 9.0 | 18.2 | 33.2 | 47.6 | 60.4 | 69.6 | 51.6 / 42.0 |

All rows: `conf.py DEV_DUMP --model`. At 30%, against the class probability (paired song bootstrap):
length +4.1 [−2.9, +9.9], expected moras +6.1 [−3.0, +14.4], energy dip +1.3 [−8.8, +8.3], rank blend
+1.5 [−4.4, +5.2]: no single feature or unfitted blend clears it. The two-feature logistic wins with every
target tried (any error +2.7 [+1.0, +4.2]; cost-weighted +3.7 [+1.8, +5.1]; joined or chopped +4.4 [+1.8, +6.0],
the best point estimate, so the rule takes it). Three- and four-feature variants (notes/s nearby, energy dip,
source restarts) added nothing (44.5–47.4%). Length and class probability are complementary: length finds
the joins (65%), probability the chops (53%); the model trades a little of each for more of the total.

**Robustness (dev).** Each note's own cost (its joins plus a share of each UST note it chops) instead of an
equal share of the component: +4.6 [+2.0, +6.3]. Per song the new score beats the class probability on 7 of 9
(not azemichi, telescope). **One global ranking** (a fixed threshold over all songs) shrinks the gain: 49.5%
against 47.9% (+1.6).

**Spent splits (diagnosis only; ledger entries at 737bdb7; model fitted on dev, so out of sample for the
score everywhere; the stage itself is out of sample only on holdout3)**, gain over the class probability at
30%: **holdout3 +4.4 [+3.4, +5.5]** (46.7% caught), holdout +3.8 [−0.2, +8.4], holdout2 +2.0 [−2.5, +5.6].
Length alone has a higher point estimate on all three (51.3 / 45.9 / 49.1%) but intervals that cross the class
probability, as on dev.

**Port.** `cellstage::note_confidence` (pure, tested without weights); `game::cell_stage_notes` returns
`StageNotes { notes, confidence }`; `game::stage_annotation_notes` keeps the scorer's timing, pitch and
sub-scores and sets the stage's confidence. `sona transcribe/serve --cell-model` use it. Verification: a dev
`--dump` re-run gives the prototype's f32 value on **all 4 700 notes** (`conf.py DUMP --ported-check`, 0
mismatches) and the same curve; the run's stdout (cells per song and phase, edit cost per class) is
identical to 03's verified run, and the dump's stage notes, GAME notes, cells, classes, probabilities and
sub-scores are bit-identical to 03's (pYIN-side timestamps differ by ≤ 3e-14 s, a float round-trip of the
pYIN cache, untouched here). Tests in `crates/sona-core/tests/stage_confidence.rs` (+ the stage's output
invariant now goes through `stage_annotation_notes`); four mutants (length sign flipped, probability averaged
over all cells, the app keeping the soft-min, confidence above 1) each fail them.

**What users see.** The editor colours each note by confidence (red 0 → green 1), outlines notes below the
Flag threshold slider (default 0.50) in red, and shows the weakest sub-score when a note is clicked; its
snap escape hatch reads `onset_clarity`. For `--cell-model` jobs: the colours now spread (dev 0.22–0.90,
median 0.80, instead of 0.60–0.65), 2.2% of notes are outlined at 0.50 (15% at 0.70, half at 0.80); the
inspector and the snapping are unchanged, so the inspector's "why" (weakest sub-score, usually "uncertain
offset" 0.50) does not explain a stage note's scalar. Other producers are unchanged.

**Not changed, with numbers.** GAME without the stage goes through the same `grid_segments` constants, so
its confidence is flat too; length alone catches 46.9% of its dev cost at 30% (+17.0 [+14.5, +20.0] over
random). Its current confidence isn't in the dump, so the paired comparison is still to do. pYIN grid's
confidence has real HMM inputs; length catches only 35.8% there.

**For the plan.** (1) The gain is dev-fitted and not holdout-confirmed; holdout3 agrees. holdout4 could
confirm it with a rule like this one. (2) The default flag threshold (0.50) outlines only 2% of stage notes;
whether to move it, or to flag by share ("check the doubtful 20%"), is a product decision. (3) The inspector
should say why a stage note is doubtful ("long note: may hide moras" / "the stage was unsure here") rather
than the weakest sub-score. (4) Give the GAME-only path the same treatment after dumping its confidence.

## Comments
