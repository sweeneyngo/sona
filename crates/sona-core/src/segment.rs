//! **Note segmentation** — the third stage of the perception pipeline (spec
//! §Pipeline 3), landed by ticket 05. This is the spec's highest-error area, so
//! the effort concentrates here.
//!
//! The stage turns three raw-perception tracks — the f0 contour ([`F0Frame`]s,
//! ticket 03), the [`Onset`]s and the energy/RMS envelope ([`EnergyFrame`]s,
//! ticket 04) — into [`NoteSegment`]s: note events with onset/offset in raw
//! seconds (no quantization — that's the UI's job, ADR-0004) plus the raw
//! material the confidence stage (ticket 06) reads.
//!
//! ## Offsets are hybrid (spec stage 3, ADR-0007)
//!
//! Each onset opens a note; where it *closes* is the hard part. Two candidates
//! compete and the **earlier** one wins:
//!
//! - **Next-onset** — when the next onset arrives within a musical threshold
//!   ([`Segmenter::max_offset_gap_s`]), it is the primary offset: back-to-back
//!   notes end where the next one attacks.
//! - **Energy-decay fallback** — when the energy has already decayed to a rest
//!   before then, the note closes there instead ([`Segmenter::rest_floor_frac`]),
//!   leaving a rest until the next onset. This is the staccato-Vocaloid common
//!   case: short note, clean gap, sharp drop.
//!
//! ## Pitch-plateau splitting (spec stage 3)
//!
//! Onsets are energy transients, so a *legato* pitch change at steady energy —
//! a slide, or two slurred notes — carries no onset (ADR-0006 left it here on
//! purpose). Within each onset's span the f0 contour is cut into runs of one
//! MIDI pitch; a run sustained past [`Segmenter::min_plateau_s`] becomes its own
//! note, while shorter excursions (vibrato swings, slide transients) are absorbed
//! so they don't over-split.
//!
//! ## Raw material for confidence (ticket 06)
//!
//! Every [`NoteSegment`] carries what the confidence stage can't recover later:
//! the onset [`strength`](NoteSegment::onset_strength) (onset clarity), and per
//! offset the [`drop_steepness`](OffsetInfo::drop_steepness) and how the offset
//! was decided ([`OffsetKind`]) (offset clarity). Pitch stability and voicing it
//! recomputes from the contour it already has.

use crate::annotation::{EnergyFrame, F0Frame};
use crate::onset::Onset;

/// How a note's offset was decided. Recorded per offset so the confidence stage
/// (ticket 06) and the UI can tell *why* a note ends where it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetKind {
    /// Closed by the next onset arriving within the musical threshold
    /// ([`Segmenter::max_offset_gap_s`]): back-to-back notes.
    NextOnset,
    /// Closed by the energy decaying to a rest before the next onset — the
    /// staccato fallback ([`Segmenter::rest_floor_frac`]).
    EnergyDecay,
    /// Closed by a sustained pitch change with no new onset (legato / slide):
    /// a pitch-plateau split. Energy is steady across it, so its steepness is low.
    PlateauSplit,
    /// Closed by the end of the stem (the last note, energy never decayed).
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
    /// MIDI pitch number (0–127): the note's f0 plateau — per-frame frequencies
    /// rounded to the nearest semitone and grouped into one sustained pitch.
    pub midi_pitch: u8,
    /// The onset transient's novelty (from the [`Onset`]), carried through for
    /// onset-clarity confidence. `0.0` for a note born of a plateau split, which
    /// has no transient of its own.
    pub onset_strength: f32,
    /// Whether this note began at a pitch-plateau split rather than an onset
    /// (legato / slide). Its onset time is a pitch change, not an attack.
    pub split_from_plateau: bool,
    /// How this note's offset was decided, and its energy-drop steepness.
    pub offset: OffsetInfo,
}

/// Note segmenter (ADR-0007). Construct with [`Segmenter::default`] for
/// staccato-vocal defaults, or set fields to tune. All thresholds are empirical
/// — the spec expects them tuned against real Vocaloid stems.
#[derive(Debug, Clone)]
pub struct Segmenter {
    /// A next onset this many seconds or less after the current onset is the
    /// primary (next-onset) offset; a larger gap means a rest, so the
    /// energy-decay fallback closes the note earlier instead.
    pub max_offset_gap_s: f64,
    /// Energy-decay rest threshold as a fraction of the stem's peak RMS. Below
    /// this the note is considered to have ended (into a rest). Peak-relative, so
    /// it tracks the stem's level like the onset stage's bleed floor (ADR-0006).
    pub rest_floor_frac: f32,
    /// Minimum note duration in seconds. The decay search starts this far past
    /// the onset so a note is never closed inside its own attack.
    pub min_note_s: f64,
    /// Minimum duration in seconds for a pitch plateau to become its own note.
    /// Shorter excursions (a vibrato swing crossing a semitone, a slide passing
    /// through a pitch) are absorbed rather than split off.
    pub min_plateau_s: f64,
    /// Half-window in seconds for the energy-drop steepness measured at an
    /// offset. Short, so it reads the *rate* of the drop, not its eventual depth.
    pub drop_window_s: f64,
    /// A contour frame counts toward a note's pitch when its voicing reaches this
    /// (in `0.0..=1.0`) and its frequency is positive.
    pub voiced_threshold: f32,
}

impl Default for Segmenter {
    fn default() -> Self {
        // Defaults retuned for general (expressive) vocals rather than staccato
        // Vocaloid. The big one is `rest_floor_frac`: at the old 0.15 a note
        // closed as soon as its energy dipped to 15% of the peak, so sustained /
        // legato singing got chopped and the tail of each phrase (until the next
        // detected onset) produced no notes. Only near-silence should close a
        // note now, so plateau splitting can cover the whole phrase.
        Self {
            max_offset_gap_s: 0.18,
            // Only a genuine rest closes a note now — a moderate mid-phrase dip
            // no longer ends it (was 0.15). Not lower than this, or low-level
            // tonal bleed starts leaking in as spurious notes.
            rest_floor_frac: 0.08,
            min_note_s: 0.05,
            min_plateau_s: 0.08,
            drop_window_s: 0.04,
            // Count slightly weaker-voiced frames toward a pitch, since real vocal
            // voicing confidence runs lower than a clean synth's (was 0.5).
            voiced_threshold: 0.4,
        }
    }
}

/// Continuous MIDI pitch of a frequency in Hz (69 = A4 = 440 Hz). Fractional —
/// callers round when they need a discrete pitch.
pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// A run of contour frames the plateau step has grouped under one MIDI pitch.
#[derive(Debug, Clone, Copy)]
struct Plateau {
    pitch: u8,
    start_s: f64,
    end_s: f64,
}

impl Plateau {
    fn duration(&self) -> f64 {
        self.end_s - self.start_s
    }
}

impl Segmenter {
    /// Segment a stem's `onsets`, f0 `contour`, and energy `envelope` into note
    /// events. Returns an empty vec if there are no onsets or no envelope to
    /// close notes against. Notes come out in onset order.
    pub fn segment(
        &self,
        onsets: &[Onset],
        contour: &[F0Frame],
        envelope: &[EnergyFrame],
    ) -> Vec<NoteSegment> {
        if onsets.is_empty() || envelope.is_empty() {
            return Vec::new();
        }

        let stem_end_s = envelope
            .last()
            .map(|f| f.time_s)
            .unwrap_or(0.0)
            .max(onsets.last().map(|o| o.time_s).unwrap_or(0.0));
        let peak_rms = envelope.iter().map(|f| f.rms).fold(0.0f32, f32::max);
        let rest_floor = peak_rms * self.rest_floor_frac;

        let mut notes = Vec::new();
        for (i, onset) in onsets.iter().enumerate() {
            let t0 = onset.time_s;
            let next_onset_s = onsets.get(i + 1).map(|o| o.time_s);
            // Upper bound on this note's span: it can never run past the next
            // onset, and never past the stem.
            let span_limit = next_onset_s.unwrap_or(stem_end_s).min(stem_end_s);
            if span_limit <= t0 {
                continue; // degenerate (coincident onsets / onset past the stem)
            }

            let (span_offset_s, span_kind) =
                self.resolve_offset(t0, next_onset_s, span_limit, envelope, rest_floor);

            // Cut the [t0, span_offset_s] span into pitch plateaus; each becomes a
            // note. The last plateau inherits the span's offset; earlier ones end
            // at a plateau split.
            let plateaus = self.split_plateaus(contour, t0, span_offset_s);
            let plateau_count = plateaus.len();
            for (p_idx, plateau) in plateaus.into_iter().enumerate() {
                let is_last = p_idx + 1 == plateau_count;
                let (kind, offset_end) = if is_last {
                    (span_kind, span_offset_s)
                } else {
                    (OffsetKind::PlateauSplit, plateau.end_s)
                };
                let drop_steepness = self.drop_steepness(envelope, offset_end);
                notes.push(NoteSegment {
                    onset_s: plateau.start_s,
                    offset_s: plateau.end_s,
                    midi_pitch: plateau.pitch,
                    onset_strength: if p_idx == 0 { onset.strength } else { 0.0 },
                    split_from_plateau: p_idx > 0,
                    offset: OffsetInfo {
                        kind,
                        drop_steepness,
                    },
                });
            }
        }
        notes
    }

    /// Decide where a note that started at `t0` closes, given the `next_onset_s`
    /// (if any) and the `span_limit` (next onset or stem end). Returns the offset
    /// time and how it was decided. The energy-decay candidate and the
    /// within-threshold next-onset candidate compete; the earlier one wins.
    fn resolve_offset(
        &self,
        t0: f64,
        next_onset_s: Option<f64>,
        span_limit: f64,
        envelope: &[EnergyFrame],
        rest_floor: f32,
    ) -> (f64, OffsetKind) {
        // Next-onset candidate: eligible only when the next onset is close enough
        // to be this note's natural end rather than a rest away.
        let next_onset_candidate = next_onset_s
            .filter(|&t_next| t_next - t0 <= self.max_offset_gap_s)
            .map(|t_next| (t_next, OffsetKind::NextOnset));

        // Energy-decay candidate: the first frame past the minimum note duration
        // whose RMS has fallen to the rest floor, bounded by the span.
        let decay_candidate = self
            .energy_decay_offset(envelope, t0, span_limit, rest_floor)
            .map(|t| (t, OffsetKind::EnergyDecay));

        match (next_onset_candidate, decay_candidate) {
            (Some(n), Some(d)) => {
                if d.0 <= n.0 {
                    d
                } else {
                    n
                }
            }
            (Some(n), None) => n,
            (None, Some(d)) => d,
            // Neither fired: the note runs to the span limit. If a next onset
            // exists (a long held note past the threshold whose energy never
            // decayed), that onset is still the honest boundary; otherwise it is
            // a trailing note closing at the stem end.
            (None, None) => (
                span_limit,
                if next_onset_s.is_some() {
                    OffsetKind::NextOnset
                } else {
                    OffsetKind::StemEnd
                },
            ),
        }
    }

    /// First envelope time at or after `t0 + min_note_s` (and before `limit`)
    /// where the RMS has fallen to `rest_floor`. `None` if it never does within
    /// the span (the note is still sounding at the limit).
    fn energy_decay_offset(
        &self,
        envelope: &[EnergyFrame],
        t0: f64,
        limit: f64,
        rest_floor: f32,
    ) -> Option<f64> {
        let search_start = t0 + self.min_note_s;
        envelope
            .iter()
            .find(|f| f.time_s >= search_start && f.time_s < limit && f.rms <= rest_floor)
            .map(|f| f.time_s)
    }

    /// Fractional fall in RMS across `offset_s`: the mean RMS in the window just
    /// after minus that just before, over the before-level, clamped to
    /// `0.0..=1.0`. A short window makes this a *rate* — a staccato drop to the
    /// floor scores near 1, a slow tail near 0. Returns 0 when there is no energy
    /// before the offset to fall from.
    fn drop_steepness(&self, envelope: &[EnergyFrame], offset_s: f64) -> f32 {
        let w = self.drop_window_s;
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
        let before = mean_rms(offset_s - w, offset_s);
        let after = mean_rms(offset_s, offset_s + w);
        match (before, after) {
            (Some(b), Some(a)) if b > f32::EPSILON => (1.0 - a / b).clamp(0.0, 1.0),
            // No "before" energy (offset at/near the stem start) — nothing to
            // fall from; treat as no measurable drop.
            _ => 0.0,
        }
    }

    /// Cut the contour between `t_start` and `t_end` into pitch plateaus. Voiced
    /// frames are quantized to a MIDI pitch and grouped into runs; runs shorter
    /// than `min_plateau_s` are absorbed into a neighbour so vibrato and slide
    /// transients don't over-split. The surviving runs tile `[t_start, t_end]`.
    ///
    /// A span with no voiced frames yields a single plateau of pitch 0 spanning
    /// the whole span, so the onset still produces a note (the confidence stage's
    /// voicing/bleed guard, ticket 06, is what culls a spurious unvoiced note —
    /// segmentation does not silently drop onsets).
    fn split_plateaus(&self, contour: &[F0Frame], t_start: f64, t_end: f64) -> Vec<Plateau> {
        // Voiced frames within the span, as (time, midi_pitch).
        let mut framed: Vec<(f64, u8)> = contour
            .iter()
            .filter(|f| {
                f.time_s >= t_start
                    && f.time_s < t_end
                    && f.voicing >= self.voiced_threshold
                    && f.frequency_hz > 0.0
            })
            .map(|f| (f.time_s, midi_round(f.frequency_hz)))
            .collect();
        framed.sort_by(|a, b| a.0.partial_cmp(&b.0).expect("finite frame times"));

        if framed.is_empty() {
            return vec![Plateau {
                pitch: 0,
                start_s: t_start,
                end_s: t_end,
            }];
        }

        // Build raw runs of equal pitch. A run spans from its first frame to the
        // next run's first frame; the first run is snapped back to t_start and the
        // last forward to t_end so the plateaus tile the whole span with no gaps.
        let mut runs: Vec<Plateau> = Vec::new();
        for (idx, &(time, pitch)) in framed.iter().enumerate() {
            let start_s = if idx == 0 { t_start } else { time };
            match runs.last_mut() {
                Some(last) if last.pitch == pitch => {}
                _ => runs.push(Plateau {
                    pitch,
                    start_s,
                    end_s: t_end, // provisional; fixed up below
                }),
            }
        }
        // Fix run ends: each run ends where the next begins.
        for k in 0..runs.len().saturating_sub(1) {
            runs[k].end_s = runs[k + 1].start_s;
        }
        if let Some(last) = runs.last_mut() {
            last.end_s = t_end;
        }

        self.absorb_short_plateaus(runs)
    }

    /// Repeatedly fold the shortest sub-threshold plateau into a neighbour (the
    /// one closer in pitch; ties go to the longer), coalescing equal-pitch
    /// neighbours, until every plateau lasts at least `min_plateau_s` (or one
    /// remains). This is what keeps vibrato and slide transients from splitting a
    /// held note into a stutter of semitone fragments.
    fn absorb_short_plateaus(&self, mut runs: Vec<Plateau>) -> Vec<Plateau> {
        loop {
            if runs.len() <= 1 {
                break;
            }
            // Shortest plateau still under threshold.
            let Some((i, _)) = runs
                .iter()
                .enumerate()
                .filter(|(_, r)| r.duration() < self.min_plateau_s)
                .min_by(|a, b| {
                    a.1.duration()
                        .partial_cmp(&b.1.duration())
                        .expect("finite durations")
                })
            else {
                break;
            };

            // Fold run i into the neighbour closest in pitch (tie → longer one).
            let left = i.checked_sub(1);
            let right = if i + 1 < runs.len() { Some(i + 1) } else { None };
            let target = match (left, right) {
                (Some(l), Some(r)) => {
                    let dl = (runs[l].pitch as i32 - runs[i].pitch as i32).abs();
                    let dr = (runs[r].pitch as i32 - runs[i].pitch as i32).abs();
                    if dl < dr || (dl == dr && runs[l].duration() >= runs[r].duration()) {
                        l
                    } else {
                        r
                    }
                }
                (Some(l), None) => l,
                (None, Some(r)) => r,
                (None, None) => break,
            };

            // Extend the target over run i's span (they are adjacent), drop i.
            if target < i {
                runs[target].end_s = runs[i].end_s;
            } else {
                runs[target].start_s = runs[i].start_s;
            }
            runs.remove(i);
            coalesce_equal_pitch(&mut runs);
        }
        runs
    }
}

/// Round a frequency to the nearest MIDI pitch, clamped to `0..=127`.
fn midi_round(hz: f32) -> u8 {
    hz_to_midi(hz).round().clamp(0.0, 127.0) as u8
}

/// Merge adjacent plateaus that share a pitch (an absorb step can leave two).
fn coalesce_equal_pitch(runs: &mut Vec<Plateau>) {
    let mut k = 0;
    while k + 1 < runs.len() {
        if runs[k].pitch == runs[k + 1].pitch {
            runs[k].end_s = runs[k + 1].end_s;
            runs.remove(k + 1);
        } else {
            k += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP_S: f64 = 256.0 / 44_100.0; // the shared pYIN / onset hop, ~5.8 ms

    /// Build an energy envelope at the shared hop with a piecewise-constant RMS:
    /// `levels` gives (until_time_s, rms) segments.
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

    /// A voiced f0 contour at the shared hop: `segments` gives (until_time_s, hz);
    /// hz <= 0 marks an unvoiced stretch.
    fn contour(segments: &[(f64, f32)]) -> Vec<F0Frame> {
        let end = segments.last().map(|s| s.0).unwrap_or(0.0);
        let n = (end / HOP_S) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 * HOP_S;
                let hz = segments
                    .iter()
                    .find(|(until, _)| t < *until)
                    .map(|(_, h)| *h)
                    .unwrap_or(0.0);
                F0Frame {
                    time_s: t,
                    frequency_hz: hz.max(0.0),
                    voicing: if hz > 0.0 { 0.9 } else { 0.0 },
                }
            })
            .collect()
    }

    fn onset(t: f64) -> Onset {
        Onset {
            time_s: t,
            strength: 1.0,
        }
    }

    #[test]
    fn hz_to_midi_maps_a440_to_69() {
        assert!((hz_to_midi(440.0) - 69.0).abs() < 1e-4);
        assert!((hz_to_midi(880.0) - 81.0).abs() < 1e-4); // octave up
        assert_eq!(midi_round(261.63), 60); // middle C
    }

    #[test]
    fn no_onsets_yields_no_notes() {
        let seg = Segmenter::default();
        assert!(seg.segment(&[], &[], &envelope(&[(1.0, 0.5)])).is_empty());
    }

    #[test]
    fn staccato_note_closes_on_energy_decay_not_next_onset() {
        // Note sounds 0.1..0.4 then a clean gap; next note attacks at 0.8. The
        // next onset is 0.7 s away — well past max_offset_gap_s — so the note must
        // close on energy decay (~0.4), leaving a rest, not stretch to 0.8.
        let env = envelope(&[(0.1, 0.01), (0.4, 0.5), (0.8, 0.01), (1.1, 0.5), (1.2, 0.01)]);
        let f0 = contour(&[(0.1, 0.0), (0.4, 440.0), (0.8, 0.0), (1.1, 523.25), (1.2, 0.0)]);
        let onsets = [onset(0.1), onset(0.8)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        assert_eq!(notes.len(), 2, "two notes: {notes:?}");
        assert!(
            (notes[0].offset_s - 0.4).abs() < 0.03,
            "first note closes near the energy drop 0.4, got {}",
            notes[0].offset_s
        );
        assert_eq!(notes[0].offset.kind, OffsetKind::EnergyDecay);
        assert!(
            notes[0].offset.drop_steepness > 0.7,
            "sharp staccato drop scores high steepness, got {}",
            notes[0].offset.drop_steepness
        );
        assert_eq!(notes[0].midi_pitch, 69); // A4
        assert_eq!(notes[1].midi_pitch, 72); // C5
    }

    #[test]
    fn back_to_back_notes_close_on_next_onset() {
        // Two connected notes, no gap: energy stays up across the boundary at
        // 0.2 and the next onset is within threshold (0.15 s < 0.18 s), so the
        // first closes on the next onset, not on decay.
        let env = envelope(&[(0.05, 0.01), (0.6, 0.5), (0.65, 0.01)]);
        let f0 = contour(&[(0.05, 0.0), (0.2, 440.0), (0.6, 440.0), (0.65, 0.0)]);
        let onsets = [onset(0.05), onset(0.2)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        assert_eq!(notes.len(), 2, "two notes: {notes:?}");
        assert!(
            (notes[0].offset_s - 0.2).abs() < 1e-9,
            "first note closes on the next onset 0.2, got {}",
            notes[0].offset_s
        );
        assert_eq!(notes[0].offset.kind, OffsetKind::NextOnset);
    }

    #[test]
    fn legato_pitch_change_splits_on_plateau() {
        // One onset, energy steady throughout, but the pitch steps 440 -> 494 at
        // 0.4 and holds. No re-attack, so onset detection missed it; the plateau
        // step must split it into two notes.
        let env = envelope(&[(0.05, 0.01), (0.85, 0.5), (0.9, 0.01)]);
        let f0 = contour(&[(0.05, 0.0), (0.4, 440.0), (0.85, 493.88), (0.9, 0.0)]);
        let onsets = [onset(0.05)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        assert_eq!(notes.len(), 2, "legato split into two notes: {notes:?}");
        assert_eq!(notes[0].midi_pitch, 69); // A4
        assert_eq!(notes[1].midi_pitch, 71); // B4
        assert!(!notes[0].split_from_plateau);
        assert!(notes[1].split_from_plateau);
        assert_eq!(notes[0].offset.kind, OffsetKind::PlateauSplit);
        assert_eq!(notes[1].onset_strength, 0.0, "a split note has no transient");
        assert!(
            (notes[0].offset_s - notes[1].onset_s).abs() < 1e-9,
            "the split is seamless: note 0 ends where note 1 begins"
        );
    }

    #[test]
    fn vibrato_does_not_over_split() {
        // A held A4 with vibrato that briefly crosses into G#4/A#4 for one hop at
        // a time. Those excursions are shorter than min_plateau_s, so they are
        // absorbed: one note, not a stutter of fragments.
        let mut segs = vec![(0.05, 0.0)];
        // Alternate 440 and a neighbour every hop from 0.05 to ~0.6.
        let mut t = 0.05;
        let mut up = true;
        while t < 0.6 {
            t += HOP_S;
            let hz = if up { 440.0 } else { 466.16 }; // A4 <-> A#4
            up = !up;
            segs.push((t, hz));
        }
        segs.push((0.65, 0.0));
        let f0 = contour(&segs);
        let env = envelope(&[(0.05, 0.01), (0.6, 0.5), (0.65, 0.01)]);
        let onsets = [onset(0.05)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        assert_eq!(
            notes.len(),
            1,
            "vibrato is one note, not many: {:?}",
            notes.iter().map(|n| n.midi_pitch).collect::<Vec<_>>()
        );
    }

    #[test]
    fn slow_tail_scores_low_offset_steepness() {
        // A note whose energy tails off gradually rather than dropping sharply.
        // The last note closes at the stem end; its drop steepness should be low.
        let env = envelope(&[
            (0.05, 0.01),
            (0.3, 0.5),
            (0.35, 0.4),
            (0.4, 0.32),
            (0.45, 0.26),
            (0.5, 0.2),
            (0.9, 0.18),
        ]);
        let f0 = contour(&[(0.05, 0.0), (0.9, 440.0)]);
        let onsets = [onset(0.05)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        let last = notes.last().unwrap();
        assert!(
            last.offset.drop_steepness < 0.3,
            "a slow tail scores low steepness, got {}",
            last.offset.drop_steepness
        );
    }

    #[test]
    fn trailing_note_closes_at_stem_end_when_energy_holds() {
        // Single note whose energy never decays to the rest floor: it closes at
        // the stem end with the StemEnd kind.
        let env = envelope(&[(0.05, 0.01), (0.9, 0.5)]);
        let f0 = contour(&[(0.05, 0.0), (0.9, 440.0)]);
        let onsets = [onset(0.05)];

        let notes = Segmenter::default().segment(&onsets, &f0, &env);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].offset.kind, OffsetKind::StemEnd);
        assert!(notes[0].offset_s > 0.8, "runs to the stem end");
    }
}
