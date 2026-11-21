# Grid-conditioned cell decoder vs the note-HMM on the UST corpus (ADR-0015)

Runs:
- `cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --grid` for the gate, all songs (about 2 min).
- `cargo run --release -p sona-eval --bin sona-eval-ust-dev -- --cfg base: --cfg grid:grid=est --per-song --vocadito` for the dev split; contours are cached after the first run.

The pipeline is unchanged up to segmentation: pYIN and the energy envelope on the demucs vocal stem. Grid mode then decodes on each song's grid (16th, or 16th-triplet for the 3 triplet songs) at the UST's integer BPM, with the **phase estimated from the stem**. The UST anchor is never given to the decoder.

The *raw* view is what a user gets with the review surface's default auto-anchor. The *quantized* view re-snaps to the UST-derived anchor, as in the original gate.

## Verdict: a large, holdout-confirmed gain on every headline number

The decision rule was pre-registered in `.scratch/grid-cell-decoder/spec.md` (commit `b5391bb`) before the holdout was read. All four criteria pass:

| holdout (9 songs), quantized, all notes | note-HMM | grid | difference (95% song bootstrap) |
|---|---|---|---|
| COnP F1 (pooled) | 0.340 | **0.455** | **+0.114** [+0.064, +0.167] |
| COnPOff F1 (pooled) | 0.158 | **0.303** | **+0.145** [+0.093, +0.183] |
| songs regressing > 0.03 COnP F1 | | | **0 of 9** |

Dev showed the same size of gain: COnP 0.341 → 0.474, COnPOff 0.133 → 0.256. The fit does not look dev-specific.

### The full gate (18 `vocaloid-original` songs)

| view (pooled) | COnP F1 | COnPOff F1 | COnP P | COnP R |
|---|---|---|---|---|
| note-HMM, quantized, all notes | 0.341 | 0.144 | 0.531 | 0.251 |
| **grid, quantized, all notes** | **0.466** | **0.276** | 0.478 | **0.455** |
| note-HMM, quantized, monophonic | 0.468 | 0.192 | 0.593 | 0.386 |
| **grid, quantized, monophonic** | **0.580** | **0.357** | 0.552 | **0.612** |

Recall nearly doubles (0.25 → 0.46) while precision gives up 0.05.

**Offsets.**
- Given a correct onset and pitch, **legato** offset accuracy rises from 40% to 59%. That is the one-cell consonant gap fill.
- **Rest-terminated** offset accuracy is flat: 57% → 55%, with intervals [0.48, 0.68] and [0.39, 0.73]. The rest-terminated sample also grows from 184 to 270 matched notes.

The original gate's question, whether offsets lag Vocadito's, is unchanged by this work. But COnPOff F1 is no longer limited by recall.

**Vocadito** is unchanged at 0.646 / 0.508, because grid mode is opt-in and the default path is byte-identical. The gate's default-mode output reproduces `ust-in-domain-gate.md` exactly.

**Human-sung** (Light Seeker, watched only): COnP 0.23 → 0.36. With n = 1 this is not evidence.

## Per song (grid mode; compare with `ust-in-domain-gate.md`)

| song | split | note-HMM quant COnP | grid quant COnP | grid quant COnPOff |
|---|---|---|---|---|
| innocent-addict | dev | 0.35 | 0.50 | 0.29 |
| kamaitachi-ni-yoru | dev | 0.37 | 0.49 | 0.29 |
| telescope-nevermore | dev | 0.32 | 0.60 | 0.40 |
| atamaro-deluxe | dev | 0.35 | 0.47 | 0.23 |
| azemichi-ni-ochita-ginga | dev | 0.39 | 0.42 | 0.27 |
| yamineko | dev | 0.27 | 0.38 | 0.16 |
| my-axiom | dev | 0.37 | 0.57 | 0.37 |
| amaten | dev | 0.26 | 0.36 | 0.14 |
| fukagyakuteki-henyou | dev | 0.40 | 0.50 | 0.20 |
| kinsei | holdout | 0.46 | 0.64 | 0.37 |
| noizugoto | holdout | 0.39 | 0.54 | 0.38 |
| bass-sousha-e-no-henjin-ronsou | holdout | 0.03 | 0.08 | 0.05 |
| smolder | holdout | 0.48 | 0.52 | 0.35 |
| dm | holdout | 0.48 | 0.62 | 0.50 |
| sandatsu | holdout | 0.20 | 0.46 | 0.27 |
| emerald | holdout | 0.41 | 0.51 | 0.37 |
| moumoku-shabon | holdout | 0.03 | 0.12 | 0.04 |
| invariant | holdout | 0.37 | 0.40 | 0.28 |

On three songs the quantized COnPOff is below raw: kinsei 0.41 → 0.37, fukagyakuteki-henyou 0.28 → 0.20, invariant 0.29 → 0.28. On those the estimated phase sits a fraction of a cell from the UST anchor, so re-snapping to the UST anchor moves some offsets to the neighbouring line. A user on the default auto-anchor gets the raw numbers.

## What else the number could be measuring (pstack `principle-explain-the-number`)

- **Phase leakage.** The decoder never sees the UST anchor; it estimates phase from its own decode cost. On dev the estimate lies within ±13 ms of the UST anchor on all 9 songs. Decoding at the UST anchor instead scores 0.465 against the estimated phase's 0.474, so the estimate is not borrowing truth.
- **BPM leakage.** BPM comes from the UST, as in the original gate. Every corpus BPM is an integer a user would know. A wrong BPM is the main real-world risk (see ADR-0015's limits).
- **Tuning on the test set.** All tuning used the 9 dev songs. The holdout was read once with frozen parameters (commit `b5391bb`), and its gain matches dev's.
- **Matching artefacts.** Grid notes sit exactly on grid lines, as UST notes do. The matcher is the gate's own mir_eval-style COnP/COnPOff (±50 ms onset, 50 cents, max(20%, 50 ms) offset), unchanged. A Python re-implementation of the matcher reproduced the Rust numbers exactly.
- **Separation failures.** BassSousha and 盲目シャボン stay near zero under both decoders. They depress both columns equally and don't change the verdict.

## Where the remaining errors are (dev, monophonic, `--diag` and the prototype's breakdown)

- **Same-pitch re-articulations** cause both false splits and merges, about 6% of reference notes each way. Voicing dip, energy dip and spectral-envelope change all separate them only weakly (AUC 0.69–0.73).
- **One-cell blips in reference rests** (breaths, bleed, ad-libs) make up most of the remaining false positives.
- **Harmony and doubled voices** cover two-thirds of the reference notes, which sit in polyphonic regions; the monophonic recall of 0.61 against all-notes 0.46 is the gap.

## Next

1. BPM and division estimation, so grid mode needs only the stem. The corpus already has golden labels for both.
2. ~~Fix the review surface's escape-hatch overlap~~ — done: `resolveOverlaps` (`web/lib/grid.ts`), model-checked in `verification/grid-notes`.
3. A stronger same-pitch re-articulation cue (e.g. a phoneme-level onset model), the largest remaining fixable error.
