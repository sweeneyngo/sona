# 02 — Fit the notes to the pasted lines

Status: ready-for-agent
Type: research
Blocked by: 01

## What

Build the mechanism 01 recommends (`crates/sona-eval/reports/lyric-placement.md`):

- Phrases are the pasted lines, and the spaces within a line (01: lines and spaces with the UST's timing
  reach −13.5 cost/min on GAME lead and 83 wrong moras/min, against −14.1 and 74 for the UST's own phrases).
- Place each line on a run of sona's notes **without the UST's timing**. Lines and sona's rests disagree
  (49% of line breaks sit on a GAME lead rest, 36% of its rests are breaks), so a line break cannot be snapped
  to a rest: use a song-level DP over lines × note boundaries, scored by the fit's own cost of each line's
  moras on its notes, with a break where notes touch allowed (a penalty, tuned on dev).
- Fit each line with `sona_core::lyric_fit::fit_span_moras` (rests inside it may hold moras). The shipped
  `fit_phrase` on honest phrases raises the cost (+16.5 on UST phrases).
- Port the placement to `sona-core`; opt-in in `sona-eval-cellstage` (a landing beside 01's) until holdout4.

## Acceptance

Dev table, GAME lead (out of sample) beside the stage: edit cost and lyric placement for the realistic
landing against 01's (d) today, (b+/r) lines placed by the UST (the ceiling) and no lyric; song-bootstrap
intervals; the penalty chosen leave-one-song-out too. Port reproduces the prototype.

## Comments
