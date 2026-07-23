# Onset detection is energy-based, hand-rolled, not spectral-flux/FFT

The spec's pipeline stage 2 calls for "energy/spectral transient" onset detection (ticket 04). Making that concrete forced a choice between an energy-novelty detector over the RMS envelope and a spectral-flux detector over a spectrogram.

**Energy novelty, no FFT.** For the target material (staccato Vocaloid: short notes, clean gaps, sharp attacks from near the noise floor — CONTEXT.md), a note attack *is* an energy rise. So `sona-core::onset` works on the RMS envelope: half-wave-rectify the frame-to-frame difference of log-energy into an onset-novelty function, then peak-pick it with an adaptive threshold and a debounce gap. No spectrogram, no FFT, no new dependency — consistent with the hand-rolled pure-Rust DSP stance ADR-0005 took for pYIN. Log compression makes attacks read as comparable-size steps regardless of note loudness and pushes the between-note floor far enough down that its jitter doesn't rival a real attack.

**Bleed guard is a peak-relative floor.** The done-criterion is "few false positives from bleed." Separation bleed sits near the noise floor, so a candidate whose post-attack RMS never reaches a fraction of the stem's peak RMS (`bleed_floor_frac`, default 0.1) is rejected. This is the onset-stage guard; the note-level bleed guard in the confidence stage (ticket 06) is separate and complementary.

**Same-pitch re-attacks vs. legato splits.** An energy detector catches a re-articulated note at the *same* pitch — the case pure pitch tracking misses, since there's no pitch change. The converse case — two *legato* pitches at steady energy — has no energy rise, so this detector deliberately does *not* split it; that is segmentation's pitch-plateau step (ticket 05, spec stage 3). Onset detection owns energy transients; pitch-driven splits live downstream.

## Consequences

- No FFT crate; the envelope is `O(frame_length)` per hop and the detector is a shallow, testable function over it.
- The energy/RMS envelope is computed once and returned with the onsets (`OnsetResult`), since it is also an annotation-JSON display aid (ADR-0004) and the raw material segmentation reads for offset steepness (ticket 05) and confidence reads for its bleed guard (ticket 06).
- Envelope frames are timestamped at the window **center**, so an attack is dated where it happens rather than a half-window early. This differs from the pYIN contour's left-aligned frame time; each frame carries its own `time_s`, so the two display aids still share one timeline.
- If real stems turn up onsets with no energy signature (e.g. a bowed legato instrument — out of the vocal target), a spectral-flux novelty can be added behind the same `OnsetDetector` seam without changing callers.
