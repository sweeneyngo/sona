//! Frame metrics on click tracks with a known tempo and a known bar 1.

use sona_eval::frame::{bar_offset, drift_beats, measure, sona_tick0_from_audio, tempo_multiple, TempoMultiple};

const SR: u32 = 22050;

/// Eighth-note clicks (a low thump on the beat, a high tick between) starting `start_s` into the audio.
fn click_track(bpm: f64, seconds: f64, start_s: f64) -> Vec<f32> {
    let n = (seconds * f64::from(SR)) as usize;
    let mut seed = 12345u32;
    (0..n)
        .map(|i| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let mut v = ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.01;
            let t = i as f64 / f64::from(SR) - start_s;
            if t >= 0.0 {
                let eighth = 30.0 / bpm;
                let since = t % eighth;
                if since < 0.03 {
                    let env = (-since * 120.0).exp();
                    let tone = if ((t / eighth).floor() as i64) % 2 == 0 {
                        (2.0 * std::f64::consts::PI * 60.0 * t).sin()
                    } else {
                        (2.0 * std::f64::consts::PI * 7000.0 * t).sin() * 0.4
                    };
                    v += (env * tone) as f32;
                }
            }
            v
        })
        .collect()
}

#[test]
fn bar_one_shifted_by_whole_beats_into_the_audio() {
    for (bpm, start) in [(180.0, 0.05), (150.0, 0.07), (200.0, 0.03)] {
        let audio = click_track(bpm, 30.0, start);
        let beat = 60.0 / bpm;
        let tick0 = sona_tick0_from_audio(&audio, SR, bpm, 16).unwrap();
        for shift in 0..4 {
            let ust_tick0 = start + f64::from(shift) * beat;
            let got = bar_offset(tick0, ust_tick0, bpm);
            assert_eq!(got.beats, (4 - shift) % 4, "{bpm} BPM, start {start}, bar 1 {shift} beats in: {got:?}");
            assert!(got.residual_beats.abs() < 0.02, "{bpm} BPM, start {start}, bar 1 {shift} beats in: {got:?}");
        }
    }
}

#[test]
fn bar_one_before_the_audio_starts() {
    for (bpm, start) in [(180.0, 0.05), (150.0, 0.07)] {
        let audio = click_track(bpm, 30.0, start);
        let beat = 60.0 / bpm;
        let tick0 = sona_tick0_from_audio(&audio, SR, bpm, 16).unwrap();
        for shift in 0..4 {
            let ust_tick0 = start - (16.0 + f64::from(shift)) * beat;
            let got = bar_offset(tick0, ust_tick0, bpm);
            assert_eq!(got.beats, shift, "{bpm} BPM, start {start}, bar 1 {shift} beats before: {got:?}");
            assert!(got.residual_beats.abs() < 0.02, "{bpm} BPM, start {start}, bar 1 {shift} beats before: {got:?}");
        }
    }
}

#[test]
fn a_sub_beat_offset_is_a_residual() {
    let got = bar_offset(0.25 * 0.4, 0.0, 150.0);
    assert_eq!(got.beats, 0);
    assert!((got.residual_beats - 0.25).abs() < 1e-9);
    let got = bar_offset(0.0, 0.25 * 0.4, 150.0);
    assert_eq!(got.beats, 0);
    assert!((got.residual_beats + 0.25).abs() < 1e-9);
    let got = bar_offset(0.0, 0.7 * 0.4, 150.0);
    assert_eq!(got.beats, 3);
    assert!((got.residual_beats - 0.3).abs() < 1e-9);
}

#[test]
fn tempo_multiples_and_drift() {
    assert_eq!(tempo_multiple(191.0, 190.0), TempoMultiple::One);
    assert_eq!(tempo_multiple(246.1, 123.0), TempoMultiple::Double);
    assert_eq!(tempo_multiple(95.0, 190.0), TempoMultiple::Half);
    assert_eq!(tempo_multiple(166.7, 125.0), TempoMultiple::Other);
    assert!((drift_beats(191.0, 190.0, 180.0) - 3.0).abs() < 1e-9);
    assert!((drift_beats(189.0, 190.0, 180.0) - 3.0).abs() < 1e-9);
    assert_eq!(drift_beats(190.0, 190.0, 180.0), 0.0);
}

#[test]
fn a_click_track_s_whole_frame() {
    let (bpm, start) = (180.0, 0.05);
    let audio = click_track(bpm, 30.0, start);
    let beat = 60.0 / bpm;
    let on_time = measure(&audio, SR, &audio, SR, bpm, 16, start - 16.0 * beat);
    assert_eq!(on_time.multiple, Some(TempoMultiple::One));
    assert!(on_time.drift_beats.unwrap() < 0.25, "{on_time:?}");
    assert!(on_time.passes(), "{on_time:?}");
    let a_beat_late = measure(&audio, SR, &audio, SR, bpm, 16, start - 15.0 * beat);
    assert_eq!(a_beat_late.bar_offset.unwrap().beats, 3);
    assert!(!a_beat_late.passes());
    let slow = click_track(110.0, 30.0, start);
    let doubled = measure(&slow, SR, &slow, SR, 110.0, 16, start);
    assert_eq!(doubled.multiple, Some(TempoMultiple::Double));
    assert_eq!(doubled.drift_beats, None);
    assert!(!doubled.passes());
}
