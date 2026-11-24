# Map: UTAU edit cost

Spec: `spec.md`. Tickets: `issues/NN-*.md`. A ticket is on the frontier when it is open, unblocked (every
`Blocked by:` ticket is resolved) and unclaimed; the lowest number goes first. Claim by setting
`Status: claimed` before any work. Resolve by appending `## Answer`, setting `Status: resolved`, and adding
a line below.

## Notes

- Order: metric and frame measurement (01, 02) in parallel, then the baseline (03). The baseline decides
  which of 05–07 is worth doing. 04 (BPM first) needs no measurement and can run any time.
- Parallel agents work in separate git worktrees off the same green commit and never touch a holdout.

## Decisions so far

- 2026-11-24: the target is cheap fixes in OpenUtau, not cell accuracy (user). Tiers and weights in
  `spec.md`.
- 2026-11-24: users usually know their BPM, so ask for it first (user).

## Fog

- **Lyrics.** A pasted lyric is split into mora and laid on notes in order, so one extra or missing note
  shifts every later mora in the phrase. If users paste lyrics in sona, extras may not be cheap at all.
  Measure the share of phrases whose note count equals their mora count once 01 exists.
- **Tempo changes and time signatures** (roadmap 2.2): the corpus has none by construction. Needs songs
  that have them before anything can be measured.
- **Time signature:** UST files don't carry one. 4/4 is assumed for bar offsets; 3/4 and 6/8 songs would
  be scored wrongly.
- **Do OpenUtau's MIDI and UST imports place notes the same way?** Unchecked. If users import MIDI, check
  the bar lines survive.
