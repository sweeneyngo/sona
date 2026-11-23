//! **Pipeline composition** — the perception stages wired end to end (spec
//! §Pipeline). This is the "emit" step the crate docs promised: a mono stem
//! (+ BPM) becomes a complete annotation [`Annotation`].
//!
//! Each of the four perception stages owns a tunable struct; [`Transcriber`]
//! just threads their outputs together in order and packs the result into the
//! annotation JSON contract (ADR-0004):
//!
//! 1. [`PitchTracker`] → the f0 contour.
//! 2. [`OnsetDetector`] → onsets + the energy/RMS envelope.
//! 3. [`Segmenter`] → note segments from contour + onsets + envelope.
//! 4. [`ConfidenceScorer`] → annotation [`Note`]s with confidence.
//!
//! The tracker is passed in rather than owned so the pluggable-backend seam
//! (ADR-0001) reaches all the way to the top: the CLI hands in pYIN today, CREPE
//! or a basic-pitch adapter later, and names the matching [`Producer`] so the
//! annotation records which backend spoke.

use crate::annotation::{Annotation, F0Frame, Producer, SCHEMA_VERSION};
use crate::confidence::ConfidenceScorer;
use crate::grid::{CueModel, GridMode, GridSpec};
use crate::timbre::TimbreFrames;
use crate::onset::OnsetDetector;
use crate::pitch::{HarmonicSalience, PitchCandidate, PitchTracker};
use crate::segment::Segmenter;

/// Composes the perception stages into a single `stem -> annotation` transform.
///
/// Holds the three tunable stage structs (the pitch tracker is supplied per call
/// so backends can be swapped). Construct with [`Transcriber::default`] for the
/// staccato-vocal defaults every stage already carries, or set fields to tune.
#[derive(Debug, Clone, Default)]
pub struct Transcriber {
    /// Onset detection + energy envelope.
    pub onset: OnsetDetector,
    /// Note segmentation.
    pub segmenter: Segmenter,
    /// Confidence scoring.
    pub scorer: ConfidenceScorer,
    /// Grid-conditioned decoding ([ADR-0015]): when set, notes are decoded on
    /// this grid at the call's BPM by the cell decoder instead of the
    /// note-HMM. `None` (the default) keeps the grid-free perception path.
    ///
    /// [ADR-0015]: ../../../docs/adr/0015-grid-conditioned-cell-decoding.md
    pub grid: Option<GridMode>,
}

impl Transcriber {
    /// Transcribe mono `samples` at `sample_rate` into an [`Annotation`], using
    /// `tracker` for pitch and recording `bpm` and `producer` verbatim.
    ///
    /// `bpm` is carried through untouched — the core does no beat tracking; it is
    /// there only to seed the UI's grid (ADR-0004). `producer` names the backend
    /// so a comparison run can tell whose annotation it is auditioning.
    pub fn transcribe(
        &self,
        tracker: &dyn PitchTracker,
        samples: &[f32],
        sample_rate: u32,
        bpm: f64,
        producer: Producer,
    ) -> Annotation {
        let f0_contour = tracker.track(samples, sample_rate);
        let onset_result = self.onset.detect(samples, sample_rate);
        let segments = match &self.grid {
            Some(g) => {
                let candidates = match g.decoder.candidates {
                    Some(_) => candidates_for(&f0_contour, samples, sample_rate),
                    None => Vec::new(),
                };
                let timbre = Self::timbre_for(g, samples, sample_rate, &self.onset);
                g.decoder.segment_with_evidence(
                    &GridSpec { bpm, division: g.division, anchor_s: g.anchor_s },
                    &onset_result.onsets,
                    &f0_contour,
                    &candidates,
                    &onset_result.energy_envelope,
                    timbre.as_ref(),
                )
            }
            None => self.segmenter.segment(
                &onset_result.onsets,
                &f0_contour,
                &onset_result.energy_envelope,
            ),
        };
        let notes = self
            .scorer
            .score_all(&segments, &onset_result.energy_envelope);

        Annotation {
            version: SCHEMA_VERSION,
            sample_rate,
            bpm,
            duration_s: samples.len() as f64 / sample_rate.max(1) as f64,
            producer,
            notes,
            f0_contour,
            energy_envelope: onset_result.energy_envelope,
            provenance: None,
        }
    }

    /// The timbre frames a [`CueModel::Spectral`] decoder reads, on the energy
    /// envelope's frame grid. `None` for other cue models, and always without
    /// the `spectral` feature (the decoder then uses the voicing/energy cue).
    fn timbre_for(g: &GridMode, samples: &[f32], sample_rate: u32, onset: &OnsetDetector) -> Option<TimbreFrames> {
        if !matches!(g.decoder.cue_model, CueModel::Spectral(_)) {
            return None;
        }
        #[cfg(feature = "spectral")]
        return Some(TimbreFrames::from_samples(samples, sample_rate, onset.frame_length, onset.hop_length));
        #[cfg(not(feature = "spectral"))]
        {
            let _ = (samples, sample_rate, onset);
            None
        }
    }
}

/// Harmonic-sum pitch candidates aligned to `contour`'s frames by time (the
/// tracker's hop is its own business): each contour frame takes the salience
/// frame that starts nearest it.
fn candidates_for(contour: &[F0Frame], samples: &[f32], sample_rate: u32) -> Vec<Vec<PitchCandidate>> {
    let hs = HarmonicSalience::default();
    let frames = hs.track(samples, sample_rate);
    let hop_s = hs.hop_length as f64 / sample_rate.max(1) as f64;
    contour
        .iter()
        .map(|f| {
            let k = (f.time_s / hop_s).round().max(0.0) as usize;
            frames.get(k).cloned().unwrap_or_default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stem shorter than a single analysis frame produces no notes and no
    /// envelope, but still a structurally complete annotation (metadata intact).
    #[test]
    fn tiny_stem_yields_a_complete_but_empty_annotation() {
        let ann = Transcriber::default().transcribe(
            &crate::PYin::default(),
            &[0.0; 64],
            44_100,
            120.0,
            Producer::Pyin,
        );
        assert_eq!(ann.version, SCHEMA_VERSION);
        assert_eq!(ann.bpm, 120.0);
        assert!(ann.notes.is_empty());
        assert!(ann.energy_envelope.is_empty());
    }

    /// `duration_s` is the sample count over the rate, and never divides by zero
    /// on a degenerate `sample_rate`.
    #[test]
    fn duration_is_samples_over_rate() {
        let ann = Transcriber::default().transcribe(
            &crate::PYin::default(),
            &vec![0.0; 44_100],
            44_100,
            120.0,
            Producer::Pyin,
        );
        assert!((ann.duration_s - 1.0).abs() < 1e-9);
    }

    /// One held pitch whose vowel changes at 0.6 s (same loudness, no voicing
    /// gap), over a faint noise floor: the spectral cue hears two syllables, the
    /// voicing/energy cue one note.
    #[cfg(feature = "spectral")]
    #[test]
    fn the_spectral_cue_splits_a_held_pitch_at_a_vowel_change() {
        use crate::grid::{CueModel, GridDecoder, SpectralCue};
        let sr = 44_100u32;
        let mut seed = 0x2545_f491u32;
        let audio: Vec<f32> = (0..sr as usize)
            .map(|i| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = 1e-3 * (seed as f32 / u32::MAX as f32 - 0.5);
                let t = i as f32 / sr as f32;
                // Harmonic weights of two "vowels" at 220 Hz with equal RMS.
                let w: [f32; 4] = if t < 0.6 { [1.0, 0.1, 0.05, 0.02] } else { [0.45, 0.7, 0.45, 0.3] };
                let norm = w.iter().map(|x| x * x).sum::<f32>().sqrt();
                let voiced = (0.2..0.9).contains(&t);
                let tone: f32 = w.iter().enumerate().map(|(h, a)| a * (std::f32::consts::TAU * 220.0 * (h + 1) as f32 * t).sin()).sum();
                noise + if voiced { 0.3 * tone / norm } else { 0.0 }
            })
            .collect();
        let transcribe = |decoder: GridDecoder| {
            let grid = GridMode { division: 16, anchor_s: Some(0.0), decoder };
            let notes = Transcriber { grid: Some(grid), ..Transcriber::default() }
                .transcribe(&crate::PYin::default(), &audio, sr, 150.0, Producer::Pyin)
                .notes;
            notes.iter().map(|n| ((n.onset_s * 10.0).round() as i32, (n.offset_s * 10.0).round() as i32, n.midi_pitch)).collect::<Vec<_>>()
        };
        // Salience off in both runs, so the test isolates the cue: after silence,
        // salience's window can open a note one cell early (a known limitation,
        // ADR-0016), which isn't what this test is about.
        let base = GridDecoder { candidates: None, ..GridDecoder::default() };
        let spectral = GridDecoder { cue_model: CueModel::Spectral(SpectralCue::default()), ..base };
        assert_eq!(transcribe(spectral), vec![(2, 6, 57), (6, 9, 57)]);
        assert_eq!(transcribe(GridDecoder { cue_model: CueModel::VoicingEnergy, ..base }), vec![(2, 9, 57)]);
    }
}
