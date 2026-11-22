# Spec: fine-tune GAME on Pifuyuu's USTs

Status: **rule committed before any training; no fine-tuned model exists at this commit.**

## Question

Does fine-tuning openvpi's GAME-medium on Pifuyuu's hand-made USTs make its notes match his
transcriptions better on songs it never trained on? GAME was trained on 32 h of human singing
(no Vocaloid). The residual after ADR-0017 is mostly his conventions: when a note ends, when to
rest, and which doubled voice is the lead.

## Data

- **Train:** the 30 aligned songs of the spent splits holdout, holdout2 and holdout3. Their lead
  stems are paired with their lead UST notes, aligned by `alignment_offset_s`, with rests as gaps.
  All three re-reads are logged in the ledger as training use.
- **Evaluate:** the 9 dev songs, never trained on. Dev was used to pick GAME's presence threshold
  (0.8) and the octave rule's thresholds, so the base model is, if anything, favoured on dev.
- The weights derive from GAME (CC BY-NC-SA 4.0) and from Pifuyuu's USTs (no redistribution), so
  they stay local and are never committed.

## Measurement

`sona-eval-ust-dev --stem lead --cfg base:grid=est,game=0.8,oct=1` with `SONA_GAME_DIR` set first to
the official GAME-1.0.3-medium ONNX, then to the fine-tuned export. Everything else is identical,
including the frozen presence threshold of 0.8. The metric is cell accuracy over all cells, pooled
over the 9 songs, with a 10 000-resample song bootstrap.

## Pre-registered decision rule

**Adopt** the fine-tuned model as sona's recommended local GAME model (documented, opt-in, local
weights) if all of these hold:
1. Cells rise by at least **+2.0 points** over base on dev, and the 95% interval excludes 0.
2. At most 2 of the 9 dev songs lose more than 2 points.
3. Note COnP F1 (the harness's pooled, quantized all-notes number) does not drop by more than 0.02.

Otherwise record it as a negative result, with its numbers.

**Exploratory, not part of the rule:** re-tuning the presence threshold for the fine-tuned model
on dev is reported separately and labelled as dev-tuned.

**Confirmation still owed:** every split is now spent, so a result that passes here needs holdout4.
That in turn needs a BowlRoll fetcher, because the OneDrive share holds only part of Pifuyuu's USTs.
