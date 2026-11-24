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

## Comments
