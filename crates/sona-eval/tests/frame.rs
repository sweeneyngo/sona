//! Frame metrics on click tracks with a known tempo and a known bar 1.

use sona_eval::frame::{
    bar_offset, cells_after, click_times, drift_beats, ear_labels, measure, music_bar_offset, nearest_tick0_s, overlay_clicks, song_bootstrap, sona_tick0_from_audio,
    sona_tick0_s, tempo_multiple, MusicFrame, TempoMultiple, CLICK_S,
};
use sona_eval::quantize::grid_spacing_seconds;

const SR: u32 = 22050;

/// Eighth-note clicks (a low thump on the beat, a high tick between) starting `start_s` into the audio.
fn click_track(bpm: f64, seconds: f64, start_s: f64) -> Vec<f32> {
    accented_click_track(bpm, seconds, start_s, None)
}

/// [`click_track`] with the thump twice as loud on every 4th beat from beat `downbeat` (counting the
/// first click as beat 0): a bar the music has and the file's start does not.
fn accented_click_track(bpm: f64, seconds: f64, start_s: f64, downbeat: Option<i64>) -> Vec<f32> {
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
                    let index = (t / eighth).floor() as i64;
                    let accent = if downbeat.is_some_and(|d| index % 8 == (2 * d).rem_euclid(8)) { 2.0 } else { 1.0 };
                    let tone = if index % 2 == 0 {
                        accent * (2.0 * std::f64::consts::PI * 60.0 * t).sin()
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
fn the_nearest_rule_takes_a_grid_line_just_before_the_audio() {
    let cell = 0.1;
    assert!((nearest_tick0_s(0.03, cell) - 0.03).abs() < 1e-12);
    assert!((nearest_tick0_s(0.08, cell) + 0.02).abs() < 1e-12);
    assert!((nearest_tick0_s(-0.02, cell) + 0.02).abs() < 1e-12);
    assert!((nearest_tick0_s(0.38, cell) + 0.02).abs() < 1e-12);
    assert!((sona_tick0_s(0.08, cell) - 0.08).abs() < 1e-12);
}

#[test]
fn the_nearest_rule_on_a_click_track_lands_on_bar_one_before_the_audio() {
    let bpm = 180.0;
    let beat = 60.0 / bpm;
    let cell = beat / 4.0;
    let start = 0.8 * cell;
    let audio = click_track(bpm, 30.0, start);
    let phase = sona_core::tempo::grid_phase(&audio, SR, cell).unwrap();
    let bar_line_before = start - cell;
    assert!((nearest_tick0_s(phase, cell) - bar_line_before).abs() < 0.003, "phase {phase}");
    assert!((sona_tick0_s(phase, cell) - start).abs() < 0.003, "phase {phase}");
}

#[test]
fn cells_after_wraps_into_one_period() {
    let (beat, cell) = (0.4, 0.1);
    assert!((cells_after(0.1, 0.0, beat, cell) - 1.0).abs() < 1e-9);
    assert!((cells_after(-0.1, 0.0, beat, cell) - 3.0).abs() < 1e-9);
    assert!((cells_after(5.0 * beat + 0.2, 0.0, beat, cell) - 2.0).abs() < 1e-9);
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
    let on_time = measure(&audio, SR, &audio, SR, bpm, 16, start - 16.0 * beat, None);
    assert_eq!(on_time.multiple, Some(TempoMultiple::One));
    assert!(on_time.drift_beats.unwrap() < 0.25, "{on_time:?}");
    assert!(on_time.passes(), "{on_time:?}");
    let a_beat_late = measure(&audio, SR, &audio, SR, bpm, 16, start - 15.0 * beat, None);
    assert_eq!(a_beat_late.bar_offset.unwrap().beats, 3);
    assert!(!a_beat_late.passes());
    let slow = click_track(110.0, 30.0, start);
    let doubled = measure(&slow, SR, &slow, SR, 110.0, 16, start, None);
    assert_eq!(doubled.multiple, Some(TempoMultiple::Double));
    assert_eq!(doubled.drift_beats, None);
    assert!(!doubled.passes());
}

/// The music's bar sits `k` beats into the file while the UST's bar lines sit on the file's start (the
/// corpus's case, `reports/bar-one.md`). sona's tick 0 is the first click, so its bar 1 is `4 − k`
/// beats past the music's bar line: the music-anchored offset says so, the UST-anchored one says 0.
#[test]
fn the_music_s_bar_shifted_from_the_file_s_start() {
    for (bpm, start) in [(180.0, 0.05), (150.0, 0.07), (200.0, 0.03)] {
        let beat = 60.0 / bpm;
        for k in 1..4u32 {
            let audio = accented_click_track(bpm, 30.0, start, Some(i64::from(k)));
            let music = MusicFrame { beat_shift_cells: 0, bar_shift_cells: 4 * k, ear_checked: false };
            let f = measure(&audio, SR, &audio, SR, bpm, 16, start - 16.0 * beat, Some(music));
            let on_music = f.music_bar_offset.unwrap();
            assert_eq!(on_music.beats, 4 - k, "{bpm} BPM, music's bar {k} beats in: {f:?}");
            assert!(on_music.residual_beats.abs() < 0.02, "{bpm} BPM, music's bar {k} beats in: {f:?}");
            assert_eq!(f.bar_offset.unwrap().beats, 0, "the UST's lines sit on the file's start: {f:?}");
            assert_eq!(f.passes_on_music(), Some(false), "against the music: {f:?}");
            if bpm == 180.0 {
                assert!(f.passes(), "against the UST: {f:?}");
            }
        }
    }
}

/// As above, with the music's beat also a cell after the UST's beat lines (6/9 dev songs). sona's
/// tick 0 is the first cell line, a cell before the music's beat: a residual of −¼ beat against the
/// music, none against the UST.
#[test]
fn the_music_s_beat_and_bar_both_off_the_ust_s_lines() {
    let (bpm, start) = (180.0, 0.02);
    let beat = 60.0 / bpm;
    let cell = grid_spacing_seconds(bpm, 16.0);
    for k in 1..4u32 {
        let audio = accented_click_track(bpm, 30.0, start + cell, Some(i64::from(k)));
        let music = MusicFrame { beat_shift_cells: 1, bar_shift_cells: 4 * k + 1, ear_checked: false };
        assert!(music.is_consistent(4));
        let f = measure(&audio, SR, &audio, SR, bpm, 16, start - 16.0 * beat, Some(music));
        let (on_music, on_ust) = (f.music_bar_offset.unwrap(), f.bar_offset.unwrap());
        assert_eq!(on_music.beats, 4 - k, "music's bar {k} beats + 1 cell in: {f:?}");
        assert!((on_music.residual_beats + 0.25).abs() < 0.02, "music's bar {k} beats + 1 cell in: {f:?}");
        assert_eq!(on_ust.beats, 0, "{f:?}");
        assert!(on_ust.residual_beats.abs() < 0.02, "{f:?}");
    }
}

#[test]
fn the_music_s_bar_on_the_ust_s_bar_agrees_with_it() {
    let (bpm, start) = (150.0, 0.07);
    let audio = accented_click_track(bpm, 30.0, start, Some(0));
    let music = MusicFrame { beat_shift_cells: 0, bar_shift_cells: 0, ear_checked: true };
    let f = measure(&audio, SR, &audio, SR, bpm, 16, start - 16.0 * 60.0 / bpm, Some(music));
    assert_eq!(f.music_bar_offset, f.bar_offset);
    assert_eq!(f.passes_on_music(), Some(f.passes()));
    let without = measure(&audio, SR, &audio, SR, bpm, 16, start, None);
    assert_eq!(without.music_bar_offset, None);
    assert_eq!(without.passes_on_music(), None);
}

#[test]
fn music_lines_are_whole_cells_after_the_ust_s() {
    let m = MusicFrame { beat_shift_cells: 1, bar_shift_cells: 13, ear_checked: false };
    assert!((m.bar_line_s(-4.9, 0.1) - (-3.6)).abs() < 1e-9);
    assert!((m.beat_line_s(-4.9, 0.1) - (-4.8)).abs() < 1e-9);
    assert!(m.is_consistent(4));
    assert!(!MusicFrame { beat_shift_cells: 1, bar_shift_cells: 14, ear_checked: false }.is_consistent(4));
    assert!(!MusicFrame { beat_shift_cells: 1, bar_shift_cells: 17, ear_checked: false }.is_consistent(4));
    assert!(MusicFrame { beat_shift_cells: 2, bar_shift_cells: 20, ear_checked: false }.is_consistent(6));
}

#[test]
fn clicks_fall_on_every_beat_with_bar_lines_marked() {
    let got = click_times(0.3, 0.5, 2.9);
    let want = [(0.3, true), (0.8, false), (1.3, false), (1.8, false), (2.3, true), (2.8, false)];
    assert_eq!(got.len(), want.len(), "{got:?}");
    for ((t, bar), (wt, wbar)) in got.iter().zip(want) {
        assert!((t - wt).abs() < 1e-9 && *bar == wbar, "{got:?}");
    }
    let before = click_times(-4.7, 0.5, 1.5);
    assert_eq!(before.iter().map(|c| c.1).collect::<Vec<_>>(), [false, false, true], "{before:?}");
    assert!((before[0].0 - 0.3).abs() < 1e-9 && (before[2].0 - 1.3).abs() < 1e-9, "{before:?}");
}

/// The overlay's clicks are where [`click_times`] says, and sona's own phase finder hears them there.
#[test]
fn an_overlay_on_silence_clicks_on_the_candidate_grid() {
    let (bpm, bar_line) = (170.0, -3.21);
    let beat = 60.0 / bpm;
    let silence = vec![0.0f32; 20 * SR as usize];
    let out = overlay_clicks(&silence, SR, bar_line, beat);
    assert_eq!(out.len(), silence.len());
    let times = click_times(bar_line, beat, 20.0);
    let window = |t: f64| {
        let s = (t * f64::from(SR)).round() as usize;
        &out[s..(s + (CLICK_S * f64::from(SR)) as usize).min(out.len())]
    };
    let crossings = |x: &[f32]| x.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count();
    for &(t, bar) in &times {
        let w = window(t);
        assert!(w.iter().any(|x| x.abs() > 0.1), "no click at {t}");
        let hz = crossings(w) as f64 / 2.0 / CLICK_S;
        assert!((hz - if bar { 2000.0 } else { 1000.0 }).abs() < 150.0, "{t}: {hz} Hz, bar {bar}");
        assert!(window(t + beat / 2.0).iter().all(|x| *x == 0.0), "a click between beats at {t}");
    }
    let phase = sona_core::tempo::grid_phase(&out, SR, beat).unwrap();
    let off = cells_after(phase, bar_line, beat, beat);
    assert!(off.min(1.0 - off) < 0.05, "phase {phase}");
}

#[test]
fn a_tick_zero_whole_beats_after_the_music_s_bar_line_is_that_many_beats_off() {
    let (bpm, division, ust_tick0) = (180.0, 16, -4.9);
    let music = MusicFrame { beat_shift_cells: 1, bar_shift_cells: 9, ear_checked: false };
    let (beat, cell) = (60.0 / bpm, grid_spacing_seconds(bpm, 16.0));
    let bar_line = ust_tick0 + 9.0 * cell;
    for k in 0..8 {
        let got = music_bar_offset(bar_line + f64::from(k) * beat + 7.0 * 4.0 * beat, ust_tick0, bpm, division, music);
        assert_eq!(got.beats, k % 4, "{k} beats after: {got:?}");
        assert!(got.residual_beats.abs() < 1e-9, "{k} beats after: {got:?}");
    }
    let a_cell_late = music_bar_offset(bar_line + cell, ust_tick0, bpm, division, music);
    assert_eq!(a_cell_late.beats, 0);
    assert!((a_cell_late.residual_beats - 0.25).abs() < 1e-9, "{a_cell_late:?}");
}

#[test]
fn a_song_bootstrap_brackets_the_count_and_is_degenerate_when_every_song_agrees() {
    assert_eq!(song_bootstrap(&[1.0; 9]), (9.0, 9.0));
    assert_eq!(song_bootstrap(&[0.0; 9]), (0.0, 0.0));
    let (lo, hi) = song_bootstrap(&[1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    assert!((1.0..5.0).contains(&lo) && hi > 5.0 && hi <= 8.0, "[{lo}, {hi}]");
}

#[test]
fn ear_labels_give_ear_checked_frames_for_the_songs_labelled_in_four_four_only() {
    let labels = r#"{"format": "x", "songs": {
        "a": {"meter": "4/4", "beat_shift_cells": 2, "bar_shift_cells": 10, "confirmed": true},
        "b": {"meter": "other", "beat_shift_cells": null, "bar_shift_cells": null},
        "c": {"meter": null}
    }}"#;
    let frames = ear_labels(labels).unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames["a"], MusicFrame { beat_shift_cells: 2, bar_shift_cells: 10, ear_checked: true });
    assert!(ear_labels(r#"{"songs": {"a": {"meter": "4/4", "beat_shift_cells": 2}}}"#).is_err(), "a 4/4 label needs both shifts");
    assert!(ear_labels(r#"{"songs": {"a": {"meter": "4/4", "beat_shift_cells": -1, "bar_shift_cells": 3}}}"#).is_err());
    assert!(ear_labels(r#"{"labels": {}}"#).is_err());
}
