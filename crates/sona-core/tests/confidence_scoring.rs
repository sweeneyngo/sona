//! Ticket 06 done-criterion, driven by the *real* upstream stages: pYIN
//! (ticket 03) → energy onsets (ticket 04) → segmentation (ticket 05) →
//! confidence (ticket 06). Every note comes out carrying a scalar plus the four
//! sub-scores, and a clean staccato phrase scores high. Separation bleed is culled
//! upstream by the onset stage's bleed floor (ADR-0006), so the pipeline never
//! even emits a note for it — confidence's own bleed guard is a second line of
//! defence for a marginal note that slips through, and the spurious-bleed and
//! ambiguous-offset done-criteria are pinned as unit tests in the `confidence`
//! module, where a segment can be constructed at exactly the level under test.

use sona_core::confidence::ConfidenceScorer;
use sona_core::onset::OnsetDetector;
use sona_core::pitch::{PYin, PitchTracker};
use sona_core::segment::Segmenter;
use sona_core::Note;
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

/// Run the full core pipeline the way ticket 07 will: pYIN → onsets → segment →
/// confidence.
fn transcribe(audio: &[f32]) -> Vec<Note> {
    let contour = PYin::default().track(audio, SR);
    let onset_result = OnsetDetector::default().detect(audio, SR);
    let segments = Segmenter::default().segment(
        &onset_result.onsets,
        &contour,
        &onset_result.energy_envelope,
    );
    ConfidenceScorer::default().score_all(&segments, &contour, &onset_result.energy_envelope)
}

#[test]
fn clean_staccato_notes_score_high_and_carry_all_sub_scores() {
    // Three loud, steady, sharply-attacked-and-dropped notes — the target-material
    // easy case. Every one should be trusted, and every one carries the full
    // sub-score vector so the UI can explain any flag.
    let pitches = [440.0f32, 523.25, 659.25]; // A4, C5, E5
    let mut audio = silence(0.1);
    for &p in &pitches {
        audio.extend(tone(p, 0.25, 0.6));
        audio.extend(silence(0.15));
    }

    let notes = transcribe(&audio);
    assert_eq!(notes.len(), pitches.len(), "one note per attack: {notes:?}");

    for note in &notes {
        let s = &note.sub_scores;
        // Every sub-score is a real fraction in range.
        for v in [
            s.pitch_stability,
            s.voicing,
            s.onset_clarity,
            s.offset_clarity,
        ] {
            assert!((0.0..=1.0).contains(&v), "sub-score in range: {v}");
        }
        // The scalar is the weakest-link soft-min: at or below the strongest link,
        // never above it, and in range.
        assert!((0.0..=1.0).contains(&note.confidence));
        let max_sub = s
            .pitch_stability
            .max(s.voicing)
            .max(s.onset_clarity)
            .max(s.offset_clarity);
        assert!(
            note.confidence <= max_sub + 1e-4,
            "soft-min never exceeds the strongest sub-score: {} vs {max_sub}",
            note.confidence
        );
        // A clean note is trustworthy.
        assert!(
            note.confidence > 0.6,
            "clean staccato note is trusted, got {} from {:?}",
            note.confidence,
            s
        );
    }
}

#[test]
fn separation_bleed_never_becomes_a_trusted_note() {
    // A real, loud note, then a second "note" ~30 dB down — separation-bleed
    // level. The onset stage's bleed floor culls the buried one before it can
    // become a segment, so the pipeline emits only the loud note, and that note is
    // trusted. Bleed is handled by *not producing a spurious note*, not by
    // producing one and flagging it.
    let mut audio = silence(0.1);
    audio.extend(tone(440.0, 0.25, 0.6)); // loud reference
    audio.extend(silence(0.15));
    audio.extend(tone(523.25, 0.25, 0.02)); // ~30 dB down: bleed-level
    audio.extend(silence(0.15));

    let notes = transcribe(&audio);
    assert_eq!(
        notes.len(),
        1,
        "the buried note is culled upstream, only the loud note remains: {notes:?}"
    );
    assert_eq!(notes[0].midi_pitch, 69, "the surviving note is the loud A4");
    assert!(
        notes[0].confidence > 0.6,
        "and it is trusted, got {} from {:?}",
        notes[0].confidence,
        notes[0].sub_scores
    );
}
