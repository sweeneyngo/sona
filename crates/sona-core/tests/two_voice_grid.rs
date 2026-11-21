//! Grid mode where two voices overlap: a lead melody over a quieter sustained
//! harmony, the case where pYIN's single f0 fails (it unvoices the mixed frame
//! or locks onto a common subharmonic). With `GridDecoder::candidates`,
//! harmonic-sum salience supplies the lead's pitch and the decoded line follows
//! the lead.

use sona_core::{CandidateEvidence, GridMode, PYin, Producer, Transcriber};
use std::f32::consts::PI;

const SR: u32 = 44_100;
const BPM: f64 = 150.0; // 16th cell = 0.1 s
const LEAD: [u8; 6] = [69, 71, 72, 74, 72, 71];
/// C#4: a sixth to a tenth below the lead, never an octave (which would make
/// the lead a harmonic of the harmony, ambiguous by nature).
const HARMONY: u8 = 61;

fn voice(midi: u8, amp: f32, n: usize, offset: usize) -> impl Iterator<Item = f32> {
    let f = 440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0);
    (offset..offset + n).map(move |i| {
        let p = 2.0 * PI * f * i as f32 / SR as f32;
        amp * (0.6 * p.sin() + 0.3 * (2.0 * p).sin() + 0.15 * (3.0 * p).sin() + 0.08 * (4.0 * p).sin())
    })
}

/// 0.2 s of silence, then the lead's notes (4 cells each) over a sustained
/// harmony at `harmony_amp`, then 0.2 s of silence.
fn duet(harmony_amp: f32) -> Vec<f32> {
    let cell = (0.1 * SR as f64) as usize;
    let mut audio = vec![0.0f32; 2 * cell];
    let start = audio.len();
    for (k, &m) in LEAD.iter().enumerate() {
        audio.extend(voice(m, 0.4, 4 * cell, start + 4 * k * cell));
    }
    let n = audio.len() - start;
    for (slot, h) in audio[start..].iter_mut().zip(voice(HARMONY, harmony_amp, n, start)) {
        *slot += h;
    }
    audio.extend(vec![0.0f32; 2 * cell]);
    audio
}

/// The decoded line as `(onset cell, offset cell, pitch)`, grid anchored at 0.
fn decode(audio: &[f32], candidates: Option<CandidateEvidence>) -> Vec<(i64, i64, u8)> {
    let mut mode = GridMode::sixteenths();
    mode.anchor_s = Some(0.0);
    mode.decoder.candidates = candidates;
    let a = Transcriber { grid: Some(mode), ..Transcriber::default() }.transcribe(&PYin::default(), audio, SR, BPM, Producer::Pyin);
    a.notes.iter().map(|n| ((n.onset_s / 0.1).round() as i64, (n.offset_s / 0.1).round() as i64, n.midi_pitch)).collect()
}

fn lead_line() -> Vec<(i64, i64, u8)> {
    LEAD.iter().enumerate().map(|(k, &m)| (2 + 4 * k as i64, 6 + 4 * k as i64, m)).collect()
}

#[test]
fn candidates_recover_a_lead_that_a_harmony_hides_from_pyin() {
    let audio = duet(0.3);
    let plain = decode(&audio, None);
    assert_ne!(plain, lead_line(), "the contour alone should fail here, or the test proves nothing");
    assert_eq!(decode(&audio, Some(CandidateEvidence::default())), lead_line(), "contour alone decoded {plain:?}");
}

/// Where only the lead sings, pYIN already gets it right; candidates must not
/// change the line.
#[test]
fn candidates_leave_a_solo_lead_as_the_contour_decodes_it() {
    let audio = duet(0.0);
    let plain = decode(&audio, None);
    assert!(lead_line().iter().all(|n| plain.contains(n)), "contour alone: {plain:?}");
    assert_eq!(decode(&audio, Some(CandidateEvidence::default())), plain);
}

#[test]
fn candidates_leave_silence_silent() {
    let mut mode = GridMode::sixteenths();
    mode.decoder.candidates = Some(CandidateEvidence::default());
    let a = Transcriber { grid: Some(mode), ..Transcriber::default() }.transcribe(&PYin::default(), &vec![0.0; SR as usize], SR, BPM, Producer::Pyin);
    assert!(a.notes.is_empty(), "{:?}", a.notes);
}
