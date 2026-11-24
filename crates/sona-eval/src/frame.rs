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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub true_bpm: f64,
    pub estimated_bpm: Option<f64>,
    pub multiple: Option<TempoMultiple>,
    /// Only for a 1× estimate.
    pub drift_beats: Option<f64>,
    pub bar_offset: Option<BarOffset>,
}

impl Frame {
    pub fn passes(&self) -> bool {
        self.multiple == Some(TempoMultiple::One)
            && self.drift_beats.is_some_and(|d| d < DRIFT_PASS_BEATS)
            && self.bar_offset.is_some_and(|b| b.beats == 0)
    }
}

/// A song's frame: tempo estimated from the full mix, bar 1 from the vocals at the true tempo.
/// `ust_tick0_s` is the audio time of the UST's tick 0 (its bar 1).
pub fn measure(mix: &[f32], mix_rate: u32, vocals: &[f32], vocals_rate: u32, true_bpm: f64, division: u32, ust_tick0_s: f64) -> Frame {
    let estimated_bpm = estimate_bpm(mix, mix_rate);
    let multiple = estimated_bpm.map(|e| tempo_multiple(e, true_bpm));
    let duration_s = mix.len() as f64 / f64::from(mix_rate);
    let drift = match (estimated_bpm, multiple) {
        (Some(e), Some(TempoMultiple::One)) => Some(drift_beats(e, true_bpm, duration_s)),
        _ => None,
    };
    let bar = sona_tick0_from_audio(vocals, vocals_rate, true_bpm, division).map(|t| bar_offset(t, ust_tick0_s, true_bpm));
    Frame { true_bpm, estimated_bpm, multiple, drift_beats: drift, bar_offset: bar }
}
