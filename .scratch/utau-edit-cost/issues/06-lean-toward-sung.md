# 06 — Lean toward a note when unsure

Status: resolved
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

## Answer

**Negative; the rule fails and the knob is deleted** (it lived at 43d9b8d). Report:
`crates/sona-eval/reports/lean-toward-sung.md`. Dev only, the stage in-sample.

- Rest bias on the stage's masked argmax: +1.2 [+0.2, +2.4] cost/min at b = 0.5, rising with b. It lengthens
  notes into the UST's rests. The unbiased stage is the minimum.
- Cells the model believes sung but no source pitches (P(rest) < τ), drawn at the touching note's pitch: up to
  100 of 213 missing notes go, but chops (own note) or joins (extend) come back. Best +0.9 [−3.3, +4.1].
  A perfect per-run choice made with the UST is worth only −3.9 to −4.4.
- GAME's presence is where the notes are dropped (86% lie under an unvoiced GAME note), but that note's pitch is
  right 18% of the time. Any presence threshold is worth at most −2.9 on GAME lead (oracle), so GAME was not
  re-run.
- With lyric lines and spaces placed (rests hold moras; stage 167.4): best +0.1 [−3.6, +2.8]. Fills place
  slightly more moras (−1 to −3 wrong/min), so extras do not steal moras here, but cost does not fall.
- Missing notes are quiet moras next to a sung note (ん 25–34% vs 1% of matched notes; pitch track unvoiced). The
  model knows they are sung (1 − P(rest) AUC 0.90–0.96), but nothing gives their pitch or their boundary with the
  neighbour. That is the joined-notes problem (07), not a threshold.
