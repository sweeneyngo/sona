//! **Frame metrics**: whether sona's timing frame (its bar lines and its tempo) matches a UST's.
//! These are the errors that cost most to fix in OpenUtau: every note in the wrong measures, or a
//! wrong BPM.
//!
//! Where sona puts tick 0: the exported project starts at the editor's grid phase, the first grid
//! line at or after audio time 0 (`web/lib/sequence.ts`, `gridPhase`). The phase comes from the
//! vocals' onsets (ADR-0027). sona never looks for beat 1 of a measure, so its bar 1 is wherever the
//! audio happens to start, give or take less than one cell.
//!
//! Definitions, all with 4/4 assumed (a UST has no time signature):
//! - **Bar offset:** with the UST's exact tempo, `(sona's tick 0 − the UST's tick 0) / beat`, taken
//!   mod 4 and split into whole beats (`0..4`, nearest) and a sub-beat residual in `[-0.5, 0.5)`.
//!   Positive means sona's bar 1 is late.
//! - **Tempo multiple:** estimated / true BPM within 2%: 1×, 2×, ½× or other.
//! - **BPM drift:** `|estimated − true| / true` × the song's length in beats at the true tempo.
//! - **Frame pass:** a 1× tempo, a bar offset of 0 beats and drift below a quarter beat.
//!
//! The UST's tick 0 is tied to the file, not to the music: Pifuyuu's bar lines match the music's on
//! 1/9 dev songs (`reports/bar-one.md`). So bar offset and frame pass are also scored **against the
//! music** when the manifest says where its beat and bar sit ([`MusicFrame`]); the UST-anchored
//! numbers stay for comparison.

use sona_core::tempo::{estimate_bpm, grid_phase};

use crate::quantize::grid_spacing_seconds;

pub const BEATS_PER_BAR: f64 = 4.0;
/// Relative tolerance of a tempo multiple, as in `sona-eval-tempo`.
pub const MULTIPLE_TOLERANCE: f64 = 0.02;
/// Drift below this many beats over the song passes.
pub const DRIFT_PASS_BEATS: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TempoMultiple {
    One,
    Double,
    Half,
    Other,
}

impl TempoMultiple {
    pub fn label(self) -> &'static str {
        match self {
            TempoMultiple::One => "1x",
            TempoMultiple::Double => "2x",
            TempoMultiple::Half => "1/2x",
            TempoMultiple::Other => "other",
        }
    }
}

/// The multiple an estimated tempo is of the true one.
pub fn tempo_multiple(estimated: f64, truth: f64) -> TempoMultiple {
    let near = |a: f64| (a - truth).abs() / truth <= MULTIPLE_TOLERANCE;
    if near(estimated) {
        TempoMultiple::One
    } else if near(estimated / 2.0) {
        TempoMultiple::Double
    } else if near(estimated * 2.0) {
        TempoMultiple::Half
    } else {
        TempoMultiple::Other
    }
}

/// Beats the estimated tempo's grid has slipped against the true one by the end of a song
/// `duration_s` long.
pub fn drift_beats(estimated: f64, truth: f64, duration_s: f64) -> f64 {
    (estimated - truth).abs() / truth * duration_s * truth / 60.0
}

/// Audio time of sona's tick 0 for a grid phase: the first grid line at or after time 0.
pub fn sona_tick0_s(phase_s: f64, spacing_s: f64) -> f64 {
    phase_s.rem_euclid(spacing_s)
}

/// Audio time of tick 0 under the "nearest grid line to 0 s" rule: the grid line closest to time 0,
/// which may sit up to half a cell before the audio starts. An alternative to [`sona_tick0_s`] for
/// the bar-one study (`reports/bar-one.md`); not what the export does.
pub fn nearest_tick0_s(phase_s: f64, spacing_s: f64) -> f64 {
    let p = phase_s.rem_euclid(spacing_s);
    if p > spacing_s / 2.0 {
        p - spacing_s
    } else {
        p
    }
}

/// How many cells the lines of a `period_s` grid through `t_s` sit after the lines of the same
/// grid through `reference_s`, in `[0, period_s / cell_s)`.
pub fn cells_after(t_s: f64, reference_s: f64, period_s: f64, cell_s: f64) -> f64 {
    (t_s - reference_s).rem_euclid(period_s) / cell_s
}

/// Audio time of sona's tick 0 for a vocals track at a given tempo and grid, as the cell stage and the
/// export place it. `None` without enough audio to hear onsets.
pub fn sona_tick0_from_audio(vocals: &[f32], sample_rate: u32, bpm: f64, division: u32) -> Option<f64> {
    let spacing = grid_spacing_seconds(bpm, f64::from(division));
    grid_phase(vocals, sample_rate, spacing).map(|p| sona_tick0_s(p, spacing))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarOffset {
    /// Whole beats sona's bar 1 sits after the UST's, mod 4, in `0..4`.
    pub beats: u32,
    /// What is left over, in beats, in `[-0.5, 0.5)`.
    pub residual_beats: f64,
}

/// Where sona's bar 1 sits relative to the UST's, both given as audio times, at the true tempo.
pub fn bar_offset(sona_tick0_s: f64, ust_tick0_s: f64, bpm: f64) -> BarOffset {
    let beats = ((sona_tick0_s - ust_tick0_s) * bpm / 60.0).rem_euclid(BEATS_PER_BAR);
    let whole = (beats + 0.5).floor();
    BarOffset { beats: whole.rem_euclid(BEATS_PER_BAR) as u32, residual_beats: beats - whole }
}

/// Where the music's beat and bar 1 sit against a UST's lines, in whole cells after them (manifest
/// `music_beat_shift_cells`, `music_bar_shift_cells`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicFrame {
    /// Cells from the UST's beat lines to the music's beat, in `0..cells per beat`.
    pub beat_shift_cells: u32,
    /// Cells from the UST's bar lines to the music's bar line, in `0..cells per bar`.
    pub bar_shift_cells: u32,
    /// Confirmed by ear (`music_frame_status: ear-checked`), not only estimated.
    pub ear_checked: bool,
}

impl MusicFrame {
    /// Audio time of one of the music's bar lines, given the UST's tick 0 in audio time.
    pub fn bar_line_s(&self, ust_tick0_s: f64, cell_s: f64) -> f64 {
        ust_tick0_s + f64::from(self.bar_shift_cells) * cell_s
    }

    /// Audio time of one of the music's beats, given the UST's tick 0 in audio time.
    pub fn beat_line_s(&self, ust_tick0_s: f64, cell_s: f64) -> f64 {
        ust_tick0_s + f64::from(self.beat_shift_cells) * cell_s
    }

    /// Whether the bar line falls on the beat: the bar shift is the beat shift plus whole beats.
    pub fn is_consistent(&self, cells_per_beat: u32) -> bool {
        cells_per_beat > 0
            && self.beat_shift_cells < cells_per_beat
            && self.bar_shift_cells < cells_per_beat * BEATS_PER_BAR as u32
            && self.bar_shift_cells % cells_per_beat == self.beat_shift_cells
    }
}

/// The ear labels of a holdout's music frames (utau-edit-cost 17): a local file the user fills while
/// listening to click overlays, never committed. `{"songs": {"<id>": {"meter": "4/4", "beat_shift_cells": 2,
/// "bar_shift_cells": 10, "confirmed": true}}}`; `meter` is `null` until the song is labelled, and a song
/// that is not in 4/4 (or that the labeller can't place) gets another word, e.g. `"other"`.
///
/// Returns the songs labelled `4/4`, as ear-checked frames. Songs not labelled yet or not in 4/4 are left
/// out (they are excluded before the read). A `4/4` song without both shifts is an error.
pub fn ear_labels(json: &str) -> Result<std::collections::BTreeMap<String, MusicFrame>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("labels: {e}"))?;
    let songs = v.get("songs").and_then(serde_json::Value::as_object).ok_or("labels: no \"songs\" object")?;
    let mut out = std::collections::BTreeMap::new();
    for (id, label) in songs {
        if label.get("meter").and_then(serde_json::Value::as_str) != Some("4/4") {
            continue;
        }
        let shift = |k: &str| {
            label.get(k).and_then(serde_json::Value::as_u64).and_then(|x| u32::try_from(x).ok()).ok_or(format!("labels: {id} is in 4/4 but has no whole {k}"))
        };
        out.insert(id.clone(), MusicFrame { beat_shift_cells: shift("beat_shift_cells")?, bar_shift_cells: shift("bar_shift_cells")?, ear_checked: true });
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub true_bpm: f64,
    pub estimated_bpm: Option<f64>,
    pub multiple: Option<TempoMultiple>,
    /// Only for a 1× estimate.
    pub drift_beats: Option<f64>,
    /// Against the UST's tick 0.
    pub bar_offset: Option<BarOffset>,
    /// Against the music's bar line; `None` without a [`MusicFrame`].
    pub music_bar_offset: Option<BarOffset>,
}

impl Frame {
    fn tempo_passes(&self) -> bool {
        self.multiple == Some(TempoMultiple::One) && self.drift_beats.is_some_and(|d| d < DRIFT_PASS_BEATS)
    }

    /// Frame pass against the UST's bar lines.
    pub fn passes(&self) -> bool {
        self.tempo_passes() && self.bar_offset.is_some_and(|b| b.beats == 0)
    }

    /// Frame pass against the music's bar lines; `None` without a [`MusicFrame`].
    pub fn passes_on_music(&self) -> Option<bool> {
        self.music_bar_offset.map(|b| self.tempo_passes() && b.beats == 0)
    }
}

/// A song's frame: tempo estimated from the full mix, bar 1 from the vocals at the true tempo.
/// `ust_tick0_s` is the audio time of the UST's tick 0 (its bar 1); `music` places the music's bar
/// against it.
#[allow(clippy::too_many_arguments)]
pub fn measure(
    mix: &[f32],
    mix_rate: u32,
    vocals: &[f32],
    vocals_rate: u32,
    true_bpm: f64,
    division: u32,
    ust_tick0_s: f64,
    music: Option<MusicFrame>,
) -> Frame {
    let estimated_bpm = estimate_bpm(mix, mix_rate);
    let multiple = estimated_bpm.map(|e| tempo_multiple(e, true_bpm));
    let duration_s = mix.len() as f64 / f64::from(mix_rate);
    let drift = match (estimated_bpm, multiple) {
        (Some(e), Some(TempoMultiple::One)) => Some(drift_beats(e, true_bpm, duration_s)),
        _ => None,
    };
    let tick0 = sona_tick0_from_audio(vocals, vocals_rate, true_bpm, division);
    let cell = grid_spacing_seconds(true_bpm, f64::from(division));
    Frame {
        true_bpm,
        estimated_bpm,
        multiple,
        drift_beats: drift,
        bar_offset: tick0.map(|t| bar_offset(t, ust_tick0_s, true_bpm)),
        music_bar_offset: tick0.zip(music).map(|(t, m)| bar_offset(t, m.bar_line_s(ust_tick0_s, cell), true_bpm)),
    }
}

/// Length of one click in an overlay.
pub const CLICK_S: f64 = 0.03;

/// Where an overlay clicks: every beat of a grid with a bar line at `bar_line_s`, within
/// `[0, duration_s)`, flagged `true` on bar lines.
pub fn click_times(bar_line_s: f64, beat_s: f64, duration_s: f64) -> Vec<(f64, bool)> {
    let first = ((0.0 - bar_line_s) / beat_s).ceil() as i64;
    let last = ((duration_s - bar_line_s) / beat_s).ceil() as i64;
    (first..last)
        .map(|k| (bar_line_s + k as f64 * beat_s, k.rem_euclid(BEATS_PER_BAR as i64) == 0))
        .filter(|&(t, _)| (0.0..duration_s).contains(&t))
        .collect()
}

/// `audio` with clicks on a candidate beat and bar grid ([`click_times`]), for checking a music
/// frame by ear: a high click on each bar line, a lower one on the other beats.
pub fn overlay_clicks(audio: &[f32], sample_rate: u32, bar_line_s: f64, beat_s: f64) -> Vec<f32> {
    let sr = f64::from(sample_rate);
    let mut out: Vec<f32> = audio.iter().map(|x| x * 0.6).collect();
    let click_len = (CLICK_S * sr) as usize;
    for (t, is_bar) in click_times(bar_line_s, beat_s, audio.len() as f64 / sr) {
        let (freq, gain) = if is_bar { (2000.0, 0.5) } else { (1000.0, 0.35) };
        let start = (t * sr).round() as usize;
        for (i, x) in out.iter_mut().skip(start).take(click_len).enumerate() {
            let s = i as f64 / sr;
            *x += (gain * (-s * 150.0).exp() * (2.0 * std::f64::consts::PI * freq * s).sin()) as f32;
        }
    }
    out
}

/// Where a tick 0 at `tick0_s` sits against the music's bar line, at the true tempo.
pub fn music_bar_offset(tick0_s: f64, ust_tick0_s: f64, bpm: f64, division: u32, music: MusicFrame) -> BarOffset {
    bar_offset(tick0_s, music.bar_line_s(ust_tick0_s, grid_spacing_seconds(bpm, f64::from(division))), bpm)
}

/// Song resamples in [`song_bootstrap`].
pub const BOOTSTRAP_ITERS: usize = 10_000;

/// 95% percentile interval of `Σ score` over songs resampled with replacement (fixed seed).
pub fn song_bootstrap(scores: &[f64]) -> (f64, f64) {
    if scores.is_empty() {
        return (0.0, 0.0);
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut draws: Vec<f64> = (0..BOOTSTRAP_ITERS).map(|_| (0..scores.len()).map(|_| scores[(next() % scores.len() as u64) as usize]).sum()).collect();
    draws.sort_by(f64::total_cmp);
    let q = |p: f64| draws[((draws.len() - 1) as f64 * p).round() as usize];
    (q(0.025), q(0.975))
}
