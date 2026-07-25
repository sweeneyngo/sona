//! **Note segmentation** — the third stage of the perception pipeline (spec
//! §Pipeline 3), the spec's highest-error area. This module owns the stage's
//! *contract* — the [`NoteSegment`] a note event is, and the [`Segmenter`] seam
//! (ADR-0004) that produces it — while the strategy behind the seam is the
//! Tony-style **note-HMM** ([`crate::hmm`], ADR-0010).
//!
//! [`Segmenter`] is a type alias for [`NoteHmm`](crate::hmm::NoteHmm): the
//! note-HMM *is* the segmenter now. The earlier ADR-0007 heuristic (hybrid
//! offsets + pitch-plateau splitting) was retired once the note-HMM beat its
//! baseline on Vocadito without regressing the staccato fixture (ticket 08; see
//! `crates/sona-eval/reports/note-hmm-sweep.md`). Its one surviving output, the
//! offset energy-drop steepness, lives on here as [`drop_steepness`].
//!
//! ## Raw material for confidence (ticket 06)
//!
//! Every [`NoteSegment`] carries what the confidence stage can't recover later:
//! the onset [`strength`](NoteSegment::onset_strength) (onset clarity), and per
//! offset the [`drop_steepness`](OffsetInfo::drop_steepness) and how the offset
//! was decided ([`OffsetKind`]) (offset clarity). Pitch stability and voicing it
//! recomputes from the contour it already has.

use crate::annotation::EnergyFrame;

/// The note segmentation seam (ADR-0004): raw perception tracks → note events.
/// The strategy is the note-HMM (ADR-0010); this alias keeps the seam's name
/// stable for everything downstream (pipeline, confidence, CLI) while the
/// implementation lives in [`crate::hmm`].
pub type Segmenter = crate::hmm::NoteHmm;

/// How a note's offset was decided. Recorded per offset so the confidence stage
/// (ticket 06) and the UI can tell *why* a note ends where it does. The
/// confidence stage reads only the [`drop_steepness`](OffsetInfo::drop_steepness),
/// not this kind — it is for the UI / debugging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetKind {
    /// The note is seamlessly followed by another note that carries its own
    /// detected onset — back-to-back re-articulated notes.
    NextOnset,
    /// The note ends into a rest: the decode entered silence before the next
    /// note (or the energy decayed away).
    EnergyDecay,
    /// The note is seamlessly followed by a pitch-born note with no onset of its
    /// own — a legato pitch change (a "plateau split" in the old vocabulary).
    PlateauSplit,
    /// Closed by the end of the stem (the last note, still sounding).
    StemEnd,
}

/// The raw material an offset hands to the confidence stage (ticket 06).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffsetInfo {
    /// How this offset was decided.
    pub kind: OffsetKind,
    /// Energy-drop steepness across the offset, in `0.0..=1.0`: the fractional
    /// fall in RMS over a short window (sharp drop → high, slow tail → low).
    /// Offset-clarity confidence is derived from this, not from which
    /// [`OffsetKind`] fired.
    pub drop_steepness: f32,
}

/// A segmented note event: raw seconds, a MIDI pitch, and the raw material the
/// confidence stage (ticket 06) needs. This is *not* an annotation [`Note`] yet
/// — confidence composes the scalar + sub-scores from these fields downstream.
///
/// [`Note`]: crate::annotation::Note
#[derive(Debug, Clone, PartialEq)]
pub struct NoteSegment {
    /// Note onset, in seconds from the start of the stem.
    pub onset_s: f64,
    /// Note offset, in seconds from the start of the stem.
    pub offset_s: f64,
    /// MIDI pitch number (0–127): the note's pitch, an alpha-trimmed mean of its
    /// voiced f0 frames rounded to the nearest semitone (ADR-0010).
    pub midi_pitch: u8,
    /// The onset transient's novelty (from the [`Onset`](crate::onset::Onset)),
    /// carried through for onset-clarity confidence. `0.0` for a pitch-born note
    /// (a legato split), which has no transient of its own.
    pub onset_strength: f32,
    /// Whether this note began at a pitch change with no detected onset (legato /
    /// slide) rather than an attack. Its onset time is a pitch change.
    pub split_from_plateau: bool,
    /// How this note's offset was decided, and its energy-drop steepness.
    pub offset: OffsetInfo,
    /// **HMM-derived pitch dispersion** (ticket 09): the RMS spread, in semitones,
    /// of the note's voiced frames about their mean pitch (rounding-free, so an
    /// off-tune note isn't penalised for sitting between semitones). Low = the
    /// contour sat tightly on one pitch; high = it wandered. Confidence's
    /// pitch-stability sub-score reads this instead of recomputing a spread from
    /// the raw contour — it is measured over the frames the HMM *voiced*.
    pub pitch_dispersion_semitones: f32,
    /// **HMM-derived voicing confidence** (ticket 09) in `0.0..=1.0`: the mean,
    /// over the note's frames, of the model's local (per-frame, transition-free)
    /// voiced-vs-silent **emission evidence** — a sigmoid of
    /// [`HmmModel::voiced_logodds`](crate::hmm::HmmModel::voiced_logodds), *not* a
    /// forward-backward posterior. It folds pYIN voicing together with pitch fit
    /// (so it is not strictly pitch-independent), giving confidence's voicing
    /// sub-score what the HMM concluded rather than the raw mean voicing.
    pub voiced_confidence: f32,
}

/// Continuous MIDI pitch of a frequency in Hz (69 = A4 = 440 Hz). Fractional —
/// callers round when they need a discrete pitch.
pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// Round a continuous MIDI pitch to the nearest integer note, clamped to `0..=127`.
pub fn midi_round_f32(midi: f32) -> u8 {
    midi.round().clamp(0.0, 127.0) as u8
}

/// Fractional fall in RMS across `offset_s`, using a half-window of `window_s`:
/// the mean RMS in the window just after minus that just before, over the
/// before-level, clamped to `0.0..=1.0`. A short window makes this a *rate* — a
/// staccato drop to the floor scores near 1, a slow tail near 0. Returns 0 when
/// there is no energy before the offset to fall from.
///
/// This is ADR-0007's one surviving output: the note-HMM (ADR-0010) measures it
/// purely as raw material for offset-clarity confidence (ticket 06) — not to
/// *decide* the offset.
pub(crate) fn drop_steepness(envelope: &[EnergyFrame], offset_s: f64, window_s: f64) -> f32 {
    let mean_rms = |lo: f64, hi: f64| -> Option<f32> {
        let vals: Vec<f32> = envelope
            .iter()
            .filter(|f| f.time_s >= lo && f.time_s < hi)
            .map(|f| f.rms)
            .collect();
        if vals.is_empty() {
            None
        } else {
            Some(vals.iter().sum::<f32>() / vals.len() as f32)
        }
    };
    let before = mean_rms(offset_s - window_s, offset_s);
    let after = mean_rms(offset_s, offset_s + window_s);
    match (before, after) {
        (Some(b), Some(a)) if b > f32::EPSILON => (1.0 - a / b).clamp(0.0, 1.0),
        // No "before" energy (offset at/near the stem start) — nothing to fall
        // from; treat as no measurable drop.
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hz_to_midi_maps_a440_to_69() {
        assert!((hz_to_midi(440.0) - 69.0).abs() < 1e-4);
        assert!((hz_to_midi(880.0) - 81.0).abs() < 1e-4); // octave up
    }

    #[test]
    fn midi_round_snaps_and_clamps() {
        assert_eq!(midi_round_f32(hz_to_midi(261.63)), 60); // middle C
        assert_eq!(midi_round_f32(69.4), 69);
        assert_eq!(midi_round_f32(-5.0), 0);
        assert_eq!(midi_round_f32(200.0), 127);
    }

    #[test]
    fn drop_steepness_is_high_for_a_sharp_fall_low_for_a_tail() {
        let hop = 256.0 / 44_100.0;
        let frames = |levels: &[(f64, f32)]| -> Vec<EnergyFrame> {
            let end = levels.last().map(|l| l.0).unwrap_or(0.0);
            let n = (end / hop) as usize;
            (0..n)
                .map(|i| {
                    let t = i as f64 * hop;
                    let rms = levels.iter().find(|(u, _)| t < *u).map(|(_, r)| *r).unwrap_or(0.0);
                    EnergyFrame { time_s: t, rms }
                })
                .collect()
        };
        // Sharp: full level then near-zero across 0.3 s.
        let sharp = frames(&[(0.3, 0.5), (0.6, 0.01)]);
        assert!(drop_steepness(&sharp, 0.3, 0.04) > 0.8);
        // Tail: a gentle decline scores low at its midpoint.
        let tail = frames(&[(0.3, 0.5), (0.35, 0.45), (0.4, 0.42), (0.6, 0.4)]);
        assert!(drop_steepness(&tail, 0.4, 0.04) < 0.3);
        // No "before" energy → 0.
        assert_eq!(drop_steepness(&sharp, 0.0, 0.04), 0.0);
    }
}
