# 03 — SuperFlux / whitening: convert recovered recall into precision

Status: done
Blocked by: 02
Phase: improvement

The gate (ticket 02) showed spectral flux recovers recall (0.87) but plain log-magnitude flux over-fires on vibrato/formant motion, so precision collapses and F1 doesn't beat energy. The fix is a vibrato-robust novelty, not a bigger threshold.

- **SuperFlux** (Böck & Widmer 2013): before differencing, max-filter the *previous* frame's magnitude spectrum across a few frequency bins (a small max-pool over ±k bins). A slowly-sweeping partial (vibrato) then falls within the max-filtered reference and does not register as flux, while a sharp broadband attack still does. This is the primary lever.
- Consider **spectral whitening / per-bin adaptive normalisation** so strong low harmonics don't dominate the flux.
- Consider a **peak-picker variant tuned for the flux baseline** (the shared one assumes energy's near-zero baseline; SuperFlux typically pairs with a moving-average threshold + short pre/post-max window).

Done when: `sona-eval-onset-compare` shows the SuperFlux novelty **beats energy F1 on Vocadito** without regressing the staccato fixtures. If so, promote it toward the default (a follow-up ADR step on ADR-0013) and re-run the onset-deviation diagnostic to confirm the recall gain and re-check the timing bias. If it still doesn't beat energy, that's the honest signal to consider an ML onset model instead.

Notes: keep it behind the `spectral` feature; SuperFlux is a novelty variant, so it slots into the same `OnsetNovelty` seam (e.g. `SpectralFlux { max_filter_bins, .. }` or a new variant).

## Comments

**Done.** SuperFlux implemented as `OnsetNovelty::SpectralFlux { max_filter_bins }` (0 = plain flux); `spectral.rs` max-filters the previous frame's log-magnitude spectrum over ±`m` bins before differencing. `sona-eval-onset-compare` now sweeps `max_filter_bins × threshold_delta`.

**Result (`reports/onset-compare.md`): strong partial win, not promoted.** SuperFlux fixes plain flux's precision collapse — m=3 reaches recall **0.629** (energy 0.441) at precision 0.509, F1 **0.542**, closing the gap to −0.023. It **beats energy on the staccato fixtures** (0.854 vs 0.750). But on the 40-stem Vocadito **onset-F1** gate it still falls short of energy's precision-driven 0.565. Shipped off; energy stays default. Two clear next levers → ticket 04.

