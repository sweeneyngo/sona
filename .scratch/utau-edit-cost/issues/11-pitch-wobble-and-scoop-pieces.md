# 11 — Merge wobble and scoop pieces inside one note

Status: claimed
Type: research
Blocked by: 03

## What

03: chopped is 12–24% of GAME lead's cost. Besides same-pitch restarts (07), the extra pieces are 1–2
semitone steps from vibrato, glide or drift (33–38%), and a one-cell piece at the note's start from a
scoop or consonant (17–28%). The learned stage chops more than GAME on every split and adds octave flips.

## Steps

- Oracle per subclass: merge exactly the pieces that are chopped in truth, then measure the cost removed.
- Mechanism: merge a short piece into its neighbour when it is within 1–2 semitones and the pitch track
  shows a glide or vibrato rather than a step; absorb a one-cell piece at a note's start into the note.
  Check how many true short notes each rule eats (they become joined, weight 4).
- Opt-in flag; dev numbers with intervals.

## Rules

Same as 07. Measure the combination with 07 before claiming either.

## Comments

- 2026-11-24 (claimed). **Selection rule, declared before any measurement.** A mechanism is worth porting
  if on dev it lowers GAME lead's pooled cost/min with a song-bootstrap 95% interval on the paired
  difference (mechanism minus baseline, 10 000 resamples of the 9 songs) that excludes 0, and it does not
  raise the learned stage's pooled cost/min (point estimate). Its thresholds are chosen leave-one-song-out
  on dev, and the out-of-fold numbers are the ones the rule is applied to. It is also measured combined
  with 07's opt-in `--fit-phrase-lyrics`, and the report states whether the gains add. Oracles (merging
  exactly the truly chopped pieces of a subclass) are ceilings and never port. For each rule, the number of
  true short notes it eats (turned into joined notes, weight 4) is reported. Dev only; spent splits are
  diagnosis only (logged re-reads) and confirm nothing; holdout4 is not read. Anything ported is opt-in
  and the default output stays byte-identical. Same-pitch restarts belong to 07 and are not redone here.
