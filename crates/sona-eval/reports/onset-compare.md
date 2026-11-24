# Onset detector comparison: energy-rise vs spectral-flux (ADR-0013 gate)

Spectral-flux onsets and this binary were removed in audit A07 (2026-11-24); this report is the record. Restore them from the parent of the A07 change to re-run.

Run: `cargo run --release -p sona-eval --features spectral --bin sona-eval-onset-compare -- --corpus`
Corpus: Vocadito, 40 stems + 4 golden fixtures. Onset P/R/F1 at the metric's ±50 ms. Spectral-flux `threshold_delta` is swept because its novelty scale/baseline differs from energy's (the shared peak-picker's threshold cannot be inherited).

## Vocadito corpus (40 stems)

| detector | mean P | mean R | mean F1 | detected | reference |
|---|---|---|---|---|---|
| **energy-rise** | 0.854 | 0.441 | **0.565** | 1109 | 2237 |
| spectral δ=0.02 | 0.302 | **0.872** | 0.442 | 6758 | 2237 |
| spectral δ=0.05 | 0.404 | 0.743 | 0.511 | 4349 | 2237 |
| spectral δ=0.10 | 0.602 | 0.476 | 0.515 | 1839 | 2237 |
| spectral δ=0.20 | 0.641 | 0.271 | 0.362 | 923 | 2237 |
| spectral δ=0.30 | 0.660 | 0.191 | 0.275 | 610 | 2237 |
| spectral δ=0.50 | 0.608 | 0.114 | 0.175 | 351 | 2237 |

Golden fixtures echo it: best spectral F1 0.676 (δ=0.10) vs energy 0.750 — spectral does not regress catastrophically but does not win.

### SuperFlux (`max_filter_bins`, per-width best-F1 threshold)

Re-run adding the SuperFlux dimension: max-filter the previous frame's spectrum over ±`m` bins before differencing, to reject vibrato.

| detector | mean P | mean R | mean F1 | detected |
|---|---|---|---|---|
| energy-rise | 0.854 | 0.441 | **0.565** | 1109 |
| plain flux (m=0, δ=0.10) | 0.602 | 0.476 | 0.515 | 1839 |
| superflux m=1 (δ=0.05) | 0.580 | 0.534 | 0.537 | 2184 |
| **superflux m=3 (δ=0.02)** | 0.509 | **0.629** | **0.542** | 3081 |
| superflux m=5 (δ=0.02) | 0.552 | 0.541 | 0.523 | 2406 |
| superflux m=8 (δ=0.02) | 0.577 | 0.467 | 0.492 | 1945 |

Staccato fixtures: superflux m=1 **F1 0.854 vs energy 0.750** — SuperFlux *wins* on the target-style material.

## Verdict: SuperFlux works, but on Vocadito onset-F1 it still falls just short

1. **The missing onsets are recoverable** — SuperFlux m=3 reaches recall **0.629** (energy 0.441, +0.19).
2. **SuperFlux fixes plain flux's precision collapse** — at comparable recall its precision is far better than plain flux (0.509 vs 0.30), closing the F1 gap from −0.050 (plain) to **−0.023 (0.542 vs energy 0.565)**.
3. **But energy still wins the Vocadito onset-F1 gate**: its very high precision (0.854) keeps its F1 ahead of every SuperFlux setting. On the staccato fixtures SuperFlux *beats* energy (0.854 vs 0.750), but the 40-stem corpus is the primary gate.

## Decision (ADR-0013)

Ship spectral flux + SuperFlux **behind the `spectral` feature, OFF by default** — energy-rise stays the default. Not promoted: it does not clear energy's onset-F1 on Vocadito. But this is a strong partial result (recall +0.19, wins on staccato, F1 within 0.023), and it exposes the right next questions:

- **Onset-F1 is a proxy — the real target is note-level COnP.** Onsets feed the HMM's same-pitch splitter: a *missed* onset merges two notes (a boundary lost), while a *false* onset is an over-split the HMM's other evidence can absorb. So SuperFlux's higher recall at slightly lower onset-F1 may still *improve* end-to-end COnP. **The next gate should run the full pipeline (energy vs SuperFlux onsets → note-F1), not just onset-F1** — that could flip the verdict.
- **Energy⊕SuperFlux ensemble.** Energy has precision (0.854), SuperFlux has recall (0.629). A union (energy's onsets + SuperFlux onsets energy missed) could capture both — a cheap, promising combination.
- **Spectral whitening / a flux-baseline peak-picker** remain available levers.

If the COnP gate and the ensemble both fail to beat energy end-to-end, that is the honest signal to reach for an ML onset model.
