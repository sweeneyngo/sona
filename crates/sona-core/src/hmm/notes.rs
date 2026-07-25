//! **Decoded path → `NoteSegment`s** (ticket 07, [ADR-0010]) and the [`NoteHmm`]
//! orchestrator that drives the whole stage.
//!
//! [ADR-0010]: ../../../docs/adr/0010-note-hmm-segmentation.md
//!
//! [`NoteHmm::segment`] is the note-HMM's implementation of the
//! [`Segmenter`](crate::segment::Segmenter) seam: it pre-filters the contour
//! (ticket 04), derives a compact pitch range from the stem, builds the model
//! (ticket 05), Viterbi-decodes it with the energy onsets fed in (ticket 06), and
//! turns the winning state path into note events — the work of *this* ticket:
//!
//! - **Boundaries from the path, not the sub-states.** A note is a maximal run of
//!   voiced frames at one integer pitch; it ends at a rest (a `Silent` frame), a
//!   pitch change, or a detected energy onset (a re-articulation). Reading
//!   boundaries this way keeps segmentation robust to how long the decode lingers
//!   in `Attack` vs `Stable`.
//! - **Final pitch decoupled from the decode grid** — an **alpha-trimmed mean** of
//!   the note's voiced contour semitones, rounded to integer MIDI (ADR-0010): the
//!   3-bin grid is for decoding stability, the annotation still emits one integer.
//! - **Post-processing** — merge adjacent same-pitch fragments the decode split by
//!   jitter, prune notes shorter than ~100 ms (literature consensus), and cull
//!   near-floor notes as separation bleed (the note-level echo of ADR-0006's
//!   onset bleed guard, since the HMM — unlike the old segmenter — births notes
//!   from the contour, not only from detected onsets).
//! - **Offsets** fall out of the path (a note ends where it enters silence or the
//!   next note attacks); the **energy-drop steepness** is still measured there,
//!   purely as confidence raw material (ADR-0007's one surviving output).

use crate::annotation::{EnergyFrame, F0Frame};
use crate::onset::Onset;
use crate::prefilter::{PitchFrame, PitchPreFilter};
use crate::segment::{drop_steepness, midi_round_f32, NoteSegment, OffsetInfo, OffsetKind};

use super::decode::{viterbi_path, DecodeInput};
use super::model::{HmmModel, HmmParams, ObsFrame, Sub};

/// The note-HMM segmentation strategy (ADR-0010): the default behind
/// [`Segmenter`](crate::segment::Segmenter). Holds the model parameters plus the
/// post-processing thresholds; construct with [`NoteHmm::default`] or set fields
/// to tune (ticket 08 sweeps [`params`](Self::params)).
#[derive(Debug, Clone)]
pub struct NoteHmm {
    /// Model + decode parameters (states, emission, transitions, onset bonus).
    pub params: HmmParams,
    /// Pre-filter applied to the contour before decoding (ticket 04).
    pub prefilter: PitchPreFilter,
    /// Alpha-trim fraction for the final per-note pitch (each tail trimmed by this
    /// fraction before averaging). Guards the mean against onset/offset glides.
    pub alpha_trim: f32,
    /// Notes shorter than this (seconds) are pruned — the ~100 ms literature floor.
    pub min_note_s: f64,
    /// A note whose peak RMS is below this fraction of the stem's peak RMS is
    /// culled as separation bleed. Mirrors the onset stage's bleed floor (ADR-0006).
    pub bleed_floor_frac: f32,
    /// Semitones of headroom added above/below the observed pitch span when
    /// sizing the decode grid, so the interval prior has room to reject an
    /// out-of-range octave artefact.
    pub range_pad_semitones: u8,
    /// Half-window (seconds) for the offset energy-drop steepness measurement.
    pub drop_window_s: f64,
    /// A detected onset is treated as this note's own attack when it lands within
    /// this many seconds of the note's start; otherwise the note is pitch-born
    /// (a legato split, `split_from_plateau = true`).
    pub onset_match_s: f64,
}

impl Default for NoteHmm {
    fn default() -> Self {
        Self {
            params: HmmParams::default(),
            prefilter: PitchPreFilter::default(),
            alpha_trim: 0.1,
            min_note_s: 0.1,
            bleed_floor_frac: 0.05,
            range_pad_semitones: 2,
            drop_window_s: 0.04,
            onset_match_s: 0.03,
        }
    }
}

impl NoteHmm {
    /// Segment a stem into note events (the [`Segmenter`](crate::segment::Segmenter)
    /// contract). Returns an empty vec when the contour carries no pitched frame
    /// (silence): the note-HMM births notes from the contour, so with no pitch
    /// there is nothing to decode.
    pub fn segment(
        &self,
        onsets: &[Onset],
        contour: &[F0Frame],
        envelope: &[EnergyFrame],
    ) -> Vec<NoteSegment> {
        let frames = self.prefilter.run(contour);
        let Some((min_midi, max_midi)) = self.pitch_range(&frames) else {
            return Vec::new();
        };

        let model = HmmModel::new(self.params, min_midi, max_midi);
        let input = DecodeInput {
            obs: frames.iter().map(observe).collect(),
            onset: onset_flags(&frames, onsets, self.onset_match_s),
        };
        let path = viterbi_path(&model, &input);

        let onset_strength = onset_strength_by_frame(&frames, onsets, self.onset_match_s);
        let raw = self.notes_from_path(&model, &path, &frames, &input.onset, &onset_strength);
        self.post_process(raw, envelope)
    }

    /// The inclusive integer-MIDI range to build the model over: the observed
    /// voiced-pitch span, padded and clamped to `0..=127`. `None` if no frame is
    /// pitched (silence → no notes).
    fn pitch_range(&self, frames: &[PitchFrame]) -> Option<(u8, u8)> {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for s in frames.iter().filter_map(|f| f.semitone) {
            lo = lo.min(s);
            hi = hi.max(s);
        }
        if !lo.is_finite() {
            return None;
        }
        let pad = self.range_pad_semitones as f32;
        let min_midi = (lo.floor() - pad).clamp(0.0, 127.0) as u8;
        let max_midi = (hi.ceil() + pad).clamp(0.0, 127.0) as u8;
        Some((min_midi, max_midi.max(min_midi)))
    }

    /// Walk the decoded state path into raw note runs. A note starts on a voiced
    /// frame that opens a rest, a pitch change, or a detected onset; it runs until
    /// the next such boundary or a rest.
    fn notes_from_path(
        &self,
        model: &HmmModel,
        path: &[usize],
        frames: &[PitchFrame],
        onset: &[bool],
        onset_strength: &[Option<f32>],
    ) -> Vec<RawNote> {
        // Per-frame: the note's integer pitch if voiced, else None (a rest).
        let voiced_midi: Vec<Option<i32>> = path
            .iter()
            .map(|&s| {
                let (bin, sub) = model.split(s);
                match sub {
                    Sub::Silent => None,
                    _ => Some(model.bin_center(bin).round() as i32),
                }
            })
            .collect();

        let n = path.len();
        let mut notes = Vec::new();
        let mut t = 0;
        while t < n {
            let Some(midi) = voiced_midi[t] else {
                t += 1;
                continue;
            };
            let start = t;
            t += 1;
            // Extend while still voiced at the same integer pitch and no new onset
            // forces a re-articulation.
            while t < n && voiced_midi[t] == Some(midi) && !onset[t] {
                t += 1;
            }
            let end = t; // first frame past this note (rest / pitch change / onset / EOF)

            // Born of a detected onset near its start, or a pitch-born split?
            let near_onset = onset_strength[start.min(onset_strength.len() - 1)];
            let (strength, split_from_plateau) = match near_onset {
                Some(s) => (s, false),
                None => (0.0, true),
            };

            let midi_pitch = self.final_pitch(frames, start, end, model, path);
            let stats = hmm_stats(frames, start, end, model, path);

            notes.push(RawNote {
                onset_s: frames[start].time_s,
                offset_s: frames[end.min(n - 1)].time_s,
                ends_in_rest: end < n && voiced_midi[end].is_none(),
                at_stem_end: end >= n,
                midi_pitch,
                onset_strength: strength,
                split_from_plateau,
                stats,
            });
        }
        notes
    }

    /// Alpha-trimmed mean of the note's voiced contour semitones, rounded to
    /// integer MIDI. Falls back to the decoded bin's centre pitch when the note
    /// somehow has no pitched contour frame.
    fn final_pitch(
        &self,
        frames: &[PitchFrame],
        start: usize,
        end: usize,
        model: &HmmModel,
        path: &[usize],
    ) -> u8 {
        let mut semis: Vec<f32> = frames[start..end].iter().filter_map(|f| f.semitone).collect();
        if semis.is_empty() {
            let (bin, _) = model.split(path[start]);
            return midi_round_f32(model.bin_center(bin));
        }
        semis.sort_by(|a, b| a.partial_cmp(b).expect("finite semitones"));
        let trim = ((semis.len() as f32) * self.alpha_trim).floor() as usize;
        // Trim `trim` from each tail; fall back to the full set if that would
        // leave nothing (very short notes).
        let kept = if 2 * trim < semis.len() {
            &semis[trim..semis.len() - trim]
        } else {
            &semis[..]
        };
        let mean = kept.iter().sum::<f32>() / kept.len() as f32;
        midi_round_f32(mean)
    }

    /// Merge jitter-split fragments, cull bleed, prune short notes, and stamp each
    /// offset's kind + drop-steepness.
    fn post_process(&self, notes: Vec<RawNote>, envelope: &[EnergyFrame]) -> Vec<NoteSegment> {
        let merged = self.merge_same_pitch(notes);
        let stem_peak_rms = envelope.iter().map(|f| f.rms).fold(0.0f32, f32::max);
        let bleed_floor = stem_peak_rms * self.bleed_floor_frac;

        // Keep notes that clear both the duration floor and the bleed floor.
        let kept: Vec<RawNote> = merged
            .into_iter()
            .filter(|n| n.offset_s - n.onset_s >= self.min_note_s)
            .filter(|n| note_peak_rms(envelope, n) > bleed_floor)
            .collect();

        // Stamp the offset kind now that the surviving neighbours are known.
        kept.iter()
            .enumerate()
            .map(|(i, n)| {
                let next = kept.get(i + 1);
                let kind = offset_kind(n, next);
                NoteSegment {
                    onset_s: n.onset_s,
                    offset_s: n.offset_s,
                    midi_pitch: n.midi_pitch,
                    onset_strength: n.onset_strength,
                    split_from_plateau: n.split_from_plateau,
                    offset: OffsetInfo {
                        kind,
                        drop_steepness: self.offset_drop_steepness(envelope, n.offset_s),
                    },
                    pitch_dispersion_semitones: n.stats.pitch_dispersion(),
                    voiced_confidence: n.stats.voiced_confidence(),
                }
            })
            .collect()
    }

    /// Steepest energy drop near a note's offset. The HMM closes a note where its
    /// pitch/voicing ends, which — because pYIN's analysis window smears the
    /// voicing edge — can precede the actual energy fall by a frame or two. So
    /// rather than measure at one instant, probe forward a little and take the
    /// steepest drop: a staccato note finds its sharp fall, while a legato offset
    /// (steady energy across a pitch change) still measures near zero.
    fn offset_drop_steepness(&self, envelope: &[EnergyFrame], offset_s: f64) -> f32 {
        let step = self.drop_window_s / 4.0; // ~4 probes per window
        let mut best = 0.0f32;
        let mut t = offset_s;
        while t <= offset_s + 2.0 * self.drop_window_s {
            best = best.max(drop_steepness(envelope, t, self.drop_window_s));
            t += step.max(f64::MIN_POSITIVE);
        }
        best
    }

    /// Fold each pitch-born fragment into the previous note when they share a
    /// pitch and are seamless (no rest between). This cleans up a held note the
    /// decode briefly split by pitch jitter, while leaving genuinely
    /// re-articulated same-pitch notes — which carry their own detected onset —
    /// separate.
    fn merge_same_pitch(&self, notes: Vec<RawNote>) -> Vec<RawNote> {
        let mut out: Vec<RawNote> = Vec::with_capacity(notes.len());
        for n in notes {
            if let Some(prev) = out.last_mut() {
                let seamless = (n.onset_s - prev.offset_s).abs() < 1e-6;
                if seamless && n.midi_pitch == prev.midi_pitch && n.split_from_plateau {
                    // Absorb: extend the previous note over this fragment and fold
                    // in its HMM confidence accumulators.
                    prev.offset_s = n.offset_s;
                    prev.ends_in_rest = n.ends_in_rest;
                    prev.at_stem_end = n.at_stem_end;
                    prev.stats.absorb(&n.stats);
                    continue;
                }
            }
            out.push(n);
        }
        out
    }
}

/// A note run before post-processing: frame span, timing, pitch, and the raw
/// material for its onset/offset classification.
#[derive(Debug, Clone)]
struct RawNote {
    onset_s: f64,
    offset_s: f64,
    /// The frame past the note is a rest (`Silent`).
    ends_in_rest: bool,
    /// The note runs to the end of the stem.
    at_stem_end: bool,
    midi_pitch: u8,
    onset_strength: f32,
    split_from_plateau: bool,
    /// HMM-derived confidence accumulators (ticket 09), kept additive so a merge
    /// can combine two fragments' stats exactly.
    stats: HmmStats,
}

/// Pitch dispersion reported for a note with no pitched frame at all: an octave,
/// large enough that confidence's pitch-stability falloff bottoms out. A sentinel
/// on the real measurement channel, kept far outside any plausible real spread.
const NO_PITCH_DISPERSION_SEMITONES: f32 = 12.0;

/// Additive accumulators for a note's HMM-derived confidence raw material, so
/// merged fragments combine exactly (sum the parts, finalise once). Pitch
/// dispersion is accumulated as sum/sum-of-squares so it can be finalised as the
/// spread *about the note's own mean* — free of any semitone-rounding bias.
#[derive(Debug, Clone, Copy, Default)]
struct HmmStats {
    /// Σ and Σ² of the note's voiced-frame semitones, and their count → RMS
    /// dispersion about the mean.
    semi_sum: f32,
    semi_sumsq: f32,
    semi_n: usize,
    /// Σ of the per-frame voiced-vs-silent emission evidence over the note's
    /// frames, and their count → mean voiced confidence.
    voiced_sum: f32,
    voiced_n: usize,
}

impl HmmStats {
    /// Fold another fragment's accumulators in (used when merging same-pitch runs).
    fn absorb(&mut self, other: &HmmStats) {
        self.semi_sum += other.semi_sum;
        self.semi_sumsq += other.semi_sumsq;
        self.semi_n += other.semi_n;
        self.voiced_sum += other.voiced_sum;
        self.voiced_n += other.voiced_n;
    }

    /// RMS semitone dispersion about the note's mean pitch (how much its contour
    /// wandered). A note with no pitched frame is maximally uncertain
    /// ([`NO_PITCH_DISPERSION_SEMITONES`]).
    fn pitch_dispersion(&self) -> f32 {
        if self.semi_n == 0 {
            return NO_PITCH_DISPERSION_SEMITONES;
        }
        let n = self.semi_n as f32;
        let mean = self.semi_sum / n;
        (self.semi_sumsq / n - mean * mean).max(0.0).sqrt()
    }

    /// Mean voiced-vs-silent emission evidence over the note's frames (`0` if
    /// empty).
    fn voiced_confidence(&self) -> f32 {
        if self.voiced_n == 0 {
            0.0
        } else {
            (self.voiced_sum / self.voiced_n as f32).clamp(0.0, 1.0)
        }
    }
}

/// The HMM-derived confidence accumulators for one note's frame span `[start,
/// end)`: the voiced-frame semitones (→ pitch dispersion about the mean) and the
/// model's per-frame voiced-vs-silent emission evidence (→ voicing confidence).
fn hmm_stats(
    frames: &[PitchFrame],
    start: usize,
    end: usize,
    model: &HmmModel,
    path: &[usize],
) -> HmmStats {
    let mut s = HmmStats::default();
    for t in start..end {
        let (bin, _) = model.split(path[t]);
        s.voiced_sum += sigmoid(model.voiced_logodds(observe(&frames[t]), bin));
        s.voiced_n += 1;
        if let Some(semitone) = frames[t].semitone {
            s.semi_sum += semitone;
            s.semi_sumsq += semitone * semitone;
            s.semi_n += 1;
        }
    }
    s
}

/// Logistic squashing of a log-odds into a `0.0..=1.0` probability.
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// One pre-filtered frame as the emission model reads it.
fn observe(f: &PitchFrame) -> ObsFrame {
    ObsFrame {
        semitone: f.semitone,
        voicing: f.voicing,
    }
}

/// Per-frame boolean: did a detected onset land within `match_s` of this frame?
fn onset_flags(frames: &[PitchFrame], onsets: &[Onset], match_s: f64) -> Vec<bool> {
    onset_strength_by_frame(frames, onsets, match_s)
        .iter()
        .map(Option::is_some)
        .collect()
}

/// Per-frame onset strength: for each frame, the strength of the nearest detected
/// onset if one lands within `match_s`, else `None`. Each onset is assigned to its
/// single nearest frame so one onset marks one frame.
fn onset_strength_by_frame(
    frames: &[PitchFrame],
    onsets: &[Onset],
    match_s: f64,
) -> Vec<Option<f32>> {
    let mut out = vec![None; frames.len()];
    if frames.is_empty() {
        return out;
    }
    for o in onsets {
        // Nearest frame to this onset time.
        let mut best = 0usize;
        let mut best_d = f64::INFINITY;
        for (i, f) in frames.iter().enumerate() {
            let d = (f.time_s - o.time_s).abs();
            if d < best_d {
                best_d = d;
                best = i;
            }
        }
        if best_d <= match_s {
            // Keep the stronger onset if two map to one frame.
            match out[best] {
                Some(s) if s >= o.strength => {}
                _ => out[best] = Some(o.strength),
            }
        }
    }
    out
}

/// Peak RMS of the envelope within a note's `[onset_s, offset_s]` span.
fn note_peak_rms(envelope: &[EnergyFrame], n: &RawNote) -> f32 {
    envelope
        .iter()
        .filter(|f| f.time_s >= n.onset_s && f.time_s <= n.offset_s)
        .map(|f| f.rms)
        .fold(0.0f32, f32::max)
}

/// Classify how a note's offset was decided, from the path context and its
/// successor. The confidence stage reads only the drop-steepness, not this kind
/// (ADR-0007), so it is for the UI/debug; the vocabulary is inherited from the
/// retired heuristic so saved annotations stay legible.
fn offset_kind(n: &RawNote, next: Option<&RawNote>) -> OffsetKind {
    if n.at_stem_end {
        OffsetKind::StemEnd
    } else if n.ends_in_rest {
        // Entered silence before the next note: an energy decay into a rest.
        OffsetKind::EnergyDecay
    } else {
        // Seamlessly followed by another note: a legato pitch change (the next is
        // pitch-born) reads as a plateau split; a re-articulation with its own
        // detected onset reads as a next-onset boundary.
        match next {
            Some(nx) if nx.split_from_plateau => OffsetKind::PlateauSplit,
            Some(_) => OffsetKind::NextOnset,
            None => OffsetKind::StemEnd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP_S: f64 = 256.0 / 44_100.0;

    /// Build a contour at the shared hop from `(until_s, hz, voicing)` segments;
    /// `hz <= 0` marks unvoiced.
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

    /// A matching energy envelope from `(until_s, rms)` segments.
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

    fn onset(t: f64) -> Onset {
        Onset { time_s: t, strength: 1.0 }
    }

    #[test]
    fn silence_yields_no_notes() {
        let c = contour(&[(0.5, 0.0, 0.0)]);
        let e = envelope(&[(0.5, 0.0)]);
        assert!(NoteHmm::default().segment(&[], &c, &e).is_empty());
    }

    #[test]
    fn one_held_note_is_a_single_segment() {
        // A held A4 over a clear energy plateau between rests.
        let c = contour(&[(0.1, 0.0, 0.0), (0.6, 440.0, 0.95), (0.75, 0.0, 0.0)]);
        let e = envelope(&[(0.1, 0.01), (0.6, 0.5), (0.75, 0.01)]);
        let notes = NoteHmm::default().segment(&[onset(0.1)], &c, &e);
        assert_eq!(notes.len(), 1, "one held note: {notes:?}");
        assert_eq!(notes[0].midi_pitch, 69);
        assert!(notes[0].onset_strength > 0.0, "born of the onset");
        assert!(!notes[0].split_from_plateau);
    }

    #[test]
    fn a_legato_pitch_change_splits_into_two_notes() {
        // A4 then C5 under one breath, energy steady, one onset at the start.
        let c = contour(&[(0.1, 0.0, 0.0), (0.45, 440.0, 0.95), (0.8, 523.25, 0.95), (0.9, 0.0, 0.0)]);
        let e = envelope(&[(0.1, 0.01), (0.8, 0.5), (0.9, 0.01)]);
        let notes = NoteHmm::default().segment(&[onset(0.1)], &c, &e);
        assert_eq!(notes.len(), 2, "legato split: {notes:?}");
        assert_eq!(notes[0].midi_pitch, 69);
        assert_eq!(notes[1].midi_pitch, 72);
        // The second note is pitch-born (no onset of its own), the first is not.
        assert!(!notes[0].split_from_plateau);
        assert!(notes[1].split_from_plateau);
        assert_eq!(notes[0].offset.kind, OffsetKind::PlateauSplit);
        assert!((notes[0].offset_s - notes[1].onset_s).abs() < 1e-6, "seamless");
    }

    #[test]
    fn a_short_blip_is_pruned() {
        // A 40 ms voiced blip (< the 100 ms floor) between rests is dropped.
        let c = contour(&[(0.2, 0.0, 0.0), (0.24, 440.0, 0.95), (0.5, 0.0, 0.0)]);
        let e = envelope(&[(0.2, 0.01), (0.24, 0.5), (0.5, 0.01)]);
        let notes = NoteHmm::default().segment(&[onset(0.2)], &c, &e);
        assert!(notes.is_empty(), "sub-100ms note pruned: {notes:?}");
    }

    #[test]
    fn hmm_confidence_fields_reflect_pitch_and_voicing() {
        // A clean held A4 should report low pitch dispersion (it sat on pitch) and
        // high voiced confidence (the model was sure it was voiced).
        let c = contour(&[(0.1, 0.0, 0.0), (0.6, 440.0, 0.95), (0.75, 0.0, 0.0)]);
        let e = envelope(&[(0.1, 0.01), (0.6, 0.5), (0.75, 0.01)]);
        let notes = NoteHmm::default().segment(&[onset(0.1)], &c, &e);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(
            notes[0].pitch_dispersion_semitones < 0.3,
            "steady note has low dispersion, got {}",
            notes[0].pitch_dispersion_semitones
        );
        assert!(
            notes[0].voiced_confidence > 0.8,
            "confidently voiced, got {}",
            notes[0].voiced_confidence
        );
    }

    #[test]
    fn a_near_floor_note_is_culled_as_bleed() {
        // A real loud A4, then a voiced-but-near-floor "note" at 3% of the peak
        // level: the second is bleed and must not survive as a segment.
        let c = contour(&[
            (0.1, 0.0, 0.0),
            (0.5, 440.0, 0.95),
            (0.6, 0.0, 0.0),
            (1.0, 523.25, 0.9), // voiced contour...
            (1.1, 0.0, 0.0),
        ]);
        let e = envelope(&[
            (0.1, 0.01),
            (0.5, 0.5), // loud note
            (0.6, 0.01),
            (1.0, 0.015), // ~3% of peak: bleed level
            (1.1, 0.01),
        ]);
        let notes = NoteHmm::default().segment(&[onset(0.1)], &c, &e);
        assert_eq!(notes.len(), 1, "only the loud note survives: {notes:?}");
        assert_eq!(notes[0].midi_pitch, 69);
    }
}
