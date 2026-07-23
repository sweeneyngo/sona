//! **Confidence scoring** — the fourth stage of the perception pipeline (spec
//! §Pipeline 4), landed by ticket 06. The spec calls this the highest-leverage
//! stage: confidence is what turns "re-listen to the whole song" into "check
//! these few flagged notes" (CONTEXT.md), so it is where the review surface earns
//! its keep.
//!
//! The stage turns a [`NoteSegment`] (ticket 05) — which carries raw perception
//! but no judgement — into an annotation [`Note`]: the same timing and pitch, now
//! with a confidence scalar and its four [`SubScores`]. It re-reads the f0
//! [contour](F0Frame) and energy [envelope](EnergyFrame) for the dimensions a
//! segment can't recover on its own (pitch spread, voicing, note level).
//!
//! ## Four sub-scores, each in `0.0..=1.0` (spec stage 4)
//!
//! - **pitch_stability** — from the f0 *spread* across the note. A steady pitch
//!   scores high; a note whose contour wanders scores low. Vibrato-tolerant: a
//!   bounded ripple has bounded spread, so it is not punished as instability.
//! - **voicing** — pYIN voicing **and** a bleed guard. The mean per-frame voicing
//!   confidence is multiplied by a note-level factor that falls to zero as the
//!   note's energy approaches the stem's noise floor. This is the spec's primary
//!   defence against spurious notes from UVR separation bleed: a note hovering
//!   near the floor scores near zero here however voiced it looks.
//! - **onset_clarity** — the onset transient's sharpness, saturated from the
//!   segment's [`onset_strength`](NoteSegment::onset_strength). A plateau-split
//!   note has no transient of its own, so its onset — inferred from a pitch change
//!   rather than measured from an attack — takes a middling baseline: flagged as
//!   uncertain, but not zeroed out as if the note weren't real.
//! - **offset_clarity** — the energy-drop steepness at the offset, read straight
//!   from [`drop_steepness`](crate::segment::OffsetInfo::drop_steepness): a sharp
//!   staccato drop scores high, a slow tail low (spec: from steepness, *not* from
//!   which fallback fired).
//!
//! ## Weakest-link soft-min → the scalar (spec stage 4, ADR-0008)
//!
//! A note is only as trustworthy as its shakiest dimension, so the scalar is a
//! **soft-min** of the four sub-scores, not their average — averaging would let
//! three strong dimensions mask the one that needs flagging. The soft-min is a
//! negative-power mean ([`soft_min`]) that sits just above the true minimum: a
//! single weak dimension drags the scalar down to near it, while a zero anywhere
//! forces the scalar to zero. Both the scalar and the sub-scores are emitted — the
//! sub-scores are what let the UI say *why* a note is flagged.

use crate::annotation::{EnergyFrame, F0Frame, Note, SubScores};
use crate::segment::{hz_to_midi, NoteSegment};

/// Per-note confidence scorer (ADR-0008). Construct with
/// [`ConfidenceScorer::default`] for staccato-vocal defaults, or set fields to
/// tune. Every threshold is empirical — the spec expects them re-tuned by ear
/// against real Vocaloid stems, so they are public fields a comparison run can
/// sweep without a code change.
#[derive(Debug, Clone)]
pub struct ConfidenceScorer {
    /// Pitch-spread tolerance in semitones for `pitch_stability`. The note's f0
    /// standard deviation (in semitones) is scored against this: at one tolerance
    /// of spread the score is `0.5`. Sized to tolerate vibrato without rewarding a
    /// note that wanders across a semitone.
    pub pitch_std_tol_semitones: f32,
    /// Voicing bleed guard, low edge: a note whose peak RMS is at or below this
    /// fraction of the stem's peak RMS is treated as pure bleed — its note-level
    /// factor is `0`. Mirrors the onset stage's bleed floor (ADR-0006).
    pub bleed_floor_frac: f32,
    /// Voicing bleed guard, high edge: at or above this fraction of the stem's
    /// peak RMS the note reads as a real, full-level note (factor `1`). Between the
    /// two edges the factor ramps linearly.
    pub note_level_frac: f32,
    /// Onset-clarity saturation half-point: an [`onset_strength`] equal to this
    /// scores `0.5`, larger transients saturate toward `1`. Since strength is an
    /// uncalibrated log-energy novelty, this sets what counts as a "clear" attack.
    ///
    /// [`onset_strength`]: NoteSegment::onset_strength
    pub onset_strength_half: f32,
    /// Onset clarity assigned to a plateau-split note, which has no transient of
    /// its own (`onset_strength = 0`, ADR-0007) but whose onset time *is* well
    /// located — it is the pitch change. A middling default: honestly below a real
    /// attack (so a slide is flagged "⚠ uncertain onset"), but not the hard `0`
    /// that the soft-min would turn into zero confidence, which would bury a real
    /// legato note among dead bleed. Reserved for split notes; a detected onset
    /// scores from its strength.
    pub split_onset_clarity: f32,
    /// Soft-min power (negative). More negative pulls the scalar closer to the
    /// hard minimum of the sub-scores; toward `0` softens it toward their mean.
    /// See [`soft_min`].
    pub softmin_p: f32,
    /// A contour frame counts as voiced (for pitch spread and the voicing mean)
    /// when its voicing reaches this and its frequency is positive. Matches the
    /// segmenter's `voiced_threshold` so both stages agree on what "voiced" means.
    pub voiced_threshold: f32,
}

impl Default for ConfidenceScorer {
    fn default() -> Self {
        Self {
            pitch_std_tol_semitones: 0.6,
            bleed_floor_frac: 0.1,
            note_level_frac: 0.3,
            onset_strength_half: 0.5,
            split_onset_clarity: 0.5,
            softmin_p: -4.0,
            voiced_threshold: 0.5,
        }
    }
}

impl ConfidenceScorer {
    /// Score every `segment` against the f0 `contour` and energy `envelope`,
    /// returning annotation [`Note`]s in the same order. The stem's peak RMS (for
    /// the bleed guard) is computed once from the envelope and shared across notes,
    /// so the guard is relative to the whole stem's level, not each note's.
    pub fn score_all(
        &self,
        segments: &[NoteSegment],
        contour: &[F0Frame],
        envelope: &[EnergyFrame],
    ) -> Vec<Note> {
        let stem_peak_rms = envelope.iter().map(|f| f.rms).fold(0.0f32, f32::max);
        segments
            .iter()
            .map(|s| self.score(s, contour, envelope, stem_peak_rms))
            .collect()
    }

    /// Score a single `segment` into a [`Note`], given the `stem_peak_rms` the
    /// bleed guard is relative to. Exposed for callers that already have the peak;
    /// most callers want [`score_all`](Self::score_all).
    pub fn score(
        &self,
        segment: &NoteSegment,
        contour: &[F0Frame],
        envelope: &[EnergyFrame],
        stem_peak_rms: f32,
    ) -> Note {
        let sub_scores = SubScores {
            pitch_stability: self.pitch_stability(segment, contour),
            voicing: self.voicing(segment, contour, envelope, stem_peak_rms),
            onset_clarity: self.onset_clarity(segment),
            offset_clarity: self.offset_clarity(segment),
        };
        let confidence = soft_min(
            &[
                sub_scores.pitch_stability,
                sub_scores.voicing,
                sub_scores.onset_clarity,
                sub_scores.offset_clarity,
            ],
            self.softmin_p,
        );
        Note {
            onset_s: segment.onset_s,
            offset_s: segment.offset_s,
            midi_pitch: segment.midi_pitch,
            confidence,
            sub_scores,
        }
    }

    /// `pitch_stability` from the spread of the note's voiced f0. The frequencies
    /// are taken in semitones (so the spread is musical, not Hz-biased toward high
    /// notes) and their standard deviation scored against
    /// [`pitch_std_tol_semitones`](Self::pitch_std_tol_semitones): `0` spread → 1,
    /// one tolerance of spread → `0.5`, wide wander → toward `0`. A note with no
    /// voiced frames scores `0` — an unpitched note is untrustworthy, and letting
    /// the bleed guard and this dimension both bottom out reinforces the flag.
    fn pitch_stability(&self, segment: &NoteSegment, contour: &[F0Frame]) -> f32 {
        let semitones: Vec<f32> = self
            .voiced_frames(segment, contour)
            .map(|f| hz_to_midi(f.frequency_hz))
            .collect();
        if semitones.is_empty() {
            return 0.0;
        }
        let n = semitones.len() as f32;
        let mean = semitones.iter().sum::<f32>() / n;
        let variance = semitones.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / n;
        let std = variance.sqrt();
        // A rational falloff: 1 at zero spread, 0.5 at one tolerance, →0 beyond.
        let r = std / self.pitch_std_tol_semitones.max(f32::EPSILON);
        1.0 / (1.0 + r * r)
    }

    /// `voicing` = mean pYIN voicing over the note's frames × a bleed-guard factor.
    /// The mean is over *every* frame in the note's span (voiced or not) so a note
    /// that is mostly unvoiced scores low. The bleed factor ramps from `0` at
    /// [`bleed_floor_frac`](Self::bleed_floor_frac) to `1` at
    /// [`note_level_frac`](Self::note_level_frac) of the stem's peak RMS, measured
    /// from the note's *peak* RMS — a real note reaches a real level, bleed never
    /// does. Multiplying (not averaging) means either a low-voiced note or a
    /// near-floor note scores low, which is the point: bleed is culled here.
    fn voicing(
        &self,
        segment: &NoteSegment,
        contour: &[F0Frame],
        envelope: &[EnergyFrame],
        stem_peak_rms: f32,
    ) -> f32 {
        let frames: Vec<&F0Frame> = contour
            .iter()
            .filter(|f| f.time_s >= segment.onset_s && f.time_s < segment.offset_s)
            .collect();
        let mean_voicing = if frames.is_empty() {
            0.0
        } else {
            frames.iter().map(|f| f.voicing).sum::<f32>() / frames.len() as f32
        };

        let note_peak_rms = envelope
            .iter()
            .filter(|f| f.time_s >= segment.onset_s && f.time_s < segment.offset_s)
            .map(|f| f.rms)
            .fold(0.0f32, f32::max);
        let bleed_factor = if stem_peak_rms <= f32::EPSILON {
            0.0
        } else {
            let frac = note_peak_rms / stem_peak_rms;
            let lo = self.bleed_floor_frac;
            let hi = self.note_level_frac.max(lo + f32::EPSILON);
            ((frac - lo) / (hi - lo)).clamp(0.0, 1.0)
        };

        (mean_voicing * bleed_factor).clamp(0.0, 1.0)
    }

    /// `onset_clarity` for a detected onset is saturated from its strength:
    /// `strength / (strength + half)`. A plateau-split note has no transient of its
    /// own, so instead of the `0` that would collapse its confidence entirely it
    /// takes the middling [`split_onset_clarity`](Self::split_onset_clarity): its
    /// onset is *inferred* from a pitch change, not measured from an attack — worth
    /// flagging as an uncertain onset, but not burying as if the note weren't real.
    fn onset_clarity(&self, segment: &NoteSegment) -> f32 {
        if segment.split_from_plateau {
            return self.split_onset_clarity.clamp(0.0, 1.0);
        }
        let s = segment.onset_strength.max(0.0);
        let half = self.onset_strength_half.max(f32::EPSILON);
        s / (s + half)
    }

    /// `offset_clarity` straight from the offset's energy-drop steepness (already
    /// in `0.0..=1.0`): sharp drop → high, slow tail → low. Per the spec this comes
    /// from the steepness, not from which [`OffsetKind`] closed the note.
    ///
    /// [`OffsetKind`]: crate::segment::OffsetKind
    fn offset_clarity(&self, segment: &NoteSegment) -> f32 {
        segment.offset.drop_steepness.clamp(0.0, 1.0)
    }

    /// The note's voiced contour frames: within `[onset_s, offset_s)`, voicing at
    /// or above the threshold, positive frequency.
    fn voiced_frames<'a>(
        &self,
        segment: &'a NoteSegment,
        contour: &'a [F0Frame],
    ) -> impl Iterator<Item = &'a F0Frame> {
        let onset_s = segment.onset_s;
        let offset_s = segment.offset_s;
        let voiced_threshold = self.voiced_threshold;
        contour.iter().filter(move |f| {
            f.time_s >= onset_s
                && f.time_s < offset_s
                && f.voicing >= voiced_threshold
                && f.frequency_hz > 0.0
        })
    }
}

/// Weakest-link **soft-min** of `scores` (each expected in `0.0..=1.0`) via a
/// negative-power mean: `(mean xᵖ)^(1/p)` with `p < 0`. It sits just above the
/// true minimum — one weak score drags the result down near it while the others
/// nudge it up slightly — so it flags the shakiest dimension without the brittle
/// all-or-nothing of a hard `min`. A zero anywhere forces the result to zero
/// (that term's power is `+∞`), which is the desired weakest-link behaviour: a
/// dead dimension means no confidence. Returns `0.0` for an empty slice.
pub fn soft_min(scores: &[f32], p: f32) -> f32 {
    if scores.is_empty() {
        return 0.0;
    }
    let n = scores.len() as f32;
    // x^p with p<0: x=0 → +∞, so the mean is +∞ and (∞)^(1/p) = 0. No special-
    // casing needed — a zero score correctly collapses the whole soft-min.
    let mean_pow = scores.iter().map(|&x| x.powf(p)).sum::<f32>() / n;
    mean_pow.powf(1.0 / p).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::segment::{OffsetInfo, OffsetKind};

    const HOP_S: f64 = 256.0 / 44_100.0; // the shared pYIN / onset hop, ~5.8 ms

    /// A piecewise-constant energy envelope: `levels` gives (until_time_s, rms).
    fn envelope(levels: &[(f64, f32)]) -> Vec<EnergyFrame> {
        let end = levels.last().map(|l| l.0).unwrap_or(0.0);
        let n = (end / HOP_S) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 * HOP_S;
                let rms = levels
                    .iter()
                    .find(|(until, _)| t < *until)
                    .map(|(_, r)| *r)
                    .unwrap_or(0.0);
                EnergyFrame { time_s: t, rms }
            })
            .collect()
    }

    /// A voiced f0 contour: `segments` gives (until_time_s, hz, voicing).
    fn contour(segments: &[(f64, f32, f32)]) -> Vec<F0Frame> {
        let end = segments.last().map(|s| s.0).unwrap_or(0.0);
        let n = (end / HOP_S) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 * HOP_S;
                let (hz, voi) = segments
                    .iter()
                    .find(|(until, _, _)| t < *until)
                    .map(|(_, h, v)| (*h, *v))
                    .unwrap_or((0.0, 0.0));
                F0Frame {
                    time_s: t,
                    frequency_hz: hz.max(0.0),
                    voicing: voi,
                }
            })
            .collect()
    }

    /// A note segment with sensible defaults; override fields per test.
    fn segment(onset_s: f64, offset_s: f64, midi_pitch: u8) -> NoteSegment {
        NoteSegment {
            onset_s,
            offset_s,
            midi_pitch,
            onset_strength: 1.0,
            split_from_plateau: false,
            offset: OffsetInfo {
                kind: OffsetKind::EnergyDecay,
                drop_steepness: 0.9,
            },
        }
    }

    // --- soft_min ---------------------------------------------------------

    #[test]
    fn soft_min_of_equal_scores_is_that_score() {
        assert!((soft_min(&[0.7, 0.7, 0.7, 0.7], -4.0) - 0.7).abs() < 1e-4);
    }

    #[test]
    fn soft_min_is_dominated_by_the_weakest_link() {
        // Three strong dimensions must not mask one weak one: the scalar sits near
        // the minimum, well below the mean (0.7).
        let s = soft_min(&[0.1, 0.9, 0.9, 0.9], -4.0);
        assert!(s < 0.25, "pulled down toward the weak 0.1, got {s}");
        assert!(s >= 0.1, "but not below the true minimum, got {s}");
    }

    #[test]
    fn soft_min_zeroes_on_any_dead_dimension() {
        // A single zero sub-score collapses the whole confidence to zero.
        assert_eq!(soft_min(&[0.0, 0.9, 0.9, 0.9], -4.0), 0.0);
    }

    #[test]
    fn soft_min_sits_above_the_hard_min() {
        // Soft, not brittle: strictly above the true min when the others are up.
        let s = soft_min(&[0.4, 0.8, 0.8, 0.8], -4.0);
        assert!(s > 0.4, "soft-min is above the hard min, got {s}");
    }

    // --- sub-scores -------------------------------------------------------

    #[test]
    fn steady_pitch_scores_high_stability() {
        let seg = segment(0.0, 0.3, 69);
        let c = contour(&[(0.3, 440.0, 0.9)]); // rock-steady A4
        let score = ConfidenceScorer::default().pitch_stability(&seg, &c);
        assert!(score > 0.95, "steady pitch is stable, got {score}");
    }

    #[test]
    fn vibrato_is_tolerated_not_punished() {
        let seg = segment(0.0, 0.4, 69);
        // ±~0.4 semitone ripple around A4 (429..452 Hz), alternating each hop.
        let mut segs = Vec::new();
        let mut t = 0.0;
        let mut up = true;
        while t < 0.4 {
            t += HOP_S;
            segs.push((t, if up { 429.0 } else { 452.0 }, 0.9));
            up = !up;
        }
        let c = contour(&segs);
        let score = ConfidenceScorer::default().pitch_stability(&seg, &c);
        assert!(
            score > 0.6,
            "bounded vibrato stays fairly stable, got {score}"
        );
    }

    #[test]
    fn wandering_pitch_scores_low_stability() {
        let seg = segment(0.0, 0.4, 69);
        // Contour slides across four semitones within the "note" — not a note.
        let c = contour(&[
            (0.1, 440.0, 0.9),
            (0.2, 466.0, 0.9),
            (0.3, 494.0, 0.9),
            (0.4, 523.0, 0.9),
        ]);
        let score = ConfidenceScorer::default().pitch_stability(&seg, &c);
        assert!(score < 0.3, "a wandering contour is unstable, got {score}");
    }

    #[test]
    fn unvoiced_note_scores_zero_stability() {
        let seg = segment(0.0, 0.3, 0);
        let c = contour(&[(0.3, 0.0, 0.0)]); // no voiced frames
        assert_eq!(ConfidenceScorer::default().pitch_stability(&seg, &c), 0.0);
    }

    #[test]
    fn full_level_voiced_note_scores_high_voicing() {
        let seg = segment(0.0, 0.3, 69);
        let c = contour(&[(0.3, 440.0, 0.95)]);
        let env = envelope(&[(0.3, 0.5)]); // note is the stem's peak level
        let score = ConfidenceScorer::default().voicing(&seg, &c, &env, 0.5);
        assert!(score > 0.9, "loud, voiced note scores high, got {score}");
    }

    #[test]
    fn near_floor_note_is_culled_by_the_bleed_guard() {
        // A note whose energy hovers at the noise floor is bleed: however voiced
        // its contour looks, the bleed guard drives voicing to zero.
        let seg = segment(0.0, 0.3, 69);
        let c = contour(&[(0.3, 440.0, 0.9)]); // looks voiced
        let stem_peak = 1.0f32;
        let env = envelope(&[(0.3, 0.05)]); // 5% of peak — below the 10% floor
        let score = ConfidenceScorer::default().voicing(&seg, &c, &env, stem_peak);
        assert!(score < 0.05, "near-floor note is culled, got {score}");
    }

    #[test]
    fn clear_attack_scores_high_onset_clarity() {
        let mut seg = segment(0.0, 0.3, 69);
        seg.onset_strength = 3.0;
        assert!(ConfidenceScorer::default().onset_clarity(&seg) > 0.8);
    }

    #[test]
    fn plateau_split_takes_the_middling_baseline_not_zero() {
        // A split note has no transient (strength 0) but its onset is well located
        // (the pitch change). It scores the baseline, below a clean attack but well
        // above 0 — so the soft-min doesn't bury a real legato note as garbage.
        let mut seg = segment(0.0, 0.3, 69);
        seg.split_from_plateau = true;
        seg.onset_strength = 0.0;
        let scorer = ConfidenceScorer::default();
        let clarity = scorer.onset_clarity(&seg);
        assert_eq!(clarity, scorer.split_onset_clarity);
        assert!(clarity > 0.3, "not zeroed, got {clarity}");
    }

    #[test]
    fn a_legato_note_is_flagged_but_not_buried() {
        // End-to-end: a plateau-split note that is otherwise clean (steady, loud,
        // sharp drop) should score middling — flagged for an uncertain onset, but
        // clearly above the near-zero a bleed note gets, so the heatmap can tell
        // "real note, check the onset" apart from "spurious garbage".
        let mut seg = segment(0.1, 0.4, 69);
        seg.split_from_plateau = true;
        seg.onset_strength = 0.0;
        let c = contour(&[(0.1, 0.0, 0.0), (0.4, 440.0, 0.95)]);
        let env = envelope(&[(0.1, 0.01), (0.4, 0.5), (0.45, 0.01)]);
        let note = ConfidenceScorer::default().score(&seg, &c, &env, 0.5);
        assert!(
            note.confidence > 0.4 && note.confidence < 0.8,
            "flagged but not buried, got {} from {:?}",
            note.confidence,
            note.sub_scores
        );
    }

    #[test]
    fn offset_clarity_follows_drop_steepness() {
        let mut sharp = segment(0.0, 0.3, 69);
        sharp.offset.drop_steepness = 0.95;
        let mut tail = segment(0.0, 0.3, 69);
        tail.offset.drop_steepness = 0.1;
        let scorer = ConfidenceScorer::default();
        assert!(scorer.offset_clarity(&sharp) > 0.9);
        assert!(scorer.offset_clarity(&tail) < 0.2);
    }

    // --- end-to-end scoring (ticket 06 done-criterion) --------------------

    #[test]
    fn a_clean_staccato_note_scores_high() {
        // Loud, steady-pitched, sharp attack, sharp drop → trustworthy note.
        let seg = segment(0.1, 0.4, 69);
        let c = contour(&[(0.1, 0.0, 0.0), (0.4, 440.0, 0.95)]);
        let env = envelope(&[(0.1, 0.01), (0.4, 0.5), (0.45, 0.01)]);
        let note = ConfidenceScorer::default().score(&seg, &c, &env, 0.5);
        assert_eq!(note.midi_pitch, 69);
        assert!(
            note.confidence > 0.75,
            "a clean note is trusted, got {} from {:?}",
            note.confidence,
            note.sub_scores
        );
    }

    #[test]
    fn a_spurious_bleed_note_scores_low() {
        // Ticket 06 done-criterion: a note sitting at the noise floor scores low.
        // Its energy is 4% of the stem peak — the bleed guard zeroes voicing, and
        // the weakest-link soft-min carries that through to the scalar.
        let seg = segment(0.1, 0.4, 69);
        let c = contour(&[(0.1, 0.0, 0.2), (0.4, 440.0, 0.4)]); // weak voicing
        let env = envelope(&[(0.5, 0.04)]); // flat, near-floor throughout
        let stem_peak = 1.0f32;
        let note = ConfidenceScorer::default().score(&seg, &c, &env, stem_peak);
        assert!(
            note.confidence < 0.2,
            "a bleed note is flagged, got {} from {:?}",
            note.confidence,
            note.sub_scores
        );
    }

    #[test]
    fn an_ambiguous_offset_scores_low() {
        // Ticket 06 done-criterion: a note that is clean in every way *except* a
        // mushy, slow-tail offset scores low — the weak offset dimension is not
        // masked by the three strong ones.
        let mut seg = segment(0.1, 0.4, 69);
        seg.offset.drop_steepness = 0.05; // slow tail, ambiguous end
        let c = contour(&[(0.1, 0.0, 0.0), (0.4, 440.0, 0.95)]);
        let env = envelope(&[(0.1, 0.01), (0.4, 0.5), (0.45, 0.45)]);
        let note = ConfidenceScorer::default().score(&seg, &c, &env, 0.5);
        assert!(
            note.sub_scores.offset_clarity < 0.15,
            "offset dimension is low, got {}",
            note.sub_scores.offset_clarity
        );
        assert!(
            note.confidence < 0.3,
            "the ambiguous offset drags the scalar down, got {} from {:?}",
            note.confidence,
            note.sub_scores
        );
    }

    #[test]
    fn score_all_preserves_order_and_fields() {
        let segs = [segment(0.1, 0.3, 69), segment(0.4, 0.6, 72)];
        let c = contour(&[(0.6, 440.0, 0.9)]);
        let env = envelope(&[(0.6, 0.5)]);
        let notes = ConfidenceScorer::default().score_all(&segs, &c, &env);
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].midi_pitch, 69);
        assert_eq!(notes[1].midi_pitch, 72);
        assert!((notes[0].onset_s - 0.1).abs() < 1e-9);
        assert!((notes[1].offset_s - 0.6).abs() < 1e-9);
    }
}
