# Note-level onset gate (ADR-0013 / ticket 04)

Run: `cargo run --release -p sona-eval --features spectral --bin sona-eval-note-onset-gate -- --corpus`
Corpus: Vocadito, 40 stems + staccato fixture. Full pipeline (pYIN → onsets → note-HMM → confidence) with each onset source; scores **note COnP / COnPOff** — the metric that decides promotion (onset-F1 was only a proxy).

## Result

| onset source | corpus COnP-F1 | corpus COnPOff-F1 | staccato COnP | staccato COnPOff |
|---|---|---|---|---|
| **energy (default)** | **0.646** | **0.508** | 1.000 | 1.000 |
| superflux δ=0.02 (high recall) | 0.519 | 0.326 | 1.000 | 1.000 |
| superflux δ=0.10 (conservative) | 0.619 | 0.477 | 1.000 | 1.000 |
| ensemble energy⊕superflux | 0.520 | 0.326 | 1.000 | 1.000 |

## Verdict: energy wins decisively at the note level — SuperFlux is not promoted

The onset-F1 proxy said energy won narrowly; **the note-level metric says it wins decisively.** Every SuperFlux and ensemble variant scores *lower* COnP than energy — more onsets make notes *worse*. The hypothesis that the HMM would absorb SuperFlux's false onsets is **falsified**: the extra onsets over-split into spurious notes that cost COnP more than the recovered boundaries gain. The ensemble inherits the aggressive SuperFlux's false onsets and is dragged down with it.

## The insight: onset recall was a proxy-metric red herring

Energy's onset *recall* is 0.44, yet its note *COnP* is 0.65 — **COnP exceeds onset recall.** That is only possible because the **note-HMM already recovers most note boundaries from pitch** (the interval-prior legato split, ADR-0010), not from onsets. The onset stage and the HMM's pitch-based splitting are therefore **partly redundant**: a "missed" onset on a pitch change is still segmented by the HMM. The onset stage's real, non-redundant job is the **same-pitch re-articulation split** (two notes at one pitch, which pitch alone can't separate) — and energy does that with high precision.

So the onset "recall gap" that motivated this whole line **does not bottleneck the product.** Improving onset recall chases a metric (onset-F1) the note-HMM makes moot. Adding onsets past energy's high-precision set only adds false splits.

## Consequence

- **Energy-rise stays the default onset detector**, now confirmed by the metric that matters, not just inherited.
- **Spectral flux + SuperFlux stay feature-gated and off** — a proven, measured capability that does not beat energy end-to-end. Kept behind the feature (isolated, no default-build cost) in case a future domain — extreme legato where pitch-splitting *also* struggles — needs it.
- **Onsets are not the lever.** The remaining measured bottleneck is **offset precision** (COnPOff 0.508), and the deepest open problem is still the **target-domain (UVR) eval gap**. If onset perception is ever revisited, the next lever is an ML onset model (the `crepe`/basic-pitch seam), not more DSP — but the note-level evidence says the payoff would be small.
