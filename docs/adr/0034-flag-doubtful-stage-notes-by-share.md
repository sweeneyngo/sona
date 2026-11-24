# ADR-0034: Flag a learned-stage result's doubtful notes by share, and say why

Status: accepted, 2026-11-24. Ticket: `.scratch/utau-edit-cost/issues/14-flag-doubtful-notes.md`.
Builds on [ADR-0033](0033-cell-stage-note-confidence.md) (the stage's confidence) and
[ADR-0008](0008-confidence-subscores-softmin.md) (the threshold and the sub-scores). Study:
`crates/sona-eval/scripts/edit-cost-taxonomy/doubt_reasons.py`.

## Context

ADR-0033 gave the learned cell stage's notes a confidence that ranks the costly fixes (joined and chopped
notes) well, but sits mostly above any fixed cutoff: on dev it runs 0.22–0.90 with a median of 0.80, so the
editor's default flag (confidence under 0.50) outlines 2.2% of stage notes. The inspector's "why" names the
weakest sub-score, which for a stage note is a constant of `grid_segments` (usually "uncertain offset 0.50")
and has nothing to do with its confidence. pYIN and GAME-only confidences are soft-mins of real or constant
sub-scores and mean something else, so they keep what they have.

## Decision

1. **Which results.** A result is a learned-stage result when its provenance says `engine: game+cells`, or
   (an older file without provenance) when its notes carry `stage_doubt`. Everything else is flagged as
   before; the editor checks that pYIN and GAME-only results draw the same pixels and tooltips as before.
2. **Flag by share.** A stage result outlines each song's `round(share × notes)` least confident notes,
   ties to the longer note; the share is a Settings slider (0–50%, step 5, default **20%**) that replaces
   the threshold slider for these results only, and it is saved with the session and undone like the
   threshold. A note the user has edited keeps its place in the ranking but loses its outline, so fixing a
   flagged note never outlines another. The escape hatch's outline is unchanged.
3. **Why, in two words.** ADR-0033's score is `1 − σ(b0 + b_prob·p + b_len·ln cells)`. Against a one-cell
   note the model is certain of, its log-odds of a split or merge rise by two non-negative terms:
   **length** `b_len·ln(cells)` and **uncertainty** `−b_prob·(1 − p)`. The core emits both per stage note
   (`cellstage::note_doubt`, the annotation's `Note.stage_doubt { length, unsure }`), and the inspector
   names the larger, for an outlined stage note only: "long note, may hide several syllables" or "the model
   was unsure here". It never names a sub-score for a stage note.
4. **Schema.** `stage_doubt` is optional (`skip_serializing_if = None`, `serde(default)`), so pYIN and
   GAME-only annotations are byte-identical and older ones still load; like `provenance` and `bar_one` it
   adds a field without breaking a reader, so `SCHEMA_VERSION` stays 1. The contract fixture carries it.

## Evidence (dev, 9 songs, in sample; `doubt_reasons.py` on 13's dump)

Flagging 20% per song as the editor ranks them catches **33.3% [29.6, 38.1]** of the attributable edit cost
(ADR-0033's table: 33.2% at 20%, random 20%). Of the 940 flagged notes, 59% say "long" and 41% "unsure", and
the reasons sort the errors:

| reason | flagged notes | cost caught | of joined cost | of chopped cost |
|---|---|---|---|---|
| long note | 556 | 19.7% | **25.9%** | 8.8% |
| model unsure | 384 | 13.6% | 6.8% | **27.4%** |

So "long" points at hidden syllables (a join to split) and "unsure" at a note cut in pieces. At 30% the
split is 69/31% and the catch 47.7% [43.9, 52.7]. No holdout was read; the weights are ADR-0033's dev fit.

## Consequences

- Twenty percent is a starting point, not a measured optimum: the catch grows about linearly with the share
  (ADR-0033's table), so the right share is how many notes a user will check. It is one slider away.
- The threshold slider still exists for every other result. The sub-score rows stay in the inspector for
  stage notes, though they don't explain the score.
- An older stage annotation (no `stage_doubt`) is flagged by share once its provenance says so, but its
  inspector falls back to the weakest sub-score; re-transcribing gives the reasons.
- `effectiveNotes` takes an optional `flagShare`; without it, it is the threshold code path unchanged.
