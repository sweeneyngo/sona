//! Onsets detected on a test vocal stem with few false positives from bleed,
//! and the energy envelope drops straight into the annotation JSON's
//! `energy_envelope` field (ADR-0004).

use sona_core::annotation::{Annotation, Producer, SCHEMA_VERSION};
use sona_core::onset::OnsetDetector;
use std::f32::consts::PI;

const SR: u32 = 44_100;

/// A short staccato "vocal stem": four notes at Vocaloid-ish spacing, each a
/// fundamental plus harmonics, separated by clean gaps — and, crucially, a bed
/// of low-level broadband **bleed** running underneath the whole thing (the UVR
/// separation artefacts this test is about). Deterministic, so the test is
/// stable.
fn vocal_stem_with_bleed() -> (Vec<f32>, Vec<f64>) {
    let note = |freq: f32, dur_s: f32| -> Vec<f32> {
        let n = (dur_s * SR as f32) as usize;
        (0..n)
            .map(|i| {
                let phase = 2.0 * PI * freq * i as f32 / SR as f32;
                0.6 * (0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin())
            })
            .collect()
    };
    let gap = |dur_s: f32| vec![0.0f32; (dur_s * SR as f32) as usize];

    // Build note-then-gap segments, remembering each note's true onset time.
    let pitches = [440.0f32, 523.25, 587.33, 440.0];
    let note_dur = 0.22f32;
    let gap_dur = 0.12f32;
    let lead = 0.1f32;

    let mut audio = gap(lead);
    let mut onsets = Vec::new();
    for &p in &pitches {
        onsets.push(audio.len() as f64 / SR as f64);
        audio.extend(note(p, note_dur));
        audio.extend(gap(gap_dur));
    }

    // Additive bleed: a deterministic LCG at ~3% amplitude, well under the
    // ~0.4 peak RMS of a note, running under the entire stem including the gaps.
    let mut state: u32 = 0x9E37_79B9;
    for s in audio.iter_mut() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let u = (state >> 8) as f32 / (1u32 << 24) as f32;
        *s += 0.03 * (2.0 * u - 1.0);
    }

    (audio, onsets)
}

#[test]
fn onsets_and_envelope_on_a_stem_with_bleed() {
    let (audio, true_onsets) = vocal_stem_with_bleed();
    let result = OnsetDetector::default().detect(&audio, SR);

    // Exactly one onset per note — no spurious attacks from the bleed bed or the
    // note offsets.
    assert_eq!(
        result.onsets.len(),
        true_onsets.len(),
        "expected one onset per note, got {:?}",
        result.onsets
    );
    for (onset, &truth) in result.onsets.iter().zip(&true_onsets) {
        assert!(
            (onset.time_s - truth).abs() < 0.04,
            "onset {} should land near the true onset {}",
            onset.time_s,
            truth
        );
        assert!(onset.strength > 0.0, "onset carries a positive strength");
    }
    // Onsets are in time order.
    for pair in result.onsets.windows(2) {
        assert!(pair[1].time_s > pair[0].time_s, "onsets time-ordered");
    }

    // The energy envelope is a usable display aid: non-empty, time-ordered,
    // non-negative RMS, and loud during a note vs. near the floor in a gap.
    let env = &result.energy_envelope;
    assert!(!env.is_empty(), "expected an energy envelope");
    for frame in env {
        assert!(frame.rms >= 0.0, "RMS is non-negative");
    }
    for pair in env.windows(2) {
        assert!(pair[1].time_s > pair[0].time_s, "envelope time-ordered");
    }
    let rms_at = |t: f64| {
        env.iter()
            .min_by(|a, b| {
                (a.time_s - t)
                    .abs()
                    .partial_cmp(&(b.time_s - t).abs())
                    .unwrap()
            })
            .map(|f| f.rms)
            .unwrap()
    };
    // Mid-first-note (0.1 + 0.11) is far louder than mid-first-gap (0.1 + 0.22 + 0.06).
    let in_note = rms_at(0.21);
    let in_gap = rms_at(0.38);
    assert!(
        in_note > in_gap * 3.0,
        "note RMS {in_note} should tower over gap RMS {in_gap}"
    );

    // The envelope is *exactly* the annotation's `energy_envelope` — no
    // adaptation needed — and survives the JSON round-trip (ADR-0004).
    let expected_len = env.len();
    let annotation = Annotation {
        version: SCHEMA_VERSION,
        sample_rate: SR,
        bpm: 120.0,
        duration_s: audio.len() as f64 / SR as f64,
        producer: Producer::Pyin,
        notes: vec![],
        f0_contour: vec![],
        energy_envelope: result.energy_envelope,
        provenance: None,
    };
    let json = serde_json::to_string(&annotation).expect("serialize");
    let round_tripped: Annotation = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped.energy_envelope.len(), expected_len);
    for (a, b) in annotation
        .energy_envelope
        .iter()
        .zip(&round_tripped.energy_envelope)
    {
        assert!((a.time_s - b.time_s).abs() < 1e-6);
        assert_eq!(a.rms, b.rms);
    }
}
