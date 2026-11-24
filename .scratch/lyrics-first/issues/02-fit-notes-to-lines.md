# 02 — Fit the notes to the pasted lines

Status: resolved
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

## Answer

Negative: **sona's notes cannot place the lines.** Report section "02" in
`crates/sona-eval/reports/lyric-placement.md`. Dev, GAME lead (stage alike):

| landing | cost/min (Δ vs no lyric) | wrong moras/min | placed |
|---|---|---|---|
| today, pasted in order | 188.7 (0) | 183.2 | 4.9% |
| lines and spaces placed by the UST (01's ceiling) | 175.2 (−13.5 [−31.1, −2.7]) | 83.4 | 56.7% |
| placed from sona's notes, pb 0 (leave-one-song-out on edit cost) | 189.7 (+0.9 [+0.0, +2.0]) | 177.2 | 8.0% |
| … pb 3 | 197.7 (+9.0 [+3.4, +15.1]) | 146.5 | 24.0% |
| best placement of 12 settings (pb 3, duration term, skips) | 199.0 | 144.6 | 25.0% |

- The Viterbi drifts: 29% of line starts on the right note (pb 3), and an offset carries to every later line.
  Only songs whose line breaks are long rests place well (telescope, yamineko 83–88%).
- The fit's costs say which note hides a mora, not how many notes a line takes; a mora per note is free, so
  lines take notes from the next. On amaten GAME lead hears 21 notes in one section's 80 moras.
- **Not ported.** The declared ship rule passed on its letter (pb 0: −6.0 [−12.2, −1.1] wrong moras per
  minute, +0.9 cost), but it places 8% against the ceiling's 57%, and every setting placing more raises the
  edit cost. The rule should have asked for a share of the ceiling; noted for the next one.

**Next:** the line timing has to come from the audio (align the pasted kana to the vocal; ticket 05's models
may give timestamps) or from the user (mark each line's first note: 14 marks per minute on dev, against 183
moras per minute retyped today). Which one is a product decision for the user. Either way, the fit after it is
01's `fit_span_moras`, and 04 is blocked on that choice.
