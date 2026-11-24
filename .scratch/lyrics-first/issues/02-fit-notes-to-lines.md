# 02 — Fit the notes to the pasted lines

Status: claimed
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

### 2026-11-24 — claimed; design and selection rule (declared before reading any result)

- Text: the UST's moras with 01's line and space breaks (perfect text; only the placement is new). Phrases =
  lines and spaces (the spec's input rule); lines alone reported beside it.
- Placement: a Viterbi over lines × note boundaries of the output notes whose midpoint lies in the UST's span
  (the user starts the lyric at the first sung note). A line's cost is the rest-aware fit's DP cost of its
  moras on its notes, with cells per mora fixed at the song's; a line starting where two notes touch costs
  `pb` more; notes outside every line cost 2 each. Then each line is fitted with `fit_span_moras` over its
  notes' span. `pb` ∈ {0, 1, 3}, chosen on dev and leave-one-song-out.
- **Ship rule (port to sona-core, opt-in):** on GAME lead, wrong moras per minute below (d) today's with a
  paired interval excluding 0, and the edit cost not raised (paired Δ against no lyric with an upper bound
  under +5 cost/min), for the leave-one-song-out choice; the stage must not get worse either. The gap to the
  ceiling (b+/r: −13.5, 83.4 wrong/min) is reported.
