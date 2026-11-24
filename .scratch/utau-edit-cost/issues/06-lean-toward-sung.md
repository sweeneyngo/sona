# 06 — Lean toward a note when unsure

Status: claimed
Type: prototype
Blocked by: 01, 03, 07

## What

Deleting an extra note (weight 1) is cheaper than drawing a missing one (weight 2). Add an opt-in
sung/rest threshold to the learned cell stage and sweep it on dev against weighted edit cost. Check the
lyric-shift Fog in `map.md` before claiming a win: extras shift pasted lyrics.

## Acceptance

- A dev sweep table (cost, clean measures, cell accuracy) with song-bootstrap intervals.
- Default unchanged; the knob stays opt-in until holdout4.

## Comments

### Selection rule (committed before the Rust sweep; dev only)

Disclosure: a Python prototype on a dev `--dump` (rest bias, fills, oracles) ran before this rule was written.
It is exploration; the Rust sweep below is the measurement the rule reads.

- **Knobs swept on the learned stage** (where the knob lives; the stage is **in-sample on dev**, so a pass is
  a dev pass only and a default still needs holdout4):
  (a) rest bias b ∈ {0.5, 1, 2}: subtracted from rest's logit before the masked argmax;
  (b) a cell no source proposes a pitch for, with P(rest) < τ, τ ∈ {0.1, 0.2, 0.3, 0.5}, becomes sung at the
  neighbouring note's pitch, as its own note or extending the neighbour.
  GAME's presence threshold is not swept: the prototype's ceiling (any subset of GAME's unvoiced notes promoted,
  chosen with the UST) is −2.9 cost/min on GAME lead, and a threshold selects a subset.
- **Ships (opt-in only)** if one setting has, on the stage without lyrics: pooled cost/min Δ vs the unmodified
  stage with song-bootstrap 95% upper bound < 0, **and** pooled clean measures not below the unmodified stage's;
  **and** stacked with lyric placement (lines and spaces, rests hold moras), its cost/min Δ point estimate ≤ 0
  and wrong moras/min not up (the lyric-shift Fog). Choice among passing settings: lowest pooled cost/min.
- Otherwise: record the numbers and delete the knob.
