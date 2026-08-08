# Spec: Spectral-flux onsets (feature-gated alternative novelty)

Status: done — capability landed, gated OFF. Note-level gate is decisive: energy wins; onset recall was a proxy-metric red herring (the note-HMM recovers boundaries from pitch).

## Goal

Recover the onsets the energy-rise detector misses. The onset-deviation diagnostic (`reports/onset-deviation.md`) showed the onset-F1 dip (≈0.565) is **recall-limited**: energy detects ~half the Vocadito onsets, blind to legato/soft attacks that change spectrum without a clear energy step. Add **spectral flux** ([ADR-0013](../../docs/adr/0013-spectral-flux-onsets.md)) as an alternative onset novelty behind a `spectral` feature, prove it on the harness, and promote to default only if it beats energy F1.

## Design

- New `OnsetDetector::novelty: OnsetNovelty` (`EnergyRise` default | `SpectralFlux`). The novelty function is the swappable seam; the peak-picker (local-max, adaptive threshold, RMS bleed guard, debounce) is shared.
- Spectral flux: pure-Rust `realfft`, log-magnitude, mean over bins of the half-wave-rectified rise, computed on the **same frame grid as the energy envelope** so the curve aligns index-for-index with the peak-picker.
- Feature-gated (`spectral`), off by default — energy stays the zero-dependency default (mirrors the `crepe` pattern). ADR-0005's "builds anywhere" holds (`realfft` is pure Rust); only ADR-0006's "no FFT" line is retired, and only under the feature.

## Non-goals

- No change to the default build (no FFT pulled in without `--features spectral`).
- No ML onset model here — pure-DSP spectral novelty is the cheaper lever to exhaust first.
- No change to the annotation contract, the `Onset` type, or the peak-picker's logic.

## Done when

- The `spectral` feature builds; default build has no FFT dependency.
- Unit tests: spectral flux recovers a legato onset energy misses; still gets a clean staccato onset; bleed guard still holds. Energy path byte-for-byte unchanged.
- **Gate ran** (`sona-eval-onset-compare`, energy vs spectral on Vocadito). Promote to default iff spectral beats energy F1 without regressing the staccato fixtures.

## Sequencing

- **01** — implement the gated capability: `OnsetNovelty` seam, `realfft` behind `spectral`, spectral novelty module, unit tests. *(done)*
- **02** — the gate: `sona-eval-onset-compare` energy vs spectral on the corpus. *(done — recall recovered, F1 not beaten; not promoted)*
- **03** — SuperFlux: a vibrato-robust novelty to convert recall into precision, then re-gate. *(done — recall 0.44→0.63, beats energy on staccato, but Vocadito onset-F1 0.542 < energy 0.565; not promoted)*
- **04** — Note-level (COnP) gate + energy⊕SuperFlux ensemble. *(done — energy COnP 0.646 beats all SuperFlux/ensemble variants; onset recall doesn't bottleneck notes because the HMM splits on pitch. Energy stays default; spectral stays gated off.)*
