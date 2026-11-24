# 11 — Merge wobble and scoop pieces inside one note

Status: ready-for-agent
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
