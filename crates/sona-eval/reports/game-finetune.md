# GAME fine-tuned on Pifuyuu's USTs: negative result

Spec and pre-registered rule: [`.scratch/game-finetune/spec.md`](../../../.scratch/game-finetune/spec.md)
(rule at 4c889c2, seeded-measurement amendment at ff582f5, both before any dev read).

## What was trained

- **Model:** openvpi GAME-1.0-medium (v1.0.0 checkpoint, code pinned at `69ba01d`). The encoder and spectrogram
  projection were frozen; the segmenter and estimator were trained.
- **Data:** the 30 aligned songs of the spent splits (holdout, holdout2, holdout3): 28 for training (267
  segments, 63 min, 51 min sung) and 2 for validation (lady-bug, maru-shite-kudasai). No dev song was used.
- **Schedule:** 1 000 steps on CPU, AdamW at 5e-5 halved at steps 400 and 800, with pitch-shift, loudness,
  noise and spectrogram-mask augmentation. Wall time was about 9 h.
- **Checkpoint:** chosen by validation loss alone, which picked **step 600**. Later steps overfitted:

  | step | 0 | 200 | 400 | **600** | 800 | 1000 |
  |---|---|---|---|---|---|---|
  | val total_loss | 1.067 | 0.977 | 0.959 | **0.944** | 0.976 | 0.988 |

- **Export:** GAME's own `deploy.py`. Graph inputs and outputs and `config.json` are identical to the
  official ONNX.
- **Licensing:** the weights derive from CC BY-NC-SA GAME weights and from Pifuyuu's USTs. They stay local
  and are never committed.

## Measurement

- **Command:** `sona-eval-ust-dev --stem lead --per-song --cfg base:grid=est,game=0.8,oct=1`, run on the 9
  dev songs.
- **Seeded segmenter:** both arms were seeded at 1–5, the same seeds for both. The amendment explains why:
  GAME's segmenter is otherwise random run to run.
- **Scoring:** per-song cells averaged over the seeds, then pooled; the 95% interval is a 10 000-resample
  song bootstrap.

| | official GAME | fine-tuned (step 600) | difference |
|---|---|---|---|
| **cells, pooled (seed mean)** | **87.18%** | **87.98%** | **+0.80** (95% CI −0.53 to +1.91) |
| cells per seed | 87.01 · 87.36 · 87.05 · 87.33 · 87.17 | 88.41 · 87.79 · 88.09 · 87.90 · 87.71 | |
| note COnP F1 (quantized, all notes) | 0.7732 | 0.7920 | +0.0188 |

| dev song | official | fine-tuned | difference |
|---|---|---|---|
| telescope-nevermore | 93.62 | 89.47 | −4.16 |
| my-axiom | 91.52 | 90.54 | −0.98 |
| yamineko | 91.99 | 91.46 | −0.53 |
| innocent-addict | 87.94 | 87.84 | −0.10 |
| fukagyakuteki-henyou | 86.81 | 88.22 | +1.41 |
| azemichi-ni-ochita-ginga | 90.02 | 91.93 | +1.91 |
| atamaro-deluxe | 83.87 | 86.09 | +2.22 |
| kamaitachi-ni-yoru | 91.87 | 94.17 | +2.30 |
| amaten | 65.37 | 69.27 | +3.90 |

## Verdict: not adopted

| Rule | Result | Outcome |
|---|---|---|
| 1. Cells +2.0 or more, with a 95% interval excluding 0 | +0.80, interval −0.53 to +1.91 | **fail** |
| 2. At most 2 songs lose more than 2 points | one song (telescope-nevermore, −4.2) | pass |
| 3. COnP F1 drops by no more than 0.02 | it rose by 0.019 | pass |

The fine-tuned model helps most where the official one is weakest: amaten +3.9, kamaitachi +2.3, atamaro +2.2.
It costs the cleanest song 4 points. Note F1 rises a little. Pooled over all nine songs, the gain is within noise.

## What else these numbers could be measuring

- **Seed noise:** the segmenter seed alone moves pooled cells by 0.34 points (official) and 0.70 (fine-tuned).
  That is why each arm averages 5 seeds. The +0.80 is about the size of one seed's spread.
- **Validation metric artefact:** validation accuracy jumped from 0.48 to 0.79 in 200 steps. That is a
  convention shift on 2 songs (GAME's pitch and timing against Pifuyuu's notes), which sona's octave rule and
  grid snapping already absorb. It is why the decision rests on dev cells, not on validation accuracy.
- **Leakage:** none. Dev songs were in neither training nor validation, and the checkpoint was chosen before
  dev was read.

## Side finding that affects earlier numbers

Unseeded, GAME gives different notes on every run, because of the D3PM segmenter's `RandomUniformLike`. The
official model gave 68, 69 and 70 notes on three runs of one 25 s clip. ADR-0017's GAME figures are
single unseeded runs, so they carry about ±0.2–0.4 points of run-to-run noise that their song bootstraps did
not include. The holdout3 decision (+3.8 against a +5 bar) is far enough from its threshold to stand.

## Not tried (possible follow-ups, none pre-registered)

- Unfreezing the encoder.
- A larger effective dataset (more of Pifuyuu's songs, which needs the BowlRoll fetcher).
- Re-tuning the presence threshold for the fine-tuned model.
- Stopping at step 200–400 instead of 600.

Any of these would need a fresh split to confirm.
