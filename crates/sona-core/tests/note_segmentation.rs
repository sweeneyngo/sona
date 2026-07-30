//! Segmentation yields musically plausible notes on a test stem, driven by the
//! *real* upstream stages — pYIN and the energy-onset detector — not hand-fed
//! tracks. Each note carries the raw material the confidence stage needs: onset
//! strength, and per offset a drop steepness and how the offset was decided.

use sona_core::onset::OnsetDetector;
use sona_core::pitch::{PYin, PitchTracker};
use sona_core::segment::{hz_to_midi, OffsetKind, Segmenter};
use std::f32::consts::PI;

const SR: u32 = 44_100;

/// A vowel-ish tone: fundamental plus two harmonics.
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

/// MIDI pitch of a frequency, for asserting the segmenter picked the right note.
/// Uses the core's own conversion so the test can't drift from it.
fn midi(hz: f32) -> u8 {
    hz_to_midi(hz).round() as u8
}

/// Run the full core pipeline: pYIN → onsets → segment.
fn transcribe(audio: &[f32]) -> Vec<sona_core::segment::NoteSegment> {
    let contour = PYin::default().track(audio, SR);
    let onset_result = OnsetDetector::default().detect(audio, SR);
    Segmenter::default().segment(&onset_result.onsets, &contour, &onset_result.energy_envelope)
}

#[test]
fn segments_a_staccato_phrase_into_plausible_notes() {
    // Three staccato notes with clean gaps — the target-material easy case. Each
    // is a distinct pitch, each followed by silence, so every offset is a sharp
    // energy drop into a rest (the energy-decay path).
    let pitches = [440.0f32, 523.25, 659.25]; // A4, C5, E5
    let note_dur = 0.25f32;
    let gap_dur = 0.15f32;

    let mut audio = silence(0.1);
    let mut starts = Vec::new();
    for &p in &pitches {
        starts.push(audio.len() as f64 / SR as f64);
        audio.extend(tone(p, note_dur, 0.6));
        audio.extend(silence(gap_dur));
    }

    let notes = transcribe(&audio);
    assert_eq!(
        notes.len(),
        pitches.len(),
        "one note per staccato attack: {:?}",
        notes.iter().map(|n| n.midi_pitch).collect::<Vec<_>>()
    );

    for (note, (&pitch, &start)) in notes.iter().zip(pitches.iter().zip(&starts)) {
        // Right pitch.
        assert_eq!(note.midi_pitch, midi(pitch), "note pitch");
        // Onset near the true attack.
        assert!(
            (note.onset_s - start).abs() < 0.05,
            "onset {} near true start {}",
            note.onset_s,
            start
        );
        // A real note has positive duration and closes before the next attack —
        // i.e. a rest was left, not swallowed.
        assert!(note.offset_s > note.onset_s, "positive duration");
        assert!(
            note.offset_s - note.onset_s < note_dur as f64 + 0.06,
            "note {} shouldn't stretch across the rest into the next",
            note.midi_pitch
        );
        // Staccato drop into silence: closed by energy decay, steep offset.
        assert_eq!(note.offset.kind, OffsetKind::EnergyDecay, "staccato → decay");
        assert!(
            note.offset.drop_steepness > 0.5,
            "sharp staccato drop scores high steepness, got {}",
            note.offset.drop_steepness
        );
        // Raw material for onset-clarity confidence is present.
        assert!(note.onset_strength > 0.0, "carries onset strength");
        assert!(!note.split_from_plateau, "born of an onset, not a split");
    }

    // Notes are in onset order and don't overlap.
    for pair in notes.windows(2) {
        assert!(
            pair[1].onset_s >= pair[0].offset_s,
            "notes are ordered and non-overlapping"
        );
    }
}

#[test]
fn splits_repeated_same_pitch_notes() {
    // Two A4 notes with a clean gap between them. Pure pitch tracking sees one
    // sustained pitch; the re-attack is an energy onset the note-HMM's energy
    // same-pitch splitter must turn into a second note at the *same* pitch — the
    // case pitch-only methods over-merge.
    let mut audio = silence(0.1);
    audio.extend(tone(440.0, 0.25, 0.6));
    audio.extend(silence(0.15));
    audio.extend(tone(440.0, 0.25, 0.6));
    audio.extend(silence(0.15));

    let notes = transcribe(&audio);
    assert_eq!(
        notes.len(),
        2,
        "two same-pitch attacks split into two notes: {:?}",
        notes.iter().map(|n| n.midi_pitch).collect::<Vec<_>>()
    );
    assert_eq!(notes[0].midi_pitch, midi(440.0));
    assert_eq!(notes[1].midi_pitch, midi(440.0));
    // Both are born of a real attack (not a pitch-born split), with a rest between.
    assert!(!notes[0].split_from_plateau && !notes[1].split_from_plateau);
    assert!(
        notes[1].onset_s > notes[0].offset_s,
        "a rest separates the two notes, got {} then {}",
        notes[0].offset_s,
        notes[1].onset_s
    );
}

#[test]
fn splits_a_legato_slur_with_no_re_attack() {
    // Two slurred pitches under one breath: 440 Hz then 523.25 Hz, no silence
    // between them, so there is no energy transient at the change — onset
    // detection sees one attack. The pitch-plateau step must still recover two
    // notes (spec stage 3, the case onsets alone miss).
    let mut audio = silence(0.1);
    let change_at = 0.1 + 0.3;
    audio.extend(tone(440.0, 0.3, 0.6));
    audio.extend(tone(523.25, 0.3, 0.6));
    audio.extend(silence(0.15));

    let notes = transcribe(&audio);
    assert_eq!(
        notes.len(),
        2,
        "legato slur splits into two notes: {:?}",
        notes.iter().map(|n| n.midi_pitch).collect::<Vec<_>>()
    );
    assert_eq!(notes[0].midi_pitch, midi(440.0));
    assert_eq!(notes[1].midi_pitch, midi(523.25));

    // The first note is the onset's; the second is a plateau split with no
    // transient of its own, closed seamlessly where the pitch changed.
    assert!(!notes[0].split_from_plateau);
    assert!(notes[1].split_from_plateau);
    assert_eq!(notes[0].offset.kind, OffsetKind::PlateauSplit);
    assert_eq!(notes[1].onset_strength, 0.0);
    assert!(
        (notes[0].offset_s - notes[1].onset_s).abs() < 1e-9,
        "seamless split"
    );
    assert!(
        (notes[0].offset_s - change_at).abs() < 0.05,
        "split lands near the pitch change {change_at}, got {}",
        notes[0].offset_s
    );
}
