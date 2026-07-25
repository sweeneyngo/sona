# 04 — Pitch pre-filter for the HMM input

Status: done
Blocked by: 03
Phase: B (note-HMM)

Prepare the contour the note-HMM decodes over.

- A short **3-tap median** on the voiced f0 track (SiPTH: 3 beat 5/7; wide medians eat short notes). pYIN already median-filters (`median_window: 5`) — decide whether the HMM consumes that or a dedicated width, and record why.
- Convert to a semitone/pitch representation the emission model reads.
- **Bilateral filter is deferred** (unproven here, can freeze real transitions) — leave a seam, don't build it unless 08's numbers ask for it.

Done when: the pre-filtered contour is available to the segmenter as (time, semitone, voicing) frames; no behaviour change yet (still feeds the old segmenter).

## Comments

**Done.** New `crates/sona-core/src/prefilter.rs`: `PitchPreFilter::run(&[F0Frame]) -> Vec<PitchFrame>`, where `PitchFrame { time_s, semitone: Option<f32>, voicing }` is the note-HMM's input unit (both re-exported from `lib.rs`). The median runs in **Hz space** over the voiced track (reusing the tracker's tested `median_filter_positive`, which skips unvoiced 0.0s) and converts once to continuous MIDI semitones via `hz_to_midi` afterward — Hz→semitone is monotonic, so the median commutes. `semitone` is gated on the **input** frame's voiced status (not the smoothed value), so the median can't resurrect a pitch on an unvoiced boundary frame — voiced-vs-silence stays the HMM's decision (ADR-0010). `voicing` is passed through verbatim for the emission model's voicing prior.

**Median-width decision (the ticket's standing question):** the HMM uses a **dedicated 3-tap median**, *not* pYIN's `median_window: 5`. Recorded in the module docs and on the `median_window` field. Two reasons: (1) SiPTH §5.3 — wide medians eat short notes; pYIN's 5 (~29 ms at the 256-sample hop) is tuned for a clean display contour + the old plateau segmenter, while the note-HMM wants the narrowest smoothing (3, ~17 ms) that still erases isolated octave spikes so a genuine short note survives to reach a state; (2) backend-independence — this stage is defined over the `F0Frame` contract, and CREPE (ticket 11) does no median filtering of its own, so owning the 3-tap here means the HMM sees bounded-jitter input from any producer. For pYIN the contour arrives already smoothed at 5, so the extra 3-tap is mostly a no-op. Consistent with ADR-0010's "A short (3-tap) median is the pre-filter."

**Bilateral filter:** deferred per ADR-0010 — not built. A documented seam marks where it would slot in (`PitchPreFilter::run`, between the median and the semitone conversion) if 08's sweep asks for it.

**No behaviour change:** `pipeline.rs`/`Segmenter` untouched; the old segmenter still runs on the raw pYIN contour. The note-HMM that consumes these `PitchFrame`s lands in 05–07.

8 unit tests (semitone conversion, 3-tap octave-spike erasure, the width-3-vs-5 short-note contrast that concretizes the decision, verbatim time/voicing pass-through, unvoiced-neighbour isolation, empty/count edge cases). Full workspace suite green (71 sona-core tests). Two-axis review passed: Spec clean; Standards removed a redundant zero-guard closure already covered by `median_filter_positive`.
