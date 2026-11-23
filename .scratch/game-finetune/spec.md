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

## Amendment (2026-11-22, before training finished and before any dev read)

**Finding.** GAME's segmenter graph has an unseeded `RandomUniformLike` node (D3PM drops boundaries at
random), in the official ONNX too. ONNX Runtime seeds it per process, so the same stem gives different
notes run to run: the official medium model gave 68, 69 and 70 notes on three runs of the same 25 s
clip. A segmenter seeded with a fixed `seed` attribute gives identical notes on every run. Every GAME
number so far (including ADR-0017's holdout reads) carries this run-to-run noise, which the song
bootstrap did not include.

**Measurement, replacing the single run above.**
- Both arms are copied with the segmenter seeded (`~/.cache/sona/game-ft/scripts/seed_segmenter.py`),
  for seeds **1, 2, 3, 4, 5**, the same seeds for both arms. Nothing else in the graphs changes.
- Each song's cell accuracy is the **mean over the 5 seeds**. The rule's pooled cells, its 95% song
  bootstrap (10 000 resamples) and the per-song losses are computed on those seed means.
- Reported alongside, as the noise floor: the base model's pooled cells for each seed (min–max), and
  the same for the fine-tuned model.

**Which fine-tuned weights.** Training runs 1 000 steps (`finetune.yaml`: LR 5e-5 halved at 400 and 800,
encoder frozen). The checkpoint is the one with the lowest validation `total_loss` on the 2 held-back
*training-split* songs (lady-bug, maru-shite-kudasai), chosen before dev is read. Dev is read once for
that checkpoint; no other checkpoint is scored on dev.

The decision rule itself is unchanged.

## Result (2026-11-22): not adopted

Step 600 (lowest validation loss) against official GAME-1.0.3-medium, 5 seeds each, 9 dev songs. Cells
87.18% → 87.98%, +0.80 (95% CI −0.53 to +1.91): **rule 1 fails**. One song lost more than 2 points
(telescope-nevermore, −4.2): rule 2 passes. COnP F1 0.7732 → 0.7920: rule 3 passes. Recorded as a
negative result: [`crates/sona-eval/reports/game-finetune.md`](../../crates/sona-eval/reports/game-finetune.md).
