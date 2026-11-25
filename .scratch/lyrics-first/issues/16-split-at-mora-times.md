# 16 — Split and time notes at the aligner's mora times

Status: ready-for-agent
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
