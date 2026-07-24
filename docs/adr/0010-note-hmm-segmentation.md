# Note segmentation is a note-HMM over the pitch contour, not offset/plateau heuristics

**Supersedes the segmentation core of [ADR-0007](0007-hybrid-offsets-plateau-splitting.md).** The energy-onset stage (ADR-0006) and pYIN tracker (ADR-0005) are unchanged; this replaces how their outputs become notes.

## Context

ADR-0007's segmenter turns onsets + f0 contour + energy envelope into notes with two stacked heuristics: a two-candidate offset competition (next-onset vs energy-decay) and pitch-plateau splitting with a short-run absorber. It was tuned for staccato Vocaloid. The target has since broadened to **expressive/normal vocals** (the relaxed `Segmenter`/`OnsetDetector` defaults), where the heuristics show their limits:

- Four coupled, empirical thresholds (`min_plateau_s`, `rest_floor_frac`, `voiced_threshold`, `max_offset_gap_s`) that interact, all tuned by eye — no way to know which to move when notes are sparse or fragmented.
- Vibrato and slides are handled by *absorbing* short plateaus, which fights sustained legato and can over-merge real short notes.
- Octave-jump artifacts (a period-multiple error out of time-domain YIN, ADR-0005) leak through and are currently patched in the **UI** (`web/lib/view.ts`: a slope filter and a MAD outlier trim) — perception logic living in the interpretation layer, against ADR-0004.

The singing-transcription literature converges on a single mechanism that subsumes all of this: a **note-model HMM over the pitch track, Viterbi-decoded** (Mauch et al., *Tony*, TENOR 2015; Ryynänen & Klapuri). Ycart & Benetos's "music language model" work shows that, for a monophonic melody, a learned sequence prior reduces to the same thing — an HMM transition prior whose main effect is temporal smoothing and spurious-change suppression. Two ideas, one implementation.

Crucially this is **pure DSP + dynamic programming — no ML runtime, consistent with ADR-0005.** Viterbi over a vocal-range pitch grid is `O(frames × states²)`, trivial for a monophonic ~4-octave range.

## Decision

Replace the heuristic segmentation core with a **Tony-style note-HMM** over the (median-smoothed) pYIN contour:

- **States** — pitches across the vocal range at **3 bins per semitone** (not hard integer MIDI: sub-semitone bins keep off-tune / non-A440 / expressive intonation from mis-snapping), plus a **silence** state. Each pitch is modeled with **Attack / Stable / Silent sub-states** (Ryynänen/Tony), so note beginnings tolerate more pitch variance than steady middles.
- **Emission** — Gaussian on the pitch deviation (in semitones) between the frame and the state's center pitch, sharpened by a trust exponent (τ≈0.1). Std-dev is sub-state–dependent: **Attack σ≈5**, **Stable σ≈0.9** semitones. A voicing prior (v≈0.5) splits mass between voiced states and the silence state, driven by the pYIN voicing probability the contour already carries.
- **Transitions** — left-to-right within a note (Attack→Stable→Silent) with self-transition probabilities ≈ **0.9 / 0.99 / 0.9999**; the Silent state fans out to other notes' Attack states through an **interval prior** that favours small pitch changes and makes large jumps (octaves) expensive. *This interval penalty is what rejects the octave artifacts at the source — replacing, and letting us delete, the UI-side slope filter and MAD trim.*
- **Energy as the same-pitch splitter** — pitch alone cannot separate two re-articulated notes at one pitch (the documented over-merge failure of pitch-only methods, e.g. SiPTH's "lalala"). The existing energy-onset detector (ADR-0006, `onset.rs`) supplies exactly this: a detected onset forces an Attack/Silence transition, splitting the syllable. This is Tony's RMS-rise splitter — and the piece sona already had. **`onset.rs` is kept and becomes an HMM input, not a separate segmentation stage.**
- **Final pitch, decoupled from the decode grid** — a note's emitted MIDI pitch is an **alpha-trimmed mean** of its voiced frames, rounded to integer MIDI, computed *after* decoding. The 3-bin grid is for decoding stability; the annotation still emits integer MIDI.
- **Post-processing** — prune notes shorter than **~100 ms** (literature consensus) and merge adjacent same-pitch fragments. Offsets fall out of the Silent-state transition; the energy-drop steepness ADR-0007 recorded is still measured, purely as raw material for offset-clarity confidence.

The `Segmenter` seam and its `NoteSegment` output contract are preserved, so downstream (confidence, CLI, annotation JSON, ADR-0004) is untouched. This is an internal replacement behind a stable interface.

## Rejected alternatives

- **First-derivative boundary detection** (from the original proposal). Both SiPTH and Tony explicitly reject differentiating vocal pitch: it amplifies jitter, vibrato, and portamento into spurious boundaries. Boundaries are decided inside the HMM transitions instead.
- **Hard 128 integer-MIDI single-state HMM.** Simpler, but mis-snaps off-tune / drifting / expressive singers and offers no attack-variance modelling. Adopted the 3-bin + sub-state structure per Tony's evidence.
- **Bilateral pitch pre-filter.** Unproven in this domain (median / Savitzky-Golay dominate) and its range kernel can freeze real fast transitions. A short (3-tap) median is the pre-filter; bilateral is deferred unless the harness shows a need.
- **Keeping the ADR-0007 heuristics, tuned harder.** The four coupled knobs can't be tuned objectively by ear; a Viterbi self-transition probability is one principled knob with a measurable effect.

## Consequences

- **Tuning surface collapses to one main knob** — the Stable self-transition probability (note-change sensitivity). Higher merges (kills vibrato jitter, risks merging fast notes); lower admits jitter. Ycart's caution stands: an over-confident prior makes real notes disappear — so this is tuned against the harness, not by eye.
- **This change is not eye-tunable and must be gated by measurement.** It cannot land without the evaluation harness ([ADR-0011](0011-evaluation-harness-vocadito-metrics.md)) established first, so the note-HMM can be proven to beat the ADR-0007 baseline on real vocals (per-stage note-F1) rather than assumed to.
- **UI perception hacks move back into the core.** The `web/lib/view.ts` slope filter and pitch-outlier MAD trim are removed once the interval prior subsumes them (restores the ADR-0004 core/UI boundary).
- **Confidence gets better raw material.** HMM state posteriors give principled pitch-stability and voicing sub-scores in place of recomputed heuristics (a follow-up to ADR-0008).
- Viterbi is `O(frames × states²)`; with ~4 octaves × 3 bins (~150 states) it is well within the current per-run budget. If it ever isn't, the FFT-autocorrelation speedup for YIN (ADR-0005) is the first lever, not the HMM.
- ADR-0007's hybrid-offset design is retired; its `drop_steepness` measurement survives as a confidence input only.
