# GAME note extraction: experimental opt-in, with an octave rule and a learned cell stage

**Status: Accepted as experimental.** holdout3 (10 fresh songs, rule pre-registered at `1bac372` in `.scratch/game-holdout3/spec.md`, read once and logged):
- GAME + the octave rule raises cell accuracy 72.5% → 76.3%. That is +3.8 with 95% interval [−0.6, +7.5], short of the pre-registered +5 bar, so GAME stays opt-in and experimental.
- The octave rule (+0.3, interval [+0.1, +0.6]) and the learned cell stage (+2.2, interval [+0.8, +3.5]) both passed their own bars.

Later: GAME is seeded and deterministic ([ADR-0023](0023-deterministic-game.md)); the learned stage is ported to Rust ([ADR-0026](0026-learned-cell-stage-in-rust.md)) and takes its grid phase from the vocals' onsets ([ADR-0027](0027-grid-phase-from-onsets.md)).

## Context

After ADR-0016, grid mode reached 79.5% dev cell accuracy. The Vocal2Midi project ([Xiantaidu/Vocal2Midi](https://github.com/Xiantaidu/Vocal2Midi)) builds on openvpi's **GAME**, a singing-to-notes model trained on Japanese and other singing with accompaniment augmentation. A 3-worker read of that repo found that GAME is the only stage that produces notes. Every other Vocal2Midi stage was reproduced on dev and none helped: language-neutral id, the silence slicer, the metrical quantizer, the lyric/forced-alignment path.

## Decision

1. **`sona-core` feature `game`** (`sona_core::game`).
   - **Runtime and weights:** GAME-1.0.3-medium ONNX through `ort` with `load-dynamic` (set `ORT_DYLIB_PATH`). The weights are user-downloaded and never bundled, because they are CC BY-NC-SA 4.0.
   - **Chunking:** chunks of about 20 s cut at the quietest point.
   - **Model settings:** 8 D3PM steps, boundary threshold 0.2 and radius 2, language ja; **presence threshold 0.8** (GAME's default is 0.2; 0.8 was best on dev).
   - **Snapping:** `snap_to_grid` removes the tuning offset, estimates the phase from GAME's onsets, snaps to the nearest line, and keeps the line monophonic.
2. **Octave rule** (`correct_octaves`, `OctaveRule::default()`). GAME sometimes tracks high voices an octave low; in 3 of 28 songs about 60% of its notes sat 12 semitones under the UST.
   - **Signal:** each note's log ratio of odd- to even-harmonic energy.
   - **Rule:** if the song's median ratio is below −0.25, raise notes below 0; elsewhere, raise only notes below −1.5.
   - **28 songs:** 79.5% → 83.7%.
   - **holdout3:** +0.3, no song worse, one song +2.2.
3. **Learned cell stage (prototype, Python; ported in ADR-0026).**
   - **Model:** a BiGRU over grid cells picks rest, or a pitch from GAME-lead, GAME-vocals, sona, or GAME-lead ±12.
   - **Features:** GAME encoder embeddings plus each system's note context.
   - **Training data:** dev plus the two spent holdouts (28 songs, logged in the ledger as training use).
   - **Results:** 28-song CV +1.2; holdout3 +2.2.
   - **Licence:** its weights are derived from Pifuyuu's USTs, whose terms forbid redistribution, and from GAME (NC-SA). They stay local and are not committed.
4. GAME is not the default. Making it so needs a fresh confirmation that clears the bar, plus a decision on shipping non-commercial weights.

## Evidence

| cells, quantized, all notes | dev (9, Rust harness) | 28 songs (prototype, CV for D) | holdout3 (10, read once) |
|---|---|---|---|
| A: grid defaults, vocal stem | 79.5% | — | 72.5% |
| B: GAME-lead | 87.4% | 79.5% | 76.0% |
| C: B + octave rule | 87.4% | 83.7% | 76.3% |
| D: C + learned cell stage | — | 84.9% | **78.8%** |

- On holdout3, D − A is +6.3 [+2.6, +9.5], with 9 of 10 songs better. This comparison was not pre-registered.
- GAME's note F1 is far higher than sona's: COnP 0.663 vs 0.495 on holdout3.

**The song that fails the bar.** ironication scores 51% with A and 41% with C. Most of its sung cells sit exactly an octave under the UST in *both* GAME and sona. GAME also rests through 769 sung cells, and the octave rule didn't trigger at the song level.

## Rejected on this data (numbers in `crates/sona-eval/reports/grid-cell-decoder.md`)

- GAME large: worse at every presence threshold.
- RMVPE as sona's f0 or candidate source.
- Vocal2Midi's slicer, its language-neutral id, and its metrical quantizer.
- Gap filling and boundary shifts.
- D3PM 4 steps: +0.3 on 28 songs, within noise.
- TiFA forced alignment of published lyrics:
  - The ceiling with perfect UST onsets plus pauses is 90.8% on lead stems, but real alignment captured none of it.
  - As hard boundaries it scored 84.8–85.5% against 85.6%.
  - As soft features it scored 84.50% against 84.91%.

## Consequences

- **holdout3 is spent.** A further confirmation needs holdout4. *(2026-11-24: not from BowlRoll, whose terms forbid mechanical collection; from older songs in Pifuyuu's OneDrive share, see `.scratch/utau-edit-cost/issues/08-holdout4.md`.)*
- **90% is not reached.** holdout3's best is 78.8%. The ceilings: perfect sung-vs-rest decisions give 91.4%; choosing the best available pitch per cell gives 95.6%.
- **Fine-tuning GAME on the labelled songs was tried (2026-11-22) and is not adopted.**
  - **Setup:** trained on the 30 spent-holdout songs, scored on dev over 5 fixed segmenter seeds.
  - **Cells:** +0.80 (95% CI −0.53 to +1.91), against a pre-registered +2.0 bar.
  - **Report:** [`game-finetune.md`](../../crates/sona-eval/reports/game-finetune.md).
- **Run-to-run noise:** GAME's D3PM segmenter samples boundaries at random, so this ADR's unseeded GAME runs carry about ±0.2–0.4 points of run-to-run noise. Seeded since ADR-0023.
- **Port:** done in ADR-0026 (holdout3 parity: 78.83% on the true phase, 78.18% on sona's estimated phase).
