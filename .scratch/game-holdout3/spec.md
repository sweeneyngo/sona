# Spec: confirm the GAME pipeline on the fresh holdout (holdout3)

Status: **done: holdout3 read once at 1bac372; rule 1 fails, rules 2 and 3 pass (Result below).**

## Question

Do the pieces built on 2026-11-21 hold on 10 songs that nothing was tuned or trained on?
The pieces are: GAME note extraction (`sona-core` `game`), its octave rule, and the learned
cell stage (± TiFA lyric features). The evidence so far is dev plus the spent holdout/holdout2
songs, used as training data (ledger entries 2026-11-21).

## Candidates (all frozen at the commit that adds this rule)

| id | pipeline | scored by |
|---|---|---|
| A | sona grid defaults (ADR-0016), vocal stems: today's shipped default | Rust harness |
| A′ | A on lead stems | Rust harness |
| B | GAME-1.0.3-medium, lead stems, presence 0.8, phase from onsets, nearest-line snap (`game=0.8`) | Rust harness |
| C | B + subharmonic octave rule (`game=0.8,oct=1`) | Rust harness |
| C\* | C as reproduced by the Python prototype | prototype scorer |
| D | C\* + learned cell stage (BiGRU, trained once on all 28 dev + spent-holdout songs; weights sha256 recorded below) | prototype scorer |
| E | D with TiFA lyric features (published lyrics → TiFA alignment → per-cell soft features) — **only if** its 28-song CV beats D by ≥ 0.5 points | prototype scorer |

D and E are compared with C\*, not C, so that each comparison uses one scorer.

## Measurement (one sitting, logged once)

1. One `sona-eval-ust-dev --split holdout3 --reason "…"` run with A, A′, B and C, plus `--dump` (built with `--features game`). This is the single ledgered read. GAME inference on holdout3 audio was done earlier without labels.
2. Then build D/E's holdout3 cells from that dump, and score them with the frozen model(s).

## Primary metric

Cell accuracy, all cells, quantized, pooled over the 10 songs. Note COnP F1 is reported alongside. Song bootstrap (10 000 resamples) gives 95% intervals for the differences.

## Pre-registered decision rule

1. **GAME pipeline.** Document C as sona's recommended opt-in path (`--game`; opt-in because the weights are CC BY-NC-SA) if:
   - C − A ≥ +5 points (dev: +6.7 vs the lead-stem default; 28 songs: much larger);
   - its bootstrap interval excludes 0.

   Otherwise keep GAME experimental and report dev as overfit.
2. **Octave rule.**
   - Keep it on in C if C ≥ B − 0.3 points pooled, and no song loses more than 2 points to it.
   - Report how many songs triggered the song-level shift (expected: 0–3).
   - If it fails, `oct` stays off by default.
3. **Learned stage.**
   - Ship it as an opt-in prototype (weights stay local: they're derived from Pifuyuu's USTs and GAME) if D − C\* ≥ +0.8 points and the interval excludes 0.
   - 28-song CV: +1.2.
   - Otherwise record it as not confirmed.
4. **TiFA features.** These count only if E was frozen as a candidate, and E − D ≥ +0.5 with the interval excluding 0.
5. **90% target.** Report the best confirmed pipeline's pooled cell accuracy against 90%, per song.

## Noise

There are 10 songs, and GAME's run-to-run outputs are deterministic. Song resampling is the noise, so a difference under about 1.5 points is not distinguishable.

## Frozen artefacts (fill before committing)

- GAME model: GAME-1.0.3-medium-onnx (ONNX Runtime 1.23.2, CPU).
- Octave rule: `OctaveRule::default()` (song −0.25, low-song note 0.0, other −1.5).
- Learned stage D: `cellstage_D.pt` (local, not committed), BiGRU hid 96, 15 epochs, seed 0, trained on all 28 songs' `cells_oct` features. sha256 `e3c1b9a5434169e91ec436a70c9f89967f898aa1e0a575f87321c556a61325c9`.
- **E is not a candidate.** Its 28-song CV (same folds/seed as D) gave 84.50% against D's 84.91% (−0.41; 1 song more than a point better, 2 worse), short of the ≥ +0.5 bar. TiFA hard boundaries were also rejected on dev (report). Rules 4 and the E rows do not apply.

## Result (holdout3, 10 songs, read once at 1bac372; both ledger entries logged)

| candidate | cells |
|---|---|
| A sona grid defaults, vocal stems | 72.51% |
| A′ the same on lead stems | 71.46% |
| B GAME-lead | 76.00% |
| C B + octave rule | 76.31% |
| C\* C in the prototype | 76.62% |
| D C\* + learned cell stage | **78.78%** |

1. **GAME pipeline, C − A = +3.80, 95% [−0.60, +7.53]. ✘** Fails both bars (≥ +5, interval excludes 0). 9 songs improve by 2 points or more; ironication loses 10.2. GAME stays experimental (ADR-0017). Dev (+6.7 over the lead-stem default) was optimistic.
2. **Octave rule, C − B = +0.31 [+0.12, +0.62], no song worse. ✔** It stays on inside the GAME path; maru-shite-kudasai gains 2.2.
3. **Learned stage, D − C\* = +2.17 [+0.83, +3.47], none worse than −0.4. ✔** Confirmed as an opt-in prototype; weights stay local.
4. Not applicable (E was not a candidate).
5. **90% target: not reached.** The best is D at 78.78%. Per song, D ranges from 46.5% (ironication) to 97.0% (denpa).

Not pre-registered, so exploratory: D − A = +6.27 [+2.64, +9.45], with 9 of 10 songs better.

After the read: ironication is an outlier for every system. GAME and sona both sit an octave under the UST in most sung cells, and GAME rests through 769 sung cells. holdout3 is now spent.
