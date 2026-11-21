# Grid mode evidence: harmonic salience and a spectral re-articulation cue (opt-in)

**Status: Accepted as opt-in.** Both features are behind flags until a fresh holdout confirms them. The holdout ADR-0015 read is spent (`crates/sona-eval/corpus/holdout-reads.jsonl`).

## Context

After ADR-0015, the dev-split error analysis (`crates/sona-eval/reports/grid-cell-decoder.md`) left two large, separate gaps:

- **Pitch where voices overlap.** Two thirds of the UST's notes sit in polyphonic regions. There, pYIN's single f0 is the lead's pitch in only 32% of frames. Grid mode decoded 28% of those lead cells as a rest and 18% at another pitch.
- **Same-pitch re-articulations.** 22% of monophonic notes repeat the previous pitch on a new syllable. The voicing/energy cue separated them from held notes with an AUC of only 0.69.

Both were attacked in parallel by a 4-worker swarm from `c1d81b5`, each on its own branch, scored the same way, and dev only:
- W1 re-tuned the decoder. It raised note F1 but lowered cell accuracy, so it was rejected.
- W2 tried lyrics (see the report).
- W3 and W4 are below.

## Decision

Two opt-in evidence tracks for `GridDecoder`, combined through `GridDecoder::segment_with_evidence`:

1. **Harmonic-salience pitch evidence** (`GridDecoder::candidates`, from W3).
   - **What:** Melodia-style harmonic summation (`pitch::HarmonicSalience`, pure Rust, its own radix-2 FFT, no new dependency) gives up to 5 salient f0s per frame.
   - **Note cost:** a capped Gaussian to the candidates, with a penalty for weaker ones, scaled by frame strength.
   - **Rest cost:** `2·strength²`. pYIN's own terms are weighted 0.5.
   - **Phase:** still estimated from the contour alone.
   - **Why it works:** the top salience candidate is the lead in 63% of polyphonic frames, against pYIN's 32%. Letting the decoder follow *weaker* candidates scored lower, so only near-ties to the strongest are followed.
2. **Spectral re-articulation cue** (`GridDecoder::cue_model = CueModel::Spectral`, from W4). It needs the `spectral` feature.
   - **What:** a 3-feature logistic model on spectral flux (±20 ms), low-band flux (±40 ms) and the cepstral distance across the grid line. A held pitch re-articulates where the log-odds reach −1.5.
   - **Weights:** fit on the dev songs' same-pitch lines. Under nested leave-one-song-out, AUC per song rose 0.69 → 0.80 and precision at the operating point 0.23 → 0.39.
   - **Scope:** it only splits held notes. It never changes the decode cost, the phase or a cell's pitch, so cell accuracy is unaffected by it.

Exposed as `sona transcribe … --grid 16 --salience --spectral-cue`. The second flag needs `--features spectral`, and the CLI errors without it rather than silently falling back to the old cue.

## Evidence (dev, quantized, all notes, `grid=est`; measured on the merged code)

| | lead stem: COnP / COnPOff / cells | vocal stem: COnP / COnPOff / cells |
|---|---|---|
| grid mode (ADR-0015) | 0.518 / 0.285 / 76.0% | 0.474 / 0.256 / 67.0% |
| + salience | 0.555 / 0.308 / 80.7% | 0.570 / 0.325 / 79.5% |
| + spectral cue | 0.558 / 0.337 / 76.0% | 0.508 / 0.306 / 67.0% |
| **+ both** | **0.596 / 0.369 / 80.7%** | **0.611 / 0.388 / 79.5%** |

The two tracks touch different parts of the decode (cell pitch versus held-note splits), and their gains add. With salience, a plain demucs vocal stem nearly matches a separated lead stem on cell accuracy: 79.5% against 80.7%.

## Overfitting

- **Salience:** 7 knobs were tuned on 9 songs, on a broad plateau. Song-bootstrap intervals on the gain exclude zero on both stems.
- **Spectral cue:** its weights were cross-validated per song. The shipped weights are refit on all 9.
- **Regressions:** fukagyakuteki regresses with salience on vocals (0.50 → 0.45), and two songs lose ≤ 0.03 with the spectral cue.
- **No out-of-sample confirmation yet.** That is why both stay opt-in.

## Consequences

- **Cost:** salience costs about one pYIN pass (≈ 22 CPU-seconds per 4-minute song).
- **Default output is unchanged.** `GridDecoder::default()` and the note-HMM path are byte-identical; both features are `None` / `VoicingEnergy` by default.
- **Model check still applies.** The TLA+ model (`verification/grid-notes`) covers `notes_from_path` and `fill_gaps`, which this change leaves untouched.
- **Next:** a fresh holdout. If it confirms, make both the grid-mode default.
