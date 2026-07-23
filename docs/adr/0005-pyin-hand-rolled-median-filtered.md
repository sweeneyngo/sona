# pYIN is hand-rolled pure-Rust DSP, smoothed by a median filter

ADR-0001 names pYIN the default `PitchTracker` and calls it "pure-Rust DSP." Making that concrete surfaced two decisions.

**No pitch crate.** The scaffold (ticket 01) wired `pitch-detection` as a placeholder, but it implements the McLeod Pitch Method (autocorrelation), not YIN. The other candidate, `aubio-rs`, has real pYIN but binds a system C library — at odds with "pure Rust" and a build-portability cost. So pYIN is implemented directly in `sona-core::pitch::pyin`: YIN's cumulative-mean-normalized difference function (de Cheveigné & Kawahara, 2002) plus pYIN's probabilistic threshold step (Mauch & Dixon, 2014) — many thresholds drawn from a `Beta(2, 18)` prior, each contributing its mass to the first candidate below it, so a frame's voicing is the total mass that found *any* periodic candidate. `pitch-detection` is dropped from the dependency set.

**Median filter instead of the HMM.** Full pYIN decodes candidates with an HMM/Viterbi pass. We substitute the median filter the spec already asks for at the contour stage: it removes the isolated octave errors and jitter the HMM targets, without the state machine, and keeps the tracker a shallow, testable function. Vibrato survives as a slow ripple — splitting a vibrato'd note is segmentation's job (ticket 05), not the tracker's.

## Consequences

- No C toolchain or model download for the default backend; it builds anywhere Rust does.
- The voicing estimate is the Beta mass under threshold, not an HMM posterior — a per-frame decision. If cross-frame voiced/unvoiced smoothing proves necessary on real stems, a Viterbi pass can be added behind the same `PitchTracker` seam without changing callers.
- The naive difference function is `O(W · τ_max)` per frame. Fine for the short stems in scope; FFT-based autocorrelation is the optimization if it ever matters.
