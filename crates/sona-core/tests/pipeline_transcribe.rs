//! Ticket 07 seam: the perception stages, wired end to end by [`Transcriber`],
//! turn raw mono audio into a complete, valid annotation. This is the core half
//! of the CLI's `stem + BPM -> annotation JSON` path — the CLI adds only WAV
//! decoding and serialization on top.

use sona_core::{PYin, Producer, Transcriber, SCHEMA_VERSION};
use std::f32::consts::PI;

const SR: u32 = 44_100;

/// A vowel-ish tone: fundamental plus two harmonics (matches the onset stage's
/// fixture), so pYIN and the energy detector both have real material to bite on.
fn tone(freq: f32, dur_s: f32, amp: f32) -> Vec<f32> {
    let n = (dur_s * SR as f32) as usize;
    (0..n)
        .map(|i| {
            let phase = 2.0 * PI * freq * i as f32 / SR as f32;
            amp * (0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin())
        })
        .collect()
}

fn silence(dur_s: f32) -> Vec<f32> {
    vec![0.0f32; (dur_s * SR as f32) as usize]
}

/// Two staccato notes (A4 then C5) with clean gaps — the target-material easy
/// case — should transcribe end to end into a complete annotation.
#[test]
fn transcribes_two_staccato_notes_end_to_end() {
    let mut audio = silence(0.15);
    audio.extend(tone(440.0, 0.3, 0.6)); // A4
    audio.extend(silence(0.15));
    audio.extend(tone(523.25, 0.3, 0.6)); // C5
    audio.extend(silence(0.15)); // trailing rest so the last note's offset drops

    let annotation =
        Transcriber::default().transcribe(&PYin::default(), &audio, SR, 128.0, Producer::Pyin);

    // Metadata is carried through verbatim; the core does not touch the BPM.
    assert_eq!(annotation.version, SCHEMA_VERSION);
    assert_eq!(annotation.producer, Producer::Pyin);
    assert_eq!(annotation.bpm, 128.0);
    assert_eq!(annotation.sample_rate, SR);
    let expected_dur = audio.len() as f64 / SR as f64;
    assert!((annotation.duration_s - expected_dur).abs() < 1e-9);

    // Display aids are populated.
    assert!(!annotation.f0_contour.is_empty(), "f0 contour emitted");
    assert!(!annotation.energy_envelope.is_empty(), "envelope emitted");

    // Two attacks -> two notes, in onset order, at roughly the right pitches.
    assert_eq!(annotation.notes.len(), 2, "notes: {:?}", annotation.notes);
    assert!(
        (annotation.notes[0].midi_pitch as i32 - 69).abs() <= 1,
        "first note ~A4, got {}",
        annotation.notes[0].midi_pitch
    );
    assert!(
        (annotation.notes[1].midi_pitch as i32 - 72).abs() <= 1,
        "second note ~C5, got {}",
        annotation.notes[1].midi_pitch
    );
    assert!(annotation.notes[0].onset_s < annotation.notes[1].onset_s);
    // Clean, loud, in-tune notes over silence should be trusted.
    for note in &annotation.notes {
        assert!(
            note.confidence > 0.5,
            "clean note should score high, got {} from {:?}",
            note.confidence,
            note.sub_scores
        );
    }
}

/// The emitted annotation serializes to JSON the CLI can print and a consumer
/// can read back — the same contract the web review surface loads. (Bit-exact
/// f64 round-trip is the ticket-02 contract test's job, with clean values; a
/// pipeline contour of computed floats trips serde_json's ~1-ULP parse quirk, so
/// here we assert the payload reads back intact, not bit-identical.)
#[test]
fn emitted_annotation_serializes_and_reads_back() {
    let mut audio = silence(0.1);
    audio.extend(tone(440.0, 0.3, 0.6));
    audio.extend(silence(0.1));
    let annotation =
        Transcriber::default().transcribe(&PYin::default(), &audio, SR, 120.0, Producer::Pyin);

    let json = serde_json::to_string(&annotation).expect("serialize");
    let decoded: sona_core::Annotation = serde_json::from_str(&json).expect("deserialize");

    // Metadata is carried exactly; note events survive the round trip.
    assert_eq!(decoded.version, annotation.version);
    assert_eq!(decoded.producer, annotation.producer);
    assert_eq!(decoded.bpm, annotation.bpm);
    assert_eq!(decoded.sample_rate, annotation.sample_rate);
    assert_eq!(decoded.notes.len(), annotation.notes.len());
    assert_eq!(
        decoded
            .notes
            .iter()
            .map(|n| n.midi_pitch)
            .collect::<Vec<_>>(),
        annotation
            .notes
            .iter()
            .map(|n| n.midi_pitch)
            .collect::<Vec<_>>(),
    );
    assert_eq!(decoded.f0_contour.len(), annotation.f0_contour.len());
    assert_eq!(
        decoded.energy_envelope.len(),
        annotation.energy_envelope.len()
    );
}

/// A silent stem has no onsets, so no notes — but the annotation is still
/// complete and valid (empty note list, populated envelope).
#[test]
fn silent_stem_yields_no_notes_but_a_valid_annotation() {
    let audio = silence(0.5);
    let annotation =
        Transcriber::default().transcribe(&PYin::default(), &audio, SR, 100.0, Producer::Pyin);
    assert!(annotation.notes.is_empty(), "silence has no notes");
    assert!(!annotation.energy_envelope.is_empty());
}
