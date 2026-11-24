//! What every consumer of sona's notes relies on, checked on generated inputs for
//! every segmenter: the note-HMM default, pYIN grid mode (ADR-0015), GAME's grid
//! snapping with the octave check (ADR-0017, `--features game`) and the learned
//! cell stage (ADR-0026) from its GAME note snapping to its cells → notes step and the notes the app gets
//! (with the stage's confidence, ADR-0033):
//!
//! - one voice: notes in onset order, never overlapping;
//! - every note ends after it starts; pitch is a MIDI number;
//! - confidence and its sub-scores are finite and in `0..=1`;
//! - grid mode: every onset and offset sits on one grid at the requested spacing
//!   (the cell stage: on the grid phase it was given).
//!
//! A UST or MIDI export of an overlapping, zero-length or out-of-range note is
//! corrupt, and no example-based test reaches the odd inputs (octave blips, tiny
//! notes, dropouts, bleed) that would produce one. GAME and the cell model need
//! weights CI doesn't have, so their note assembly is driven with random raw
//! GAME notes and random per-cell choices.

use sona_core::cellstage::{cell_features, cells_to_notes, choose_class, note_confidence, snap_notes, Candidates, EncoderChunk, Sources, StageInputs, StageNote};
use sona_core::{
    timbre::N_CEPS, Annotation, CandidateEvidence, CueModel, EnergyFrame, F0Frame, GridDecoder, GridMode, GridSpec, Note, NoteHmm, NoteSegment,
    Onset, PYin, PitchCandidate, Producer, SpectralCue, TimbreFrames, Transcriber,
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
        assert!(n.midi_pitch <= 127, "{what}: pitch {n:?}");
    }
    assert_spans_one_voice(&notes.iter().map(|n| (n.onset_s, n.offset_s)).collect::<Vec<_>>(), what);
}

fn assert_spans_one_voice(spans: &[(f64, f64)], what: &str) {
    for n in spans {
        assert!(n.1 > n.0, "{what}: empty note {n:?}");
        assert!(n.0.is_finite() && n.1.is_finite(), "{what}: non-finite {n:?}");
    }
    for w in spans.windows(2) {
        assert!(w[0].0 < w[1].0, "{what}: out of order {:?} then {:?}", w[0], w[1]);
        assert!(w[0].1 <= w[1].0 + 1e-9, "{what}: overlap {:?} then {:?}", w[0], w[1]);
    }
}

fn assert_on_grid_at(spans: &[(f64, f64)], anchor: f64, spacing: f64, what: &str) {
    for n in spans {
        for t in [n.0, n.1] {
            let k = (t - anchor) / spacing;
            assert!((k - k.round()).abs() < 1e-6, "{what}: {t} is off the grid at {anchor} + k × {spacing}");
        }
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

        // The spectral re-articulation cue, on random timbre frames (own stream, so
        // the cases above stay the same).
        let mut trng = Rng(0x7153_0000 + case);
        let timbre = TimbreFrames {
            time_s: contour.iter().map(|f| f.time_s).collect(),
            flux: contour.iter().map(|_| trng.range(0.0, 8.0) as f32).collect(),
            flux_lo: contour.iter().map(|_| trng.range(0.0, 8.0) as f32).collect(),
            ceps: contour.iter().map(|_| [trng.range(-1.0, 1.0) as f32; N_CEPS]).collect(),
        };
        let spectral = GridDecoder { cue_model: CueModel::Spectral(SpectralCue::default()), ..GridDecoder::default() };
        let cells = spectral.segment_with_timbre(&grid, &onsets, &contour, &envelope, Some(&timbre));
        let what = format!("spectral-cue grid case {case} ({grid:?})");
        assert_one_voice(&cells, &what);
        assert_on_one_grid(&cells, grid.spacing_s(), &what);
    }
}

fn assert_valid_annotation(a: &Annotation, what: &str) {
    assert_valid_notes(&a.notes, what);
}

fn assert_valid_notes(notes: &[Note], what: &str) {
    for n in notes {
        assert!(n.offset_s > n.onset_s && n.midi_pitch <= 127, "{what}: {n:?}");
        let s = &n.sub_scores;
        for x in [n.confidence, s.pitch_stability, s.voicing, s.onset_clarity, s.offset_clarity] {
            assert!(x.is_finite() && (0.0..=1.0).contains(&x), "{what}: score {x} out of range in {n:?}");
        }
    }
    for w in notes.windows(2) {
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

/// One grid cell at a random tempo, sixteenths or sixteenth triplets.
fn random_spacing(rng: &mut Rng) -> f64 {
    240.0 / rng.range(60.0, 250.0).round() / if rng.unit() < 0.3 { 24.0 } else { 16.0 }
}

/// A grid phase in `[0, spacing)`, often at its edges: exactly 0, or just under one cell.
fn random_phase(rng: &mut Rng, spacing: f64) -> f64 {
    match (rng.unit() * 5.0) as u32 {
        0 => 0.0,
        1 => f64::from_bits(spacing.to_bits() - 1),
        2 => spacing * (1.0 - 1e-7),
        _ => rng.range(0.0, spacing),
    }
}

/// Raw GAME-like notes (onset, offset, continuous MIDI, voiced): mostly back to back like GAME's
/// segments, near a grid at `phase`, some on its 10 ms frames, with rests, a song-wide tuning offset,
/// sub-cell and zero-length notes, octave blips, a few overlaps and stray pitches.
fn random_raw_notes(rng: &mut Rng, spacing: f64, phase: f64) -> Vec<(f64, f64, f32, bool)> {
    let tune = rng.range(-0.5, 0.5);
    let jitter = rng.range(0.0, 0.6) * spacing;
    let frames = rng.unit() < 0.5;
    let at = |x: f64| if frames { (x / 0.01).round() * 0.01 } else { x };
    let mut t = phase + rng.range(0.0, 8.0).floor() * spacing;
    let mut out = Vec::new();
    for _ in 0..(rng.unit() * 60.0) as usize {
        let u = rng.unit();
        let len = if u < 0.08 {
            0.0
        } else if u < 0.16 {
            rng.range(0.0, 1e-3)
        } else if u < 0.4 {
            rng.range(0.0, spacing)
        } else {
            rng.range(1.0, 8.0).round() * spacing + rng.range(-jitter, jitter)
        };
        let on = if rng.unit() < 0.1 { (t - rng.range(0.0, 2.0) * spacing).max(0.0) } else { t };
        let u = rng.unit();
        let blip = if u < 0.08 { 12.0 } else if u < 0.11 { -12.0 } else { 0.0 };
        let midi = if u < 0.03 { rng.range(0.0, 127.0) } else { rng.range(40.0, 90.0).round() + tune + rng.range(-0.2, 0.2) + blip };
        out.push((at(on), at(on + len), midi as f32, rng.unit() < 0.8));
        t = on + len;
    }
    out
}

fn stage_notes(raw: &[(f64, f64, f32, bool)]) -> Vec<StageNote> {
    raw.iter().map(|&(onset_s, offset_s, midi, voiced)| StageNote { onset_s, offset_s, midi, voiced }).collect()
}

/// Everything the learned cell stage reads, at random: GAME's raw notes on the lead (with octave-check
/// ratios) and the vocals, sona's grid notes on the true phase, an encoder chunk, and the vocals' pitch
/// track and energy.
struct StageCase {
    spacing: f64,
    phase: f64,
    lead: Vec<StageNote>,
    ratios: Vec<f32>,
    vocals: Vec<StageNote>,
    sona: Vec<(f64, f64, i32)>,
    frames: Vec<EncoderChunk>,
    f0: Vec<(f64, f64)>,
    rms: Vec<(f64, f64)>,
    duration: f64,
}

impl StageCase {
    fn random(rng: &mut Rng) -> Self {
        let spacing = random_spacing(rng);
        let phase = random_phase(rng, spacing);
        let lead = stage_notes(&random_raw_notes(rng, spacing, phase));
        let ratios = lead.iter().map(|_| rng.range(-3.0, 2.0) as f32).collect();
        let vocals = stage_notes(&random_raw_notes(rng, spacing, phase));
        let mut sona = Vec::new();
        let mut k = (rng.unit() * 6.0) as i64;
        for _ in 0..(rng.unit() * 40.0) as usize {
            let len = 1 + (rng.unit() * 6.0) as i64;
            sona.push((phase + k as f64 * spacing, phase + (k + len) as f64 * spacing, rng.range(40.0, 90.0) as i32));
            k += len + if rng.unit() < 0.3 { 1 + (rng.unit() * 4.0) as i64 } else { 0 };
        }
        let end = lead.iter().chain(&vocals).map(|n| n.offset_s).chain(sona.iter().map(|n| n.1)).fold(0.0, f64::max);
        let duration = end + rng.range(0.0, 1.0);
        let dim = 4;
        let rows = (rng.range(0.0, duration) / 0.01) as usize;
        let frames = vec![EncoderChunk { t0: rng.range(0.0, 0.5), frames: rows, dim, data: (0..rows * dim).map(|_| rng.range(-2.0, 2.0) as f32).collect() }];
        let hops = (duration / HOP_S) as usize;
        let f0 = (0..hops).map(|i| (i as f64 * HOP_S, if rng.unit() < 0.3 { 0.0 } else { rng.range(80.0, 900.0) })).collect();
        let rms = (0..hops).map(|i| (i as f64 * HOP_S, rng.range(0.0, 0.8))).collect();
        Self { spacing, phase, lead, ratios, vocals, sona, frames, f0, rms, duration }
    }

    fn inputs(&self, anchor: f64) -> StageInputs<'_> {
        StageInputs {
            spacing: self.spacing,
            anchor,
            lead_notes: &self.lead,
            lead_ratios: &self.ratios,
            vocals_notes: &self.vocals,
            sona_notes: &self.sona,
            lead_frames: &self.frames,
            vocals_frames: &self.frames,
            f0: &self.f0,
            rms: &self.rms,
        }
    }

    /// The cell stage without its model: features, random logits, the masked choice, cells → notes.
    fn notes(&self, rng: &mut Rng, anchor: f64, cells: (i64, usize)) -> Vec<(f64, f64, i32)> {
        self.notes_with_confidence(rng, anchor, cells).0
    }

    /// [`notes`](Self::notes) and each note's confidence from the same random logits.
    fn notes_with_confidence(&self, rng: &mut Rng, anchor: f64, (k0, n): (i64, usize)) -> (Vec<(f64, f64, i32)>, Vec<f32>) {
        let (_, cands, sources) = cell_features(&self.inputs(anchor), k0, n);
        let logits: Vec<f32> = (0..n * 6).map(|_| rng.range(-4.0, 4.0) as f32).collect();
        let notes = cells_to_notes(&choose_class(&logits, &cands), &cands, &sources, k0, self.spacing, anchor);
        let confidence = note_confidence(&logits, k0, self.spacing, anchor, &notes);
        (notes, confidence)
    }
}

fn spans(notes: &[(f64, f64, i32)]) -> Vec<(f64, f64)> {
    notes.iter().map(|n| (n.0, n.1)).collect()
}

#[test]
fn cell_stage_snapping_keeps_every_voiced_note_in_one_voice_on_the_grid() {
    let mut rng = Rng(0x5a17_0026);
    for case in 0..3000 {
        let spacing = random_spacing(&mut rng);
        let anchor = random_phase(&mut rng, spacing);
        let notes = stage_notes(&random_raw_notes(&mut rng, spacing, anchor));
        let snapped = snap_notes(&notes, spacing, anchor);
        let what = format!("snap_notes case {case} (spacing {spacing}, anchor {anchor})");
        assert_eq!(snapped.len(), notes.iter().filter(|n| n.voiced).count(), "{what}: a voiced note was lost");
        assert_spans_one_voice(&spans(&snapped), &what);
        assert_on_grid_at(&spans(&snapped), anchor, spacing, &what);
    }
}

#[test]
fn cell_stage_notes_are_exactly_the_cells_choices() {
    let mut rng = Rng(0xce11_0026);
    for case in 0..3000 {
        let spacing = random_spacing(&mut rng);
        let anchor = random_phase(&mut rng, spacing);
        let (k0, n) = (rng.range(-3.0, 40.0).floor() as i64, (rng.unit() * 80.0) as usize);
        let pitch = |rng: &mut Rng| if rng.unit() < 0.4 { -1 } else { 60 + 2 * (rng.unit() * 3.0) as i32 };
        let source = |rng: &mut Rng| Some((rng.unit() * 3.0) as usize);
        let cands: Vec<Candidates> = (0..n)
            .map(|_| {
                let l = pitch(&mut rng);
                [-1, l, pitch(&mut rng), pitch(&mut rng), if l >= 0 { l + 12 } else { -1 }, if l >= 0 { l - 12 } else { -1 }]
            })
            .collect();
        let sources: Vec<Sources> = (0..n)
            .map(|_| {
                let l = source(&mut rng);
                [None, l, source(&mut rng), source(&mut rng), l, l]
            })
            .collect();
        let classes: Vec<usize> = (0..n).map(|_| if rng.unit() < 0.25 { 0 } else { (rng.unit() * 6.0) as usize }).collect();
        let notes = cells_to_notes(&classes, &cands, &sources, k0, spacing, anchor);
        let what = format!("cells_to_notes case {case} (spacing {spacing}, anchor {anchor}, k0 {k0})");
        assert_spans_one_voice(&spans(&notes), &what);
        assert_on_grid_at(&spans(&notes), anchor, spacing, &what);
        for c in 0..n {
            let chosen = if classes[c] == 0 { -1 } else { cands[c][classes[c]] };
            let mid = anchor + ((k0 + c as i64) as f64 + 0.5) * spacing;
            match notes.iter().find(|x| x.0 <= mid && mid < x.1) {
                Some(x) => assert!(chosen >= 0 && x.2 == chosen, "{what}: cell {c} chose {chosen} but sits in {x:?}"),
                None => assert!(chosen < 0, "{what}: cell {c} chose {chosen} but no note holds it"),
            }
        }
    }
}

#[test]
fn cell_stage_emits_one_voice_on_its_grid_from_random_inputs() {
    let mut rng = Rng(0x57a6_e026);
    for case in 0..300 {
        let s = StageCase::random(&mut rng);
        let k0 = ((0.0 - s.phase) / s.spacing).floor() as i64;
        let n = ((s.duration - s.phase) / s.spacing).ceil().max(0.0) as usize + 1;
        let notes = s.notes(&mut rng, s.phase, (k0, n));
        let what = format!("cell stage case {case} (spacing {}, anchor {})", s.spacing, s.phase);
        assert!(notes.iter().all(|x| x.2 >= 0), "{what}: a rest came out as a note: {notes:?}");
        assert_spans_one_voice(&spans(&notes), &what);
        assert_on_grid_at(&spans(&notes), s.phase, s.spacing, &what);
    }
}

/// GAME's own engine and the cell stage's last step, both behind `--features game`. The module name puts
/// them under CI's `game::` test filter.
#[cfg(feature = "game")]
mod game {
    use super::*;
    use sona_core::game::{cells_covering, correct_octaves_with, grid_spacing_s, phase_of, snap_to_grid, stage_annotation_notes, stage_segments, GameNote, OctaveRule, StageNotes};
    use sona_core::ConfidenceScorer;

    fn envelope(rng: &mut Rng, duration: f64) -> Vec<EnergyFrame> {
        (0..(duration / HOP_S) as usize).map(|i| EnergyFrame { time_s: i as f64 * HOP_S, rms: if rng.unit() < 0.2 { 0.0 } else { rng.range(0.0, 0.9) as f32 } }).collect()
    }

    #[test]
    fn snap_to_grid_emits_one_voice_on_one_grid_with_or_without_the_octave_check() {
        let mut rng = Rng(0x6a3e_0017);
        for case in 0..3000 {
            let spacing = grid_spacing_s(rng.range(60.0, 250.0).round(), if rng.unit() < 0.3 { 24 } else { 16 });
            let phase = random_phase(&mut rng, spacing);
            let mut notes: Vec<GameNote> = random_raw_notes(&mut rng, spacing, phase).into_iter().map(|(onset_s, offset_s, midi, voiced)| GameNote { onset_s, offset_s, midi, voiced }).collect();
            if rng.unit() < 0.5 {
                let ratios: Vec<f32> = notes.iter().map(|_| rng.range(-3.0, 2.0) as f32).collect();
                correct_octaves_with(&mut notes, &ratios, OctaveRule::default());
            }
            let segs = snap_to_grid(&notes, spacing);
            let what = format!("snap_to_grid case {case} (spacing {spacing}, phase {phase})");
            assert_eq!(segs.len(), notes.iter().filter(|n| n.voiced && n.offset_s > n.onset_s).count(), "{what}: a voiced note was lost");
            assert_one_voice(&segs, &what);
            assert_on_one_grid(&segs, spacing, &what);
            let end = segs.last().map_or(1.0, |n| n.offset_s + 0.5);
            assert_valid_notes(&ConfidenceScorer::default().score_all(&segs, &envelope(&mut rng, end)), &what);
        }
    }

    #[test]
    fn cell_stage_output_is_one_voice_of_midi_notes_on_the_estimated_phase() {
        let mut rng = Rng(0xce11_6a3e);
        for case in 0..300 {
            let s = StageCase::random(&mut rng);
            let what = format!("cell stage case {case} (spacing {}, true phase {})", s.spacing, s.phase);
            let anchor = phase_of(&s.sona, s.spacing);
            assert!((0.0..s.spacing).contains(&anchor), "{what}: phase {anchor} is outside one cell");
            assert_on_grid_at(&spans(&s.sona), anchor, s.spacing, &what);
            let (k0, n) = cells_covering(s.duration, s.spacing, anchor);
            let first = anchor + k0 as f64 * s.spacing;
            assert!(first <= 0.0 && first > -s.spacing, "{what}: the first cell starts at {first}");
            assert!(anchor + (k0 + n as i64) as f64 * s.spacing >= s.duration - s.spacing / 2.0, "{what}: the cells stop short of {}", s.duration);
            let (notes, confidence) = s.notes_with_confidence(&mut rng, anchor, (k0, n));
            let segs = stage_segments(&notes);
            assert_one_voice(&segs, &what);
            assert_on_grid_at(&segs.iter().map(|n| (n.onset_s, n.offset_s)).collect::<Vec<_>>(), anchor, s.spacing, &what);
            let shown = stage_annotation_notes(&StageNotes { notes, confidence }, &ConfidenceScorer::default(), &envelope(&mut rng, s.duration + 0.5));
            assert_eq!(shown.len(), segs.len(), "{what}: a note was lost on the way to the app");
            assert_valid_notes(&shown, &what);
        }
    }
}
