//! Bar 1 from the music (`sona_core::bar_one`) on synthetic songs whose beat and downbeat are known:
//! a drum hit on every beat, a chord pad that changes on the bar line, and a louder sung line whose own
//! pitch changes fall on another beat of the bar.
#![cfg(feature = "spectral")]

use sona_core::bar_one::{beat_line, downbeat_vote, estimate, first_beat, tracked_bar, MIN_CONFIDENCE};
use std::f64::consts::PI;

const SR: u32 = 22050;
const CHORDS: [[f64; 4]; 4] = [[130.81, 261.63, 329.63, 392.0], [110.0, 220.0, 261.63, 329.63], [174.61, 349.23, 440.0, 523.25], [196.0, 392.0, 493.88, 587.33]];
const SUNG: [f64; 3] = [659.25, 783.99, 880.0];

struct Song {
    mix: Vec<f32>,
    vocals: Vec<f32>,
}

fn add_tone(buf: &mut [f32], start_s: f64, dur_s: f64, freqs: &[f64], amp: f64, decay: f64) {
    let a = (start_s.max(0.0) * f64::from(SR)) as usize;
    let b = (((start_s + dur_s) * f64::from(SR)) as usize).min(buf.len());
    for (i, v) in buf.iter_mut().enumerate().take(b).skip(a) {
        let t = i as f64 / f64::from(SR) - start_s;
        let env = (t / 0.002).min(1.0) * ((dur_s - t) / 0.01).clamp(0.0, 1.0) * (-t * decay).exp();
        *v += (amp * env * freqs.iter().map(|f| (2.0 * PI * f * t).sin()).sum::<f64>() / freqs.len() as f64) as f32;
    }
}

fn add_drum(buf: &mut [f32], at_s: f64, seed: &mut u32) {
    let a = (at_s.max(0.0) * f64::from(SR)) as usize;
    for (i, v) in buf.iter_mut().enumerate().skip(a).take((0.08 * f64::from(SR)) as usize) {
        let t = i as f64 / f64::from(SR) - at_s;
        *seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        let noise = (*seed >> 8) as f64 / f64::from(1u32 << 24) - 0.5;
        *v += (0.9 * (-t * 40.0).exp() * (2.0 * PI * 60.0 * t).sin() + 0.8 * (-t * 60.0).exp() * noise) as f32;
    }
}

/// A song at `bpm` whose first beat is `first_beat_s` into the audio. `downbeats[b]` is the beat of
/// the bar (counting the first beat as 0) on which block `b`'s chords change; a block is 8 bars.
fn song(bpm: f64, first_beat_s: f64, downbeats: &[usize], sung_on: usize) -> Song {
    let beat = 60.0 / bpm;
    let beats = downbeats.len() * 32 + 8;
    let n = ((first_beat_s + beats as f64 * beat + 1.0) * f64::from(SR)) as usize;
    let mut acc = vec![0.0f32; n];
    let mut vocals = vec![0.0f32; n];
    let mut seed = 7u32;
    for j in 0..beats {
        add_drum(&mut acc, first_beat_s + j as f64 * beat, &mut seed);
    }
    for (b, &down) in downbeats.iter().enumerate() {
        for bar in -1..8i64 {
            let start = (b as i64 * 32 + down as i64 + 4 * bar) as f64;
            let chord = CHORDS[((b as i64 * 8 + bar).rem_euclid(4)) as usize];
            add_tone(&mut acc, first_beat_s + start * beat, 4.0 * beat, &chord, 0.25, 0.0);
        }
    }
    for e in 0..2 * beats {
        let at = first_beat_s + e as f64 * beat / 2.0;
        let beat_no = e / 2;
        let held = ((beat_no + 4 - sung_on % 4) / 4) % SUNG.len();
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        if seed >> 30 != 0 {
            add_tone(&mut vocals, at, beat / 2.0 * 0.8, &[SUNG[held]], 0.6, 12.0);
        }
    }
    let mix = acc.iter().zip(&vocals).map(|(a, v)| a + v).collect();
    Song { mix, vocals }
}

fn off_by(t: f64, want: f64, period: f64) -> f64 {
    let d = (t - want).rem_euclid(period);
    d.min(period - d)
}

#[test]
fn finds_the_beat_and_the_chords_bar_line_not_the_singer_s() {
    for (bpm, first, down) in [(150.0, 0.13, 1), (200.0, 0.02, 3), (180.0, 0.31, 2), (240.0, 0.07, 0)] {
        let beat = 60.0 / bpm;
        let s = song(bpm, first, &[down; 3], down + 2);
        let got = estimate(&s.mix, &s.vocals, SR, bpm, 16).unwrap();
        assert!(off_by(got.beat_s, first, beat) < 0.003, "{bpm} BPM: beat {} for {first}", got.beat_s);
        assert!((0.0..beat).contains(&got.beat_s), "{bpm} BPM: beat {} is not the first at or after 0 s", got.beat_s);
        let want_bar = first + down as f64 * beat;
        assert!(off_by(got.bar_s, want_bar, 4.0 * beat) < 0.003, "{bpm} BPM, downbeat on beat {down}: bar {} for {want_bar}, {got:?}", got.bar_s);
        assert!((0.0..4.0 * beat).contains(&got.bar_s), "{bpm} BPM: bar {} is not the first at or after 0 s", got.bar_s);
        assert!(got.from_harmony() && got.confidence >= 0.99, "{bpm} BPM: {got:?}");
    }
}

#[test]
fn falls_back_to_the_beat_when_the_song_s_stretches_disagree_on_the_bar() {
    let (bpm, first) = (200.0, 0.11);
    let beat = 60.0 / bpm;
    let s = song(bpm, first, &[1, 2, 3], 0);
    let got = estimate(&s.mix, &s.vocals, SR, bpm, 16).unwrap();
    assert!(got.confidence < MIN_CONFIDENCE, "{got:?}");
    assert!(!got.from_harmony());
    assert!(off_by(got.bar_s, first, beat) < 0.003 && (got.bar_s - got.beat_s).abs() < 1e-9, "{got:?}");
}

#[test]
fn a_beat_lands_on_the_singer_s_cell_line_nearest_the_drums() {
    let (cell, beat) = (0.1, 0.4);
    assert!((beat_line(0.23, 0.02, cell, beat) - 0.22).abs() < 1e-9);
    assert!((beat_line(0.27, 0.02, cell, beat) - 0.32).abs() < 1e-9);
    assert!((beat_line(0.395, 0.0, cell, beat) - 0.0).abs() < 1e-9);
    assert!((beat_line(0.01, 0.09, cell, beat) - 0.39).abs() < 1e-9);
}

#[test]
fn says_nothing_without_audio_or_tempo() {
    assert_eq!(estimate(&[], &[], SR, 150.0, 16), None);
    let s = song(150.0, 0.1, &[0], 2);
    assert_eq!(estimate(&s.mix, &s.vocals, SR, 0.0, 16), None);
    assert_eq!(estimate(&s.mix, &s.vocals, SR, 150.0, 0), None);
}

#[test]
fn a_tracker_s_downbeats_put_bar_one_on_the_beat_they_fall_on() {
    for (bpm, first, down) in [(150.0, 0.13, 1), (200.0, 0.02, 3), (180.0, 0.31, 2), (240.0, 0.07, 0)] {
        let beat = 60.0 / bpm;
        let s = song(bpm, first, &[down; 3], down + 2);
        let beat_s = first_beat(&s.mix, &s.vocals, SR, bpm, 16).unwrap();
        let downbeats: Vec<f64> = (0..24).map(|bar| first + (down + 4 * bar) as f64 * beat + 0.01 * ((bar % 3) as f64 - 1.0)).collect();
        let got = tracked_bar(beat_s, bpm, &downbeats);
        assert!(off_by(got.bar_s, first + down as f64 * beat, 4.0 * beat) < 0.003, "{bpm} BPM, downbeat on beat {down}: {got:?}");
        assert!((0.0..4.0 * beat).contains(&got.bar_s), "{bpm} BPM: bar {} is not the first at or after 0 s", got.bar_s);
        assert_eq!((got.bar_beat, got.share), (Some(down), 1.0), "{bpm} BPM: {got:?}");
    }
}

#[test]
fn the_vote_is_over_four_beat_bars_and_its_share_is_the_confidence() {
    let (reference, beat) = (0.1, 0.5);
    let on = |beats: i64| reference + beats as f64 * beat;
    let half_tempo: Vec<f64> = (0..10).map(|k| on(1 + 8 * k)).collect();
    assert_eq!(downbeat_vote(&half_tempo, reference, beat), Some((1, 1.0)));
    let mut mixed: Vec<f64> = (0..10).map(|k| on(2 + 4 * k)).collect();
    mixed.extend((0..3).map(|k| on(3 + 4 * k)));
    let got = tracked_bar(reference, 120.0, &mixed);
    assert_eq!(got.bar_beat, Some(2), "{got:?}");
    assert!((got.share - 10.0 / 13.0).abs() < 1e-12 && (got.bar_s - on(2)).abs() < 1e-12, "{got:?}");
}

#[test]
fn a_pickup_before_the_first_beat_still_votes_for_its_bar() {
    assert_eq!(downbeat_vote(&[0.5, 2.5, 4.5], 1.0, 0.5), Some((3, 1.0)));
}

#[test]
fn a_tied_vote_goes_to_the_bar_voted_for_first() {
    assert_eq!(downbeat_vote(&[1.5, 0.5, 3.5, 2.5], 0.0, 0.5), Some((3, 0.5)));
    assert_eq!(downbeat_vote(&[0.5, 1.5, 2.5, 3.5], 0.0, 0.5), Some((1, 0.5)));
}

#[test]
fn without_downbeats_bar_one_is_the_beat_with_no_confidence() {
    assert_eq!(downbeat_vote(&[], 0.2, 0.5), None);
    assert_eq!(downbeat_vote(&[f64::NAN], 0.2, 0.5), None);
    let got = tracked_bar(0.2, 120.0, &[]);
    assert_eq!((got.bar_s, got.bar_beat, got.share), (0.2, None, 0.0));
}
