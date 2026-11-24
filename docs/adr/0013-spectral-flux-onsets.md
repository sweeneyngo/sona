# Spectral-flux onsets as a feature-gated alternative novelty

**Revisits [ADR-0006](0006-energy-based-onset-detection.md)'s "no FFT" stance.** The energy-rise detector and its peak-picker are unchanged and remain the default; this adds spectral flux as a second, opt-in novelty source behind a feature flag.

**Status: Rejected; code removed (audit A07, 2026-11-24).** `OnsetNovelty`, `onset/spectral.rs` and the two eval bins that only compared onset sources (`sona-eval-onset-compare`, `sona-eval-note-onset-gate`) are deleted: nothing outside those bins ever selected spectral flux, so the default output is byte-identical. The reports stay as the record. To revisit, restore them from the commit before A07. The `spectral` feature remains for grid mode's cue (ADR-0016) and tempo (ADR-0022).

**Status before removal: Accepted as a gated, off-by-default capability — energy-rise stays default, now confirmed at the note level.** Spectral flux and SuperFlux are implemented behind the `spectral` feature. The onset-F1 gate (`reports/onset-compare.md`) showed SuperFlux recovers recall (0.44 → 0.63) but falls just short of energy on onset-F1 (0.542 vs 0.565). The **note-level COnP gate** (`reports/note-onset-gate.md`) then settled it decisively: energy COnP-F1 **0.646** beats every SuperFlux/ensemble variant (0.52–0.62) — more onsets make *notes* worse. The insight (below): the note-HMM already recovers most boundaries from pitch, so onset recall was a proxy-metric red herring. Not promoted; energy stays default.

## Context

The onset-deviation diagnostic (`reports/onset-deviation.md`) found the onset-F1 dip (≈0.565) is **recall-limited**: the energy-rise novelty (ADR-0006) detects only ~1109 of 2237 Vocadito onsets. It fires on an energy *rise*, so it is structurally blind to a **legato attack** — a new syllable or slurred note change at similar loudness, where the spectrum changes but energy barely moves. That is a large fraction of real singing, and of the real ripped vocals sona targets — it was never a staccato-only edge case.

ADR-0006 chose energy precisely to avoid an FFT ("an attack *is* an energy rise" for staccato Vocaloid). The recall data shows the cost of that choice on expressive material. The standard signal for a timbral/harmonic attack is **spectral flux** — the frame-to-frame growth of the magnitude spectrum — which requires a short-time Fourier transform.

## Decision

Add **spectral flux** as an alternative onset-novelty source, selected by a new `OnsetDetector::novelty` field (`OnsetNovelty::{EnergyRise, SpectralFlux}`), behind a **`spectral` feature**:

- The novelty *function* is the swappable seam; the **peak-picker is shared** (local-max, adaptive threshold, RMS bleed guard, debounce). Spectral flux is computed on the **same frame grid as the energy envelope**, so the novelty curve aligns index-for-index and the picker — including its bleed guard — works unchanged.
- The flux is **log-magnitude**, mean over bins of the half-wave-rectified rise, keeping it in units comparable to the energy path.
- The FFT is **`realfft`** — pure Rust, no C, no ML runtime. So ADR-0005's "builds anywhere" property holds; only ADR-0006's "no FFT" line is retired, and only when the feature is enabled. The default build pulls in no FFT.
- Feature-gated and off by default, mirroring the `crepe` pattern (ADR-0001): the harness earns the swap before it becomes default.

## Rejected alternatives

- **Replace energy outright.** Rejected: flips the no-FFT default before the harness has spoken, and the gate then showed energy still wins on F1. Gating first was correct.
- **Reuse energy's peak-picker threshold for flux.** Measured and rejected: flux has a high, noisy baseline (vibrato/formant motion), so the energy-tuned `threshold_delta` suppressed real onsets (F1 0.275). The threshold must be swept per novelty; even swept, plain flux tops out at F1 0.515.
- **Replace the whole detector with an ML onset model.** Heavier (ONNX), and premature — a pure-DSP spectral novelty is the cheaper lever to exhaust first.

## Consequences

- **The gate result is a partial win, honestly bounded.** Plain log-magnitude flux recovers recall (up to 0.87) but over-fires on continuous spectral motion, so precision collapses (F1 0.515). **SuperFlux** (`max_filter_bins`, Böck & Widmer 2013) max-filters the previous spectrum across frequency before differencing, fixing the precision collapse: at `m=3` it reaches recall 0.629 with precision 0.509, F1 **0.542** — closing the gap to −0.023 and *beating* energy on the staccato fixtures (0.854 vs 0.750). But on the 40-stem Vocadito **onset-F1** gate it still falls short of energy's precision-driven 0.565. It ships off; energy stays default.
- **The note-level gate settled it against SuperFlux — and revealed why.** Onset-F1 is a proxy; the real metric is note-level COnP. The gate (`reports/note-onset-gate.md`) ran the full pipeline with energy / SuperFlux / ensemble onsets: **energy COnP-F1 0.646 beats every alternative (0.52–0.62)** — the false onsets over-split into spurious notes the HMM does *not* absorb, so the "absorb" hypothesis is falsified. The revealing detail: energy's onset *recall* is 0.44 but its note COnP is 0.65 — COnP exceeds onset recall because **the note-HMM already recovers most boundaries from pitch** (the interval-prior legato split, ADR-0010), making the onset stage and pitch-splitting partly redundant. The onset stage's real, non-redundant job is the same-pitch re-articulation split, which energy does precisely. So **onset recall was a proxy-metric red herring; it does not bottleneck the product.** If onsets are ever revisited, an ML onset model is the only remaining lever — but the note-level evidence says the payoff is small; offsets and the target-domain eval gap are the real levers.
- **No cost to the default build.** `realfft` is optional; the default path is byte-for-byte the ADR-0006 energy detector, still FFT-free and dependency-light.
- The `OnsetNovelty` seam and shared peak-picker mean a better novelty (SuperFlux tuning, an ensemble, or later an ML onset model) drops into the same slot; SuperFlux is already a field (`max_filter_bins`) on the `SpectralFlux` variant.
