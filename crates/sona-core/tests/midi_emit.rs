//! The optional raw, unquantized MIDI the CLI can emit as a convenience
//! (spec: *not* the authoritative export — that's the UI's job). The
//! annotation's note times in seconds are placed on a timed MIDI track through
//! the carried BPM, so the MIDI is raw (unquantized) but audible.

use midly::{Format, MetaMessage, MidiMessage, Timing, TrackEventKind};
use sona_core::midi::{to_smf, TICKS_PER_QUARTER};
use sona_core::{Annotation, Note, Producer, SubScores, SCHEMA_VERSION};

fn note(onset_s: f64, offset_s: f64, midi_pitch: u8) -> Note {
    Note {
        onset_s,
        offset_s,
        midi_pitch,
        confidence: 0.9,
        sub_scores: SubScores {
            pitch_stability: 0.9,
            voicing: 0.9,
            onset_clarity: 0.9,
            offset_clarity: 0.9,
        },
    }
}

fn annotation(bpm: f64, notes: Vec<Note>) -> Annotation {
    Annotation {
        version: SCHEMA_VERSION,
        sample_rate: 44_100,
        bpm,
        duration_s: 2.0,
        producer: Producer::Pyin,
        notes,
        f0_contour: vec![],
        energy_envelope: vec![],
        provenance: None,
        bar_one: None,
    }
}

/// Absolute-tick note-on / note-off events recovered from the single track by
/// accumulating deltas — the shape a MIDI reader would see.
fn note_events(smf: &midly::Smf) -> Vec<(u32, bool, u8)> {
    let mut out = Vec::new();
    let mut abs = 0u32;
    for ev in &smf.tracks[0] {
        abs += ev.delta.as_int();
        if let TrackEventKind::Midi { message, .. } = ev.kind {
            match message {
                MidiMessage::NoteOn { key, vel } => out.push((abs, vel.as_int() > 0, key.as_int())),
                MidiMessage::NoteOff { key, .. } => out.push((abs, false, key.as_int())),
                _ => {}
            }
        }
    }
    out
}

#[test]
fn emits_a_single_metrical_track() {
    let smf = to_smf(&annotation(120.0, vec![note(0.0, 0.5, 69)]));
    assert_eq!(smf.header.format, Format::SingleTrack);
    assert_eq!(
        smf.header.timing,
        Timing::Metrical(TICKS_PER_QUARTER.into())
    );
    assert_eq!(smf.tracks.len(), 1);
}

#[test]
fn carries_the_bpm_as_a_tempo_meta() {
    let smf = to_smf(&annotation(120.0, vec![note(0.0, 0.5, 69)]));
    // 120 BPM -> 500_000 microseconds per quarter note.
    let tempo = smf.tracks[0].iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::Tempo(us)) => Some(us.as_int()),
        _ => None,
    });
    assert_eq!(tempo, Some(500_000));
}

#[test]
fn places_notes_at_unquantized_tick_times() {
    // At 120 BPM a quarter note is 0.5 s = TICKS_PER_QUARTER ticks. A note from
    // 0.5 s to 1.0 s therefore spans one quarter, starting one quarter in.
    let smf = to_smf(&annotation(120.0, vec![note(0.5, 1.0, 69)]));
    let events = note_events(&smf);
    let tpq = TICKS_PER_QUARTER as u32;
    assert_eq!(
        events,
        vec![(tpq, true, 69), (2 * tpq, false, 69)],
        "one quarter in, one quarter long"
    );
}

#[test]
fn overlapping_ticks_close_before_they_open() {
    // Two back-to-back notes where the first's offset coincides with the second's
    // onset: the note-off must be emitted before the note-on at that tick so the
    // notes don't stack.
    let smf = to_smf(&annotation(
        120.0,
        vec![note(0.0, 0.5, 69), note(0.5, 1.0, 71)],
    ));
    let events = note_events(&smf);
    let tpq = TICKS_PER_QUARTER as u32;
    assert_eq!(
        events,
        vec![
            (0, true, 69),
            (tpq, false, 69),
            (tpq, true, 71),
            (2 * tpq, false, 71),
        ]
    );
}

#[test]
fn no_notes_still_produces_a_valid_track() {
    // An empty annotation (silent stem) yields a well-formed, note-free track.
    let smf = to_smf(&annotation(120.0, vec![]));
    assert_eq!(smf.tracks.len(), 1);
    assert!(note_events(&smf).is_empty());
    // Ends cleanly.
    assert!(matches!(
        smf.tracks[0].last().map(|ev| ev.kind),
        Some(TrackEventKind::Meta(MetaMessage::EndOfTrack))
    ));
}
