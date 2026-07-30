//! pYIN produces a clean f0 contour + voicing on a test vocal stem, and that
//! contour drops straight into the annotation JSON's `f0_contour` field
//! (ADR-0004).

use sona_core::annotation::{Annotation, Producer, SCHEMA_VERSION};
use sona_core::pitch::{PYin, PitchTracker};
use std::f32::consts::PI;

const SR: u32 = 44_100;

/// A short "vocal stem": a sung A4 (440 Hz) with gentle vibrato and a couple of
/// harmonics, a breath (rest), then a C5 (523.25 Hz). Stands in for a real
/// isolated stem so the test is deterministic.
fn vocal_stem() -> Vec<f32> {
    let note = |freq: f32, dur_s: f32, vibrato: bool| -> Vec<f32> {
        let n = (dur_s * SR as f32) as usize;
        // Accumulate phase from the *instantaneous* frequency: for an FM tone
        // the phase is the integral of frequency, so phase must step by
        // `2π·f(t)/sr` each sample (using `2π·f(t)·t` would inject a spurious
        // `t·f'(t)` term that swings the true pitch far past the vibrato depth).
        let mut phase = 0.0f32;
        (0..n)
            .map(|i| {
                let t = i as f32 / SR as f32;
                // ~5.5 Hz, ±3% (~±52 cents) vibrato — realistic depth, and a
                // slow ripple the tracker should ride as one note, not split.
                let f = if vibrato {
                    freq * (1.0 + 0.03 * (2.0 * PI * 5.5 * t).sin())
                } else {
                    freq
                };
                phase += 2.0 * PI * f / SR as f32;
                0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin()
            })
            .collect()
    };

    let mut audio = note(440.0, 0.4, true);
    audio.extend(vec![0.0f32; (0.15 * SR as f32) as usize]); // breath
    audio.extend(note(523.25, 0.4, true));
    audio
}

#[test]
fn pyin_contour_feeds_the_annotation() {
    let audio = vocal_stem();
    let contour = PYin::default().track(&audio, SR);

    // A clean contour: frames in time order, voicing in range, voiced frames
    // carry a positive f0 and unvoiced frames don't.
    assert!(!contour.is_empty(), "expected a contour");
    for pair in contour.windows(2) {
        assert!(
            pair[1].time_s >= pair[0].time_s,
            "frames must be time-ordered"
        );
    }
    for frame in &contour {
        assert!((0.0..=1.0).contains(&frame.voicing), "voicing in [0,1]");
        if frame.voicing >= 0.5 {
            assert!(frame.frequency_hz > 0.0, "voiced frame has an f0");
        } else {
            assert_eq!(frame.frequency_hz, 0.0, "unvoiced frame has no f0");
        }
    }

    // Both sung pitches show up, with an unvoiced breath between them.
    let near = |target: f32| {
        contour
            .iter()
            .any(|f| f.voicing >= 0.5 && (f.frequency_hz - target).abs() / target < 0.03)
    };
    // The vibrato'd A4 must read as *one* note: across its sustained region the
    // contour stays coherent — no median-surviving octave error or spurious
    // jump that segmentation would split on. It is not *flattened*,
    // though (ADR-0005): the vibrato ripple is preserved as a display aid, so
    // some frame still deviates from the center.
    // Stop short of 0.4 s: frames whose ~46 ms analysis window straddles the
    // note→breath boundary would read a mixed pitch, unrelated to smoothing.
    let a4_region: Vec<f32> = contour
        .iter()
        .filter(|f| f.time_s < 0.35 && f.voicing >= 0.5)
        .map(|f| f.frequency_hz)
        .collect();
    assert!(a4_region.len() > 10, "expected a sustained A4 region");
    assert!(
        a4_region
            .iter()
            .all(|&hz| (hz - 440.0).abs() / 440.0 < 0.08),
        "A4 contour must stay coherent (within ~one tone), no octave jumps: {a4_region:?}"
    );
    assert!(
        a4_region
            .iter()
            .any(|&hz| (hz - 440.0).abs() / 440.0 > 0.01),
        "vibrato should be preserved in the contour, not flattened"
    );

    assert!(near(440.0), "A4 should appear in the contour");
    assert!(near(523.25), "C5 should appear in the contour");
    assert!(
        contour.iter().any(|f| f.voicing < 0.5),
        "breath should be unvoiced"
    );

    // The contour is *exactly* the JSON's `f0_contour` — no adaptation needed:
    // a tracker's output drops straight into an annotation and serializes.
    let expected_len = contour.len();
    let annotation = Annotation {
        version: SCHEMA_VERSION,
        sample_rate: SR,
        bpm: 120.0,
        duration_s: audio.len() as f64 / SR as f64,
        producer: Producer::Pyin,
        notes: vec![],
        f0_contour: contour,
        energy_envelope: vec![],
    };
    assert!(!annotation.f0_contour.is_empty());

    let json = serde_json::to_string(&annotation).expect("serialize");
    let round_tripped: Annotation = serde_json::from_str(&json).expect("deserialize");
    // The contour survives the wire. (Bit-exact f64 round-trip is a serde_json
    // parser-config detail of the annotation contract, not asserted here — the
    // UI consumes the JSON in JS, which parses f64 natively.)
    assert_eq!(round_tripped.f0_contour.len(), expected_len);
    for (a, b) in annotation.f0_contour.iter().zip(&round_tripped.f0_contour) {
        assert!((a.time_s - b.time_s).abs() < 1e-6);
        assert_eq!(a.frequency_hz, b.frequency_hz);
        assert_eq!(a.voicing, b.voicing);
    }
}
