# In-domain gate: sona vs. UST on its target material (UST corpus gate, ticket 06)

> **Follow-up:** the recall gap found here is addressed by the opt-in grid-conditioned cell decoder. See [`grid-cell-decoder.md`](grid-cell-decoder.md) and ADR-0015.

Run: `cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --vocadito --include-holdout --reason "…" --reread` (about 2 min; songs are transcribed in parallel). Since the holdout ledger was added, the gate scores dev only unless asked.

Corpus: 19 of the manifest's 20 songs. 18 are `vocaloid-original` (**the gate**) and 1 is `human-sung` (watched only). plus-space was excluded in 05 because its alignment was flagged. The baseline is Vocadito (40 stems), recomputed with the same code.

Pipeline: default `Transcriber` (pYIN + note-HMM) on the demucs `htdemucs` vocal stem. *Quantized* means the review surface's hard-snap (`quantize.rs`, ported from `grid.ts`), applied on each song's own grid (16th, or 16th-triplet for the 3 triplet songs) and anchored at the song's UST-derived offset.

**The decision rule was pre-registered before any result.** It's in `issues/06-in-domain-gate.md` and was committed in `e56ad73`, before the gate first ran on the full corpus.

## Verdict: a real offset gap remains, and it is not the biggest in-domain gap

| | value | 95% interval (song bootstrap) | n |
|---|---|---|---|
| **The offset number**: offset accuracy given a correct onset + pitch, rest-terminated notes, monophonic regions, quantized | **0.571** | [0.476, 0.678] | 184 pairs |
| same, raw (before quantizing) | 0.656 | [0.566, 0.739] | 180 |
| Vocadito, same measure, raw | 0.86 | — | 484 |

Under the pre-registered rule, **a real gap remains**: the number is below 0.75 and its interval's upper end is below 0.85. The dev and holdout halves agree (57% and 56%).

The ticket expected quantization to absorb much of the offset gap. **The data shows the opposite, plus two surprises:**

1. **Quantization makes offsets worse,** from 0.656 raw to 0.571 quantized. After snapping, rest-terminated offsets are *exact* 48%, *off by 1 cell* 36%, and *off by ≥ 2 cells* 16%: bimodal, as the ticket predicted, but with too much weight on the wrong cell. For 147 of the 184 pairs the COnPOff tolerance (`max(20% of duration, 50 ms)`) is *less than one cell*, so a one-cell snap error always fails.
   - The adversarial review confirmed this is structural, not a bug. 9013 of 9082 reference onsets sit exactly on the anchored grid. No pair with a raw error under half a cell fails after snapping. Sliding the grid makes it worse.
2. **sona's offsets are *better* on Vocadito (86%) than in-domain (66%)** on the same conditional measure. That's the reverse of CONTEXT.md's expectation that offsets are easier on staccato Vocaloid. It holds within note-duration bands (in-domain vs Vocadito: < 0.25 s 0.66 vs 0.87; 0.25–0.5 s 0.60 vs 0.84; 0.5–1 s 0.75 vs 0.90) and with a fixed 50 ms tolerance (0.58 vs 0.81).
3. **Offset misses go both ways.** Raw offset error is median −23 ms (p10 −102, p90 +97). 24% of notes end more than 50 ms early and 18% end more than 50 ms late. A mechanism that only tightens late note ends can address at most the 18% late tail.

### The bigger in-domain gap is note recall

| view (vocaloid-original, pooled) | COnP F1 | COnPOff F1 | COnP P | COnP R |
|---|---|---|---|---|
| raw, all notes | 0.338 | 0.157 | 0.528 | 0.249 |
| quantized, all notes | 0.341 | 0.144 | 0.531 | 0.251 |
| raw, monophonic | 0.476 | 0.224 | 0.602 | 0.393 |
| quantized, monophonic | 0.468 | 0.192 | 0.593 | 0.386 |
| **Vocadito (raw)** | **0.639** | **0.502** | | |

(The mean of per-song F1, which is how the Vocadito baseline was first recorded, gives 0.33 and 0.42 for raw COnP on all notes and monophonic, against Vocadito's 0.646.)

sona finds only **25–39% of the reference notes** in-domain, while 53–60% of the notes it emits are correct. Where the recall goes:
- **Harmony and doubled voices in the stem.** demucs can't separate lead from harmony. Restricting to monophonic regions lifts recall from 0.25 to 0.39.
- **The 100 ms prune floor.** 14.9% of reference notes are shorter than sona can emit, which caps recall at 85.1%. It's concentrated: 68% of 甘天's notes, 52% of BassSousha's, 50% of ノイズゴト's, 32% of アタマロ's.
- **Same-pitch re-articulations,** from the adversarial review. 22% of monophonic reference notes repeat the previous note's pitch (a syllable change on a held pitch), and sona recovers only 26% of them. Excluding those and the sub-100 ms notes, recall is 0.48 against precision 0.60.
- **Separation failures.** These two songs score near zero for reasons outside sona. Excluding them doesn't change the verdict (0.566 [0.466, 0.674]).
  - **BassSousha:** its original is bass and drums only, so sona tracks bass that bleeds into the vocal stem, at −28 and −31 semitones.
  - **盲目シャボン:** its whole lead is doubled an octave down, and 29% of matched pitches come out at −12.

## What else the number could be measuring (pstack `principle-explain-the-number`)

- **Misalignment.** The offset was set without sona (05) and verified with sona's onset detector: medians between −32 and +3 ms, no drift. Two caveats:
  - The adversarial review found that sona's **note onsets** (as opposed to its detector onsets) land +23 to +32 ms late in 金星 and Telescope, with two-peaked distributions. Shifting those references 35–45 ms later recovers 35–44% more COnP matches.
  - The drum grid and the detector onsets both independently place the references where they are, and Vocadito shows no such lag (its best shift is 0 ms). So this reads as sona's note-onset placement lagging in some songs (pitch-born onsets settling after the attack), not misalignment. It can't be fully excluded.
  - Either way the offset verdict holds: with every song's reference moved to its best-COnP position, the offset number is 0.508 [0.412, 0.637].
- **Harmony bleed.** That's what the monophonic view controls for. The offset number is computed only in monophonic regions.
- **The quantizer anchor is optimistic.** The gate anchors on the true UST-derived offset. The review surface instead auto-anchors on the first detected onset, so quantized numbers here are an upper bound for a user who doesn't fix the grid.
- **The escape hatch.** Disabling it leaves the offset number unchanged (0.571).
- **Errors in the UST itself.** It's a fan transcription, not the producer's MIDI. Pitch agrees well: 76–93% of onset-matched notes have exactly the UST pitch, excluding the two separation failures. UST offset timing can't be checked independently, and that's the main unbounded caveat.
- **The rest-terminated definition.** Alternatives (a gap of 50 ms, or 100 ms) give 0.571 and 0.575.

## Per song

```
song                             grid  notes  mono    ALL: raw → quantized      MONO: raw → quantized     <100   ceil
                                                     COnP  COnPOff  off|rt     COnP  COnPOff  off|rt
innocent-addict                  16th    679   42%   0.42→0.35 0.24→0.16 76%→81% 0.59→0.49 0.37→0.25 86%→90%    8%    92%
kinsei                           16th    502   66%   0.42→0.46 0.20→0.22 53%→56% 0.39→0.44 0.15→0.18 50%→56%    0%   100%
kamaitachi-ni-yoru               16th    439   38%   0.37→0.37 0.15→0.14 60%→62% 0.59→0.60 0.23→0.21 61%→64%    0%   100%
noizugoto                        trip    342   51%   0.35→0.39 0.15→0.25 70%→75% 0.45→0.50 0.17→0.30 67%→71%   50%    50%
telescope-nevermore              16th    520   36%   0.26→0.32 0.09→0.13 38%→47% 0.36→0.45 0.15→0.23 50%→43%    0%   100%
bass-sousha-e-no-henjin-ronsou   16th    312    2%   0.03→0.03 0.02→0.02 —→—     0.10→0.10 0.10→0.10 —→—       52%    48%
light-seeker                     16th    254   13%   0.23→0.23 0.12→0.11 60%→40% 0.26→0.26 0.18→0.16 100%→50%    4%    96%  (human-sung)
atamaro-deluxe                   trip    793    5%   0.32→0.35 0.14→0.19 68%→68% 0.20→0.31 0.10→0.20 —→—       32%    68%
smolder                          16th    256   79%   0.50→0.48 0.22→0.18 75%→67% 0.52→0.50 0.22→0.18 75%→67%    0%   100%
azemichi-ni-ochita-ginga         16th    515   49%   0.40→0.39 0.19→0.12 70%→48% 0.67→0.65 0.33→0.21 72%→52%    1%    99%
dm                               16th    399   19%   0.40→0.48 0.23→0.28 50%→38% 0.55→0.66 0.40→0.48 62%→55%    0%   100%
yamineko                         16th    597   45%   0.30→0.27 0.12→0.07 63%→47% 0.48→0.45 0.20→0.12 61%→47%    0%   100%
sandatsu                         16th    569   44%   0.21→0.20 0.08→0.06 12%→12% 0.32→0.30 0.13→0.09 12%→12%   14%    86%
my-axiom                         16th    599   14%   0.39→0.37 0.23→0.18 69%→38% 0.54→0.53 0.26→0.20 67%→50%   12%    88%
emerald                          16th    611   12%   0.41→0.41 0.19→0.18 84%→73% 0.57→0.56 0.31→0.28 100%→100%    0%   100%
amaten                           16th    694   11%   0.26→0.26 0.08→0.06 100%→80% 0.23→0.23 0.11→0.08 100%→0%   68%    32%
moumoku-shabon                   trip    504   10%   0.05→0.03 0.02→0.01 80%→75% 0.17→0.14 0.08→0.08 100%→100%    0%   100%
fukagyakuteki-henyou             16th    500   85%   0.48→0.40 0.24→0.14 65%→52% 0.48→0.38 0.23→0.12 62%→46%   14%    86%
invariant                        16th    251   10%   0.30→0.37 0.13→0.13 50%→45% 0.35→0.48 0.06→0.13 33%→25%    2%    98%
```

`off|rt` is the offset accuracy on rest-terminated COnP pairs; per-song values rest on small n (5 songs supply 120 of the 184 monophonic pairs). `<100` is the share of notes below the prune floor, and `ceil` is the resulting recall ceiling.

The **human-sung** stratum is one song (Light Seeker): COnP 0.23, and 41% of its matched pitches sit 2 semitones low, which suggests a key change or a different version from the UST. It's watched only, and n = 1 supports no conclusion.

## Decision and next step

**Don't close the offset item.** In-domain, sona's offsets are worse than on Vocadito, and quantization doesn't hide that.

**I don't recommend the next move the ticket pre-wrote (re-sweeping the ADR-0012 Release path in-domain), at least not as the next move.** Three findings undercut it:
1. Release tightens only *late* ends, and late misses are 18% of rest-terminated notes, against 24% early.
2. Offsets are a second-order problem in-domain next to recall (0.25–0.39).
3. About a fifth of the quantized offset loss comes from the snap itself (0.656 → 0.571 of a 0.429 total shortfall), which is a review-surface policy, not perception.

Ranked by measured size, the next candidates are:
1. **A tempo-aware prune floor.** 14.9% of in-domain notes, up to 68% of a song, are structurally unreachable at 100 ms. Measurable on the dev split today.
2. **Recall on same-pitch re-articulations and dense legato.** 22% of monophonic notes, recovered at 26%.
3. **The offset snap policy** in `grid.ts` (e.g. snap an offset only when its raw error is within a fraction of a cell, or snap only onsets). This accounts for about a fifth of the quantized offset loss and is cheap to test with the Rust port.
4. Only then, an offset mechanism. Whichever one is tried, a new ADR has to argue the overfitting tension explicitly, as ADR-0012 requires.

Iterate on the **dev** split (9 scored songs). Read the **holdout** split (9 songs) once, at the end. An ML offset model stays off the table at this corpus size (9,082 reference notes).
