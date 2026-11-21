//! What every consumer of sona's notes relies on, checked on generated inputs for
//! both segmenters (the note-HMM default and grid mode, ADR-0015):
//!
//! - one voice: notes in onset order, never overlapping;
//! - every note ends after it starts; pitch is a MIDI number;
//! - confidence and its sub-scores are finite and in `0..=1`;
//! - grid mode: every onset and offset sits on one grid at the requested spacing.
//!
//! A UST or MIDI export of an overlapping, zero-length or out-of-range note is
//! corrupt, and no example-based test reaches the odd inputs (octave blips, tiny
//! notes, dropouts, bleed) that would produce one.

use sona_core::{
    Annotation, CandidateEvidence, EnergyFrame, F0Frame, GridDecoder, GridMode, GridSpec, NoteHmm, NoteSegment, Onset, PYin, PitchCandidate,
    Producer, Transcriber,
};
use std::f32::consts::PI;

const HOP_S: f64 = 256.0 / 44_100.0;
const SR: u32 = 44_100;

/// Deterministic xorshift, so a failure reproduces from its case number.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }
}

/// A random singing-like stem as the segmenters see it: notes of random length
/// (some far below a grid cell), rests, vibrato, glides, octave blips, voicing
/// dropouts, and quiet bleed.
fn random_tracks(rng: &mut Rng) -> (Vec<F0Frame>, Vec<EnergyFrame>, Vec<Onset>) {
    let (mut contour, mut envelope, mut onsets) = (Vec::new(), Vec::new(), Vec::new());
    let mut t = 0.0;
    let end = rng.range(1.0, 6.0);
    while t < end {
        let dur = rng.range(0.02, 0.6);
        let rest = rng.unit() < 0.25;
        let midi = rng.range(45.0, 85.0) as f32;
        let loud = if rng.unit() < 0.1 { 0.02 } else { rng.range(0.2, 0.9) as f32 };
        if !rest && rng.unit() < 0.7 {
            onsets.push(Onset { time_s: t + rng.range(-0.02, 0.02), strength: rng.range(0.1, 3.0) as f32 });
        }
        let n = (dur / HOP_S).max(1.0) as usize;
        for i in 0..n {
            let ti = t + i as f64 * HOP_S;
            let dropout = rng.unit() < 0.08;
            let (hz, voicing, rms) = if rest || dropout {
                (0.0, rng.range(0.0, 0.3) as f32, if rest { 0.01 } else { loud })
            } else {
                let vib = 0.3 * (2.0 * PI * 5.5 * ti as f32).sin();
                let blip = if rng.unit() < 0.02 { 12.0 } else { 0.0 };
                let semi = midi + vib + blip + rng.range(-0.15, 0.15) as f32;
                (440.0 * 2f32.powf((semi - 69.0) / 12.0), rng.range(0.5, 1.0) as f32, loud)
            };
            contour.push(F0Frame { time_s: ti, frequency_hz: hz, voicing });
            envelope.push(EnergyFrame { time_s: ti, rms });
        }
        t += n as f64 * HOP_S;
    }
    (contour, envelope, onsets)
}

fn assert_one_voice(notes: &[NoteSegment], what: &str) {
    for n in notes {
        assert!(n.offset_s > n.onset_s, "{what}: empty note {n:?}");
        assert!(n.midi_pitch <= 127, "{what}: pitch {n:?}");
        assert!(n.onset_s.is_finite() && n.offset_s.is_finite(), "{what}: non-finite {n:?}");
    }
    for w in notes.windows(2) {
        assert!(w[0].onset_s < w[1].onset_s, "{what}: out of order {:?} then {:?}", w[0], w[1]);
        assert!(w[0].offset_s <= w[1].onset_s + 1e-9, "{what}: overlap {:?} then {:?}", w[0], w[1]);
    }
}

fn assert_on_one_grid(notes: &[NoteSegment], spacing: f64, what: &str) {
    let Some(first) = notes.first() else { return };
    for n in notes {
        for t in [n.onset_s, n.offset_s] {
            let k = (t - first.onset_s) / spacing;
            assert!((k - k.round()).abs() < 1e-6, "{what}: {t} is off the grid through {}", first.onset_s);
        }
    }
}

#[test]
fn both_segmenters_emit_one_voice_on_random_stems() {
    let mut rng = Rng(0x5eed_504a);
    for case in 0..300 {
        let (contour, envelope, onsets) = random_tracks(&mut rng);
        let hmm = NoteHmm::default().segment(&onsets, &contour, &envelope);
        assert_one_voice(&hmm, &format!("note-HMM case {case}"));

        let grid = GridSpec {
            bpm: rng.range(80.0, 250.0).round(),
            division: if rng.unit() < 0.3 { 24 } else { 16 },
            anchor_s: if rng.unit() < 0.5 { Some(rng.range(-0.2, 0.2)) } else { None },
        };
        let cells = GridDecoder::default().segment(&grid, &onsets, &contour, &envelope);
        let what = format!("grid case {case} ({grid:?})");
        assert_one_voice(&cells, &what);
        assert_on_one_grid(&cells, grid.spacing_s(), &what);

        // With pitch candidates (some frames with none, the tail missing), drawn
        // from their own stream so the cases above stay as they were.
        let mut crng = Rng(0xca4d_0000 + case);
        let n = contour.len().saturating_sub((crng.unit() * 20.0) as usize);
        let cands: Vec<Vec<PitchCandidate>> = (0..n)
            .map(|_| {
                (0..(crng.unit() * 5.0) as usize)
                    .map(|_| PitchCandidate { frequency_hz: crng.range(50.0, 1500.0) as f32, salience: crng.range(0.0, 3.0) as f32 })
                    .collect()
            })
            .collect();
        let decoder = GridDecoder { candidates: Some(CandidateEvidence::default()), ..GridDecoder::default() };
        let cells = decoder.segment_with_candidates(&grid, &onsets, &contour, &cands, &envelope);
        let what = format!("grid+candidates case {case} ({grid:?})");
        assert_one_voice(&cells, &what);
        assert_on_one_grid(&cells, grid.spacing_s(), &what);
    }
}

fn assert_valid_annotation(a: &Annotation, what: &str) {
    for n in &a.notes {
        assert!(n.offset_s > n.onset_s && n.midi_pitch <= 127, "{what}: {n:?}");
        let s = &n.sub_scores;
        for x in [n.confidence, s.pitch_stability, s.voicing, s.onset_clarity, s.offset_clarity] {
            assert!(x.is_finite() && (0.0..=1.0).contains(&x), "{what}: score {x} out of range in {n:?}");
        }
    }
    for w in a.notes.windows(2) {
        assert!(w[0].offset_s <= w[1].onset_s + 1e-9, "{what}: overlap {:?} then {:?}", w[0], w[1]);
    }
}

/// Through the public pipeline on real audio: random melodies of harmonic tones,
/// with rests and a quiet second voice, transcribed in both modes.
#[test]
fn transcribed_annotations_are_valid_in_both_modes() {
    let mut rng = Rng(0xa110_f175);
    for case in 0..6 {
        let mut audio = Vec::new();
        while audio.len() < 3 * SR as usize {
            let dur = rng.range(0.05, 0.5);
            let n = (dur * SR as f64) as usize;
            if rng.unit() < 0.2 {
                audio.extend(std::iter::repeat_n(0.0f32, n));
                continue;
            }
            let (f, g) = (rng.range(150.0, 800.0) as f32, rng.range(150.0, 800.0) as f32);
            let amp = rng.range(0.2, 0.7) as f32;
            audio.extend((0..n).map(|i| {
                let p = 2.0 * PI * i as f32 / SR as f32;
                amp * (0.6 * (f * p).sin() + 0.3 * (2.0 * f * p).sin()) + 0.05 * (g * p).sin()
            }));
        }
        let bpm = rng.range(90.0, 200.0).round();
        let default = Transcriber::default().transcribe(&PYin::default(), &audio, SR, bpm, Producer::Pyin);
        assert_valid_annotation(&default, &format!("note-HMM case {case}"));
        let grid = Transcriber { grid: Some(GridMode::sixteenths()), ..Transcriber::default() }
            .transcribe(&PYin::default(), &audio, SR, bpm, Producer::Pyin);
        assert_valid_annotation(&grid, &format!("grid case {case}"));
        let mut mode = GridMode::sixteenths();
        mode.decoder.candidates = Some(CandidateEvidence::default());
        let cands = Transcriber { grid: Some(mode), ..Transcriber::default() }.transcribe(&PYin::default(), &audio, SR, bpm, Producer::Pyin);
        assert_valid_annotation(&cands, &format!("grid+candidates case {case}"));
    }
}
