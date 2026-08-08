# 01 — Gated spectral-flux novelty + the OnsetNovelty seam

Status: done
Phase: implementation

Factor the onset detector's novelty function into a swappable seam and add spectral flux behind a feature.

- `OnsetNovelty { EnergyRise (default), #[cfg(feature="spectral")] SpectralFlux }`; `OnsetDetector::novelty` field.
- Split `onsets_from_envelope` into `energy_novelty` + shared `peak_pick`; `detect` selects the novelty source (spectral needs the samples, so it routes through `detect`, not the envelope-only entry point).
- `onset/spectral.rs` (feature-gated): log-magnitude spectral flux on the **same frame grid as the energy envelope**, `realfft`, Hann window.
- `realfft` optional dep + `spectral` feature on sona-core (and a forwarding `spectral` feature on sona-eval).

Done when: default build has no FFT; `--features spectral` builds; the energy path is byte-for-byte unchanged; unit tests cover the legato-onset recovery, the clean-staccato case, and the bleed guard on the spectral path.

## Comments

**Done.** `OnsetNovelty` seam in `onset.rs`; `onset/spectral.rs` behind the feature. 83 core tests green with `--features spectral` (80 without); default and spectral builds both clean; clippy clean. The legato capability test (`recovers_a_legato_onset_the_energy_rise_misses`) proves spectral flux fires on a constant-energy pitch change the energy rise is blind to.
