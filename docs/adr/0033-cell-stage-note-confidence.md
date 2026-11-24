# ADR-0033: The learned cell stage's note confidence

Status: accepted, 2026-11-24. Ticket: `.scratch/utau-edit-cost/issues/13-note-confidence.md`.
Builds on [ADR-0008](0008-confidence-subscores-softmin.md) (the scorer) and
[ADR-0026](0026-learned-cell-stage-in-rust.md) (the cell stage). Study: `crates/sona-eval/scripts/edit-cost-taxonomy/conf.py`.

## Context

A per-note confidence should tell a user where to look before fixing the notes in OpenUtau. The costliest
fixes there are joined notes (two UST moras under one note) and chopped ones (one held note in pieces),
weight 4 each. For the cell stage's notes the confidence said nothing: they reach the scorer through
`grid_segments`, which fixes onset strength, drop steepness, dispersion and voicing, so 91–99% of notes
scored 0.60–0.65, and flagging the lowest 30% caught 31% of the edit cost on dev, as random flags would
(utau-edit-cost 03).

## Decision

1. **`cellstage::note_confidence`**: for every stage note, `1 − σ(2.2738 − 4.4420·p + 0.5818·ln(cells))`,
   where `p` is the mean over the note's cells of the model's top class probability (softmax of all six
   logits) and `cells` its length. It is the chance the note is neither joined nor chopped, from a logistic
   model fitted on the dev songs' shipped notes. Long notes are where hidden moras sit; unsure cells are
   where pieces get cut.
2. **`game::cell_stage_notes`** returns the notes with these confidences (`StageNotes`), and
   **`game::stage_annotation_notes`** turns them into annotation notes: timing, pitch and sub-scores exactly
   as before (`ConfidenceScorer` on `stage_segments`), the stage's confidence as the scalar. `sona
   transcribe --cell-model` and `sona serve --cell-model` use it. The notes, cells and edit cost are
   unchanged; only the scalar moves.
3. **Not changed:** pYIN grid mode, the note-HMM and GAME without the stage keep the scorer's soft-min.

## Evidence (dev, 9 songs; `conf.py`)

Selection rule committed before measuring (ticket 13): each song flags its lowest 30%, pooled share of
attributable edit cost caught; the winner must beat both the stage's class probability and random with a
song-bootstrap 95% interval (paired) on each difference excluding 0.

| signal, cost caught when x% flagged | 5% | 10% | 20% | 30% | 40% | 50% |
|---|---|---|---|---|---|---|
| random | 4.9 | 9.9 | 19.9 | 29.9 | 40.0 | 50.0 |
| old confidence | 6.2 | 11.5 | 21.5 | 30.9 | 40.6 | 51.5 |
| stage class probability | 7.5 | 16.4 | 30.6 | 43.1 | 54.2 | 63.0 |
| note length | 10.7 | 19.4 | 36.9 | 47.2 | 57.0 | 65.3 |
| **new, leave-one-song-out** | 8.8 | 17.9 | 33.2 | **47.5** | 60.0 | 69.5 |
| new, as shipped (all-dev fit) | 9.0 | 18.2 | 33.2 | 47.6 | 60.4 | 69.6 |

At 30%: 47.5% [43.5, 52.7]; +4.4 [+1.8, +6.0] over the class probability and +17.6 [+13.6, +22.7] over
random. It catches 51% of the joined cost and 42% of the chopped. No single feature or unfitted blend
cleared the class probability. The spent splits (diagnosis only, the score fitted on dev): +4.4 [+3.4, +5.5]
on holdout3, where the stage is out of sample; +3.8 [−0.2, +8.4] on holdout; +2.0 [−2.5, +5.6] on holdout2.

## Consequences

- The scalar of a stage note is no longer the soft-min of its sub-scores. The sub-scores stay the scorer's,
  so the editor's snap escape hatch (`onset_clarity`) and the "why" in the inspector are unchanged; the
  inspector still names the weakest sub-score, which for a stage note is not what lowered the scalar.
- In the editor the heatmap now spreads: on dev, stage notes run from 0.22 to 0.90 (median 0.80) instead of
  0.60–0.65 (the top, a one-cell note the model is sure of, is 0.90, so no stage note is full green). At the
  default flag threshold (0.50) 2.2% of notes are outlined (previously under 1%: the few near-floor notes the bleed guard caught); 0.70
  outlines 15%, 0.80 about half.
- Verified: a dev `--dump` re-run of `sona-eval-cellstage` gives the prototype's f32 value on all 4 700
  notes and the same curve; its stdout (cells, edit costs) equals ticket 03's run, and the stage's notes,
  cells, probabilities and sub-scores in the dump are unchanged.
- The weights were fitted on dev, where the stage itself is in sample (its probabilities are sharper than
  on new songs). holdout3 shows the same gain, but nothing confirms it; holdout4 can.
- The GAME-only path is flat for the same reason; length alone catches 46.9% of its cost at 30% on dev
  (random 30%). Changing it needs its current confidence measured first.
