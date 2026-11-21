# Grid-conditioned cell decoder vs the note-HMM on the UST corpus (ADR-0015)

Runs:
- `cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --grid --include-holdout --reason "…" --reread` for the gate, all songs (about 2 min). Without `--include-holdout` it scores dev only; holdout reads are logged in `corpus/holdout-reads.jsonl`.
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

## How far from "85–100% of the UST"? Ceilings on dev

`sona-eval-ust-dev` now also prints **cell accuracy**: at each grid cell, does the exported line have the UST's pitch, or a rest where the UST rests?

| dev, quantized | note-HMM | grid mode |
|---|---|---|
| cell accuracy, all cells | 53.3% | **67.0%** |
| cell accuracy, monophonic regions | 78.6% | **86.7%** |
| cell accuracy, sung cells | 38.7% | 59.9% |
| cell accuracy, sung cells, monophonic | 61.4% | **82.5%** |

Where only the lead sings, grid mode is already at about 85% per cell. Oracles show what these stems allow on monophonic note COnP F1:

| oracle (monophonic COnP F1) | score |
|---|---|
| grid mode today | 0.589 |
| UST boundaries, pitch from sona's contour | **0.909** |
| UST notes with same-pitch repeats merged (no re-articulation detection) | 0.839 |
| … and pitch from the contour | 0.775 |

The pitch evidence is there (0.91 with perfect boundaries). Two things stand between today and 85%+:

1. **Polyphony in the stem.** Two thirds of the UST's notes sit where other voices also sing. There, only 49% of lead cells come out right: 28% decode as a rest because pYIN loses voicing under two voices, 18% take a harmony pitch, and 5% are octave errors. This is an input problem. `scripts/separate-lead.sh` makes a lead-only stem with a UVR karaoke model, and `--stem lead` scores it. It hasn't been run yet; it is maintainer-run.
2. **Same-pitch re-articulation.** 22% of monophonic UST notes repeat the previous pitch (a new syllable on a held pitch). Without detecting them, note F1 is capped near 0.78. No acoustic cue tried separates them well (AUC 0.69–0.73). The UST places one note per syllable (mora), so **the lyrics** would solve this directly: the number of morae in a phrase is its number of notes.

Tried on dev and rejected (the gains were within the song-bootstrap noise or negative):
- A note-duration (semi-Markov) prior learned from the dev USTs: COnP 0.464 against 0.465.
- A lower pYIN voicing threshold (0.3): +0.01 on all notes, −0.01 monophonic.
- Re-tuning the rest cost around that threshold.

## Lyrics: what the dev experiments showed (2026-11-20)

Pifuyuu's lyrics are CV kana, one mora per note, with `R` for rests. All 5,336 dev notes line up with a lyric. Each attempt below is a Python prototype on the dev split, scored against grid mode's output with the gate's matcher. (Those grid-mode baselines are slightly below the table above because the prototype keeps the raw decoder phase.)

| lyric information used | method | all COnP | mono COnP |
|---|---|---|---|
| (none, grid mode) | | 0.458 | 0.589 |
| mora count per phrase (oracle phrase spans) | greedy split/merge at the strongest/weakest cues | 0.467 | 0.577 |
| mora count per phrase (oracle phrase spans) | count-constrained Viterbi re-decode | 0.473 | 0.562 |
| mora vowel sequence | global forced alignment, vowel templates self-learned per song | 0.462 (best) | 0.569 (best) |

**Counts don't help, because placement is the problem.** In monophonic phrases where grid mode already emits exactly the right number of notes, only 71% of them match. Knowing *how many* notes there are doesn't say *where* the boundaries go.

**Vowels are identifiable, but global alignment drifts.** A per-song nearest-centroid classifier on the cepstrum at a note's middle identifies its vowel 80% of the time (chance is 20–30%). Of the misplaced same-pitch boundaries, 53 of 60 change vowel. But aligning about 600 morae to a whole song at once drifts: one slip, often on a bleed note, shifts every later mora, so the self-learned templates train on misaligned cells. Raising the vowel weight makes it worse (0.36 all notes).

The vowel evidence is real. What it needs is **anchoring**: lyric lines matched to sung phrases, re-anchored at every rest, before vowels can place boundaries. That is the next lyrics step, if it is pursued. It also needs no vowel model trained on the UST.

## Swarm results (2026-11-21): four hypotheses in parallel from `c1d81b5`

Each worker ran on its own branch and in its own worktree, scored with the same `sona-eval-ust-dev` command on the dev split only (the ledger blocks holdout reads). The selection rule: beat the lead-stem baseline (0.518 / 0.285 / 76.0% cells) by more than 0.01 without hurting Vocadito. The user chose **cell accuracy** as the target.

| worker | idea | lead COnP / COnPOff / cells | verdict |
|---|---|---|---|
| W1 | re-tune `sigma` and `voiced_in_rest` for lead stems | 0.572 / 0.319 / 75.3% (sung 71.2 → 67.8%) | **not merged**: note F1 rises but cell accuracy falls (16% fewer notes) |
| W2 | lyrics anchored per phrase | see below | |
| W3 | harmonic-salience pitch evidence | 0.555 / 0.308 / **80.7%** | **merged** (ADR-0016) |
| W4 | spectral re-articulation cue (leave-one-song-out) | 0.553 / 0.333 / 76.0% | **merged** (ADR-0016) |
| W3 + W4 (my measurement on the merge) | | **0.596 / 0.369 / 80.7%** | vocals: 0.611 / 0.388 / 79.5% |

What each worker learned:
- **W1:** most of its single-knob wins came from flipping one song's grid phase (telescope-nevermore, off by half a cell). Two knobs survived checks at both the UST and the estimated phase.
- **W3:** its key oracle result is that the top salience candidate is the lead in 63% of polyphonic frames, against 32% for pYIN. Following weaker candidates hurt.
- **W4:** voicing and energy add nothing once spectral flux and cepstral change are in the model, and metrical position is uninformative (AUC 0.50).
- **Telescope-nevermore's lead-stem phase** is still half a cell off under every configuration except W1's. Robust phase estimation is open work.

## Fresh holdout (holdout2) confirms ADR-0016 (2026-11-21)

10 songs nothing was tuned on (the next uploads in channel order; 2 alignment failures were backfilled before the read), read once against the rule in `.scratch/grid-evidence-holdout2/spec.md`:

| holdout2, vocal stems, quantized, all notes | plain grid mode | + salience + spectral cue |
|---|---|---|
| cell accuracy, all / sung / mono | 52.8% / 36.1% / 72.3% | **70.0% / 62.2% / 77.8%** |
| COnP F1 / COnPOff F1 | 0.330 / 0.206 | **0.514 / 0.335** |

- **Cell gain:** +17.2 points, 95% [+12.0, +22.3], and every song improves.
- **Lead stems:** 69.5%, so they add nothing once salience is on.
- **Both features are now grid mode's defaults.**
- **Absolute level:** below dev (70% against 80%). holdout2 is more polyphonic, and cinema-mitaina (38%) needs a data check.

## Toward 90%: where the remaining 20.5% of wrong cells are (dev, vocal stems, defaults)

| error | share of all cells | monophonic / polyphonic | at a note's first or last cell |
|---|---|---|---|
| UST sings, sona rests | 8.3% | 399 / 1506 cells | 71% |
| UST rests, sona sings | 4.2% | 527 / 426 | (n/a) |
| wrong pitch ±1 | 3.2% | 267 / 460 | 69% |
| wrong pitch ±2 | 2.0% | 149 / 299 | 78% |
| wrong pitch, other | 1.8% | 65 / 344 | 65% |
| wrong pitch ±12 | 1.3% | 9 / 294 | 60% |

- **Boundaries dominate.** 56% of wrong cells are a note's first or last cell: a boundary one cell off, not a wrong note.
- **Onsets** are 78% exact, 9% one cell late and 4% one cell early. On amaten, 37% are one cell late.
- **Rest-terminated offsets** are only 48% exact, and miss in both directions.
- **Phase is not the cause.** Every dev song's estimated grid phase is within ¼ cell of the UST's.
- **The mechanism.** In the late onsets, the new note's first cell is unvoiced in pYIN (its consonant), so nothing argues for the change there, and the decoder extends the previous note.
- **azemichi** accounts for most of "UST rests, sona sings" (15% of its cells), probably singing the UST doesn't transcribe.

Tried on dev and rejected (default-off knobs, removed again):

| idea | best result (cells, all) |
|---|---|
| core-weighted emission (head/tail frames of a cell down-weighted) | 79.1% (from 79.5%): worse at every setting |
| onset-cue bonus on note starts, from the spectral cue's log-odds | all starts: sung cells +1.4 but spurious notes, all cells 79.7%; pitch changes only: 79.6% (+0.1) |

The consonant evidence the spectral cue sees at those lines isn't distinctive enough to place them. The next levers are a stronger front end: a neural f0, or note-onset models from the singing-transcription literature (a research swarm is surveying them).

## Where Pifuyuu's note starts sit (premise test, dev, monophonic pitch changes)

The two rejected boundary fixes both assumed that late onsets come from weak consonant evidence. Measured directly:

| first sound of the new syllable | new pitch first voiced, relative to the UST start | old pitch last voiced |
|---|---|---|
| unvoiced consonant, n=280 | median +0.52 cell (p10 +0.15, p90 +1.17) | median −0.68 cell |
| voiced consonant, n=504 | median +0.30 | −0.29 |
| vowel, n=161 | median +0.31 | −0.12 |

The UST start sits inside the unvoiced gap, about 57% of the way from the old note's end to the new vowel. That is neither the consonant onset nor the vowel onset that UTAU, VOCALOID and DiffSinger use. Placing pitch-change boundaries at the grid line nearest that point *lowered* cell accuracy (79.2% → 78.8–78.9%). The decoder's placement already beats a fixed rule, so the remaining late onsets need better evidence, not a placement rule.

## Web research swarm (2026-11-21): external methods, ranked for sona

Four sourced reports (state-of-the-art transcription, open-source tools, robust pitch, syllable alignment). The main points:

| candidate | why | licence / packaging | targets |
|---|---|---|---|
| **RMVPE** f0 (Interspeech 2023, [arXiv 2306.15412](https://arxiv.org/abs/2306.15412)) | Only released f0 model trained to pull the vocal out of a mix. Polyphonic MIR-1K raw pitch accuracy 95.4 against pYIN's 77.3. Outputs 360-bin salience, so it can also replace the candidates | code Apache-2.0, weights MIT (stated by the Hugging Face page), ONNX ([Scicom-intl/rmvpe-onnx](https://huggingface.co/Scicom-intl/rmvpe-onnx)), about 186 MB, about 18× real time on 1 core | sung-as-rest under harmony, octave errors |
| **GAME** (openvpi; successor to SOME, [repo](https://github.com/openvpi/GAME)) | Singing-to-notes, trained with Japanese and with accompaniment/noise augmentation. Gives boundary probabilities, presence and pitch, and can estimate on given boundaries. OpenUtau runs it through ONNX Runtime | code MIT; **weights CC BY-NC-SA 4.0**; official ONNX; small model about 12.7M parameters | onsets, sung-as-rest, spurious notes |
| Yong et al., phoneme-informed AST ([arXiv 2304.05917](https://arxiv.org/abs/2304.05917)) | Phoneme posteriors are what catch same-pitch re-onsets | MIT, 106 MB checkpoint; ONNX untested; English phoneme set | re-articulations |
| Tatum-level CRNN-HSMM (Nishikimi 2021) | Same formulation as sona's grid Viterbi. A metrical onset prior plus a long-note penalty mainly fixes onset and offset errors | paper only (pure-Rust idea) | onsets, offsets |
| basic-pitch (Spotify) | onset and frame posteriors as extra features | Apache-2.0, official ONNX, under 17K parameters | onsets (untested on singing) |

Two more findings:
- Published systems reach COnP F1 about 0.77 on pop vocals (MIR-ST500, [T3MS](https://arxiv.org/html/2502.12438v1)). sona's 0.51–0.61 trails mainly on boundaries.
- sona's `crepe` feature is an empty placeholder, so any neural front end means building the `ort` path.

## Next

0. ~~Confirm ADR-0016 on a fresh holdout~~: done (holdout2). Toward 90% cell accuracy, in order:
   - RMVPE as an opt-in `ort` f0 and candidate source (permissive licence; targets the largest polyphonic classes);
   - GAME's boundary and presence probabilities as decoder evidence (needs a licence decision: user-downloaded, non-commercial weights);
   - Nishikimi's metrical onset prior (pure Rust).

   Older items still open:
   - break down the remaining wrong cells on dev;
   - fix the two known limitations (salience lead-in after silence; spectral cue on clean tones);
   - phase robustness (telescope-nevermore).

1. Run `scripts/separate-lead.sh`, then `sona-eval-ust-dev --stem lead --cfg grid:grid=est` to measure the polyphony fix.
2. Lyrics-aware decoding: an optional mora sequence per phrase fixes the note count, which removes the re-articulation cap. This is a new input, so it needs a product decision and an ADR.
3. BPM and division estimation, so grid mode needs only the stem. The corpus already has golden labels for both.
4. A fresh holdout: the current one is spent. Newer or older Pifuyuu uploads, fetched with the maintainer-run scripts.
5. ~~Fix the review surface's escape-hatch overlap~~. Done: `resolveOverlaps` (`web/lib/grid.ts`), model-checked in `verification/grid-notes`.
