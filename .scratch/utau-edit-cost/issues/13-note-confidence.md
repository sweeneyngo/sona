# 13 — A confidence score that finds the expensive errors

Status: claimed
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

## Comments
