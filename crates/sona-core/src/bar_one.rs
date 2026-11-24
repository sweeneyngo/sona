//! **Bar 1 from the music** (utau-edit-cost 05, `reports/bar-one.md`): where the song's beat and the
//! first beat of its measures fall, at a BPM the user gave, so an export's bar lines are the music's.
//!
//! - **Beat:** the vocals' cell line nearest [`grid_phase`] of the full mix at one-beat spacing. The
//!   mix's drums carry the beat; the vocals' grid keeps it on a line the notes are snapped to.
//! - **Bar:** the beat, mod 4, with the most harmonic change in the chroma of the accompaniment
//!   (mix − vocals), summed over 1-, 2- and 4-beat windows either side of each beat.
//! - **Confidence:** the share of the song's 8-bar blocks whose own harmonic vote agrees with the
//!   whole song's. Below [`MIN_CONFIDENCE`] the bar falls back to the first beat.
//!
//! 4/4 only, one tempo.

use realfft::RealFftPlanner;

use crate::tempo::grid_phase;

pub const BEATS_PER_BAR: usize = 4;
/// Change windows either side of a beat, in beats.
pub const WINDOWS_BEATS: [usize; 3] = [1, 2, 4];
/// Bars per block in the confidence vote.
pub const BLOCK_BARS: usize = 8;
/// Below this share of agreeing blocks the harmonic bar is not used.
pub const MIN_CONFIDENCE: f64 = 0.5;
const N_FFT: usize = 4096;
const HOP: usize = 1024;
const MIN_HZ: f64 = 65.0;
const MAX_HZ: f64 = 2100.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarOne {
    /// The first of the music's beats at or after 0 s, on the vocals' cell grid, in seconds.
    pub beat_s: f64,
    /// The first bar line at or after 0 s: the harmonic bar, or `beat_s` when it is too weak.
    pub bar_s: f64,
    /// Beats from `beat_s` to the harmonic bar line, in `0..4`.
    pub harmonic_beat: usize,
    /// Share of 8-bar blocks whose own vote is `harmonic_beat`, in `0..=1`.
    pub confidence: f64,
}

impl BarOne {
    /// Whether `bar_s` comes from harmony rather than the beat fallback.
    pub fn from_harmony(&self) -> bool {
        self.confidence >= MIN_CONFIDENCE
    }
}

/// Bar 1 of a song from its full mix and its vocals stem (same rate, sample-aligned) at `bpm`, on a
/// grid of `division` cells per whole note. `None` when either is too short or silent to hear onsets.
pub fn estimate(mix: &[f32], vocals: &[f32], sample_rate: u32, bpm: f64, division: u32) -> Option<BarOne> {
    if !(bpm.is_finite() && bpm > 0.0) || division == 0 {
        return None;
    }
    let beat = 60.0 / bpm;
    let cell = 240.0 / (bpm * f64::from(division));
    let beat_s = beat_line(grid_phase(mix, sample_rate, beat)?, grid_phase(vocals, sample_rate, cell)?, cell, beat);
    let residual: Vec<f32> = mix.iter().zip(vocals).map(|(m, v)| m - v).collect();
    let chroma = chroma(&residual, sample_rate);
    let sr = f64::from(sample_rate);
    let change = beat_change(&chroma, HOP as f64 / sr, N_FFT as f64 / sr, beat_s, beat);
    let (harmonic_beat, confidence) = vote(&change);
    let bar_s = if confidence >= MIN_CONFIDENCE { beat_s + harmonic_beat as f64 * beat } else { beat_s };
    Some(BarOne { beat_s, bar_s, harmonic_beat, confidence })
}

/// The cell line (through `cell_phase`) nearest `beat_phase`, as the first such beat at or after 0 s.
pub fn beat_line(beat_phase: f64, cell_phase: f64, cell: f64, beat: f64) -> f64 {
    let on_cells = cell_phase + ((beat_phase - cell_phase) / cell).round() * cell;
    let t = on_cells.rem_euclid(beat);
    if beat - t < 1e-9 { 0.0 } else { t }
}

/// Per-frame chroma, each frame scaled to a maximum of 1 (silent frames stay zero). Frame `f`
/// covers samples `f·HOP .. f·HOP + N_FFT`.
pub fn chroma(x: &[f32], sample_rate: u32) -> Vec<[f64; 12]> {
    if x.len() < N_FFT {
        return Vec::new();
    }
    let sr = f64::from(sample_rate);
    let frames = (x.len() - N_FFT) / HOP + 1;
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);
    let window: Vec<f32> = (0..N_FFT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos()).collect();
    let bins: Vec<(usize, usize)> = (1..N_FFT / 2)
        .filter_map(|k| {
            let hz = k as f64 * sr / N_FFT as f64;
            (MIN_HZ..=MAX_HZ).contains(&hz).then(|| (k, ((12.0 * (hz / 440.0).log2()).round() as i64 + 69).rem_euclid(12) as usize))
        })
        .collect();
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut out = Vec::with_capacity(frames);
    for f in 0..frames {
        for (i, v) in input.iter_mut().enumerate() {
            *v = x[f * HOP + i] * window[i];
        }
        let mut c = [0.0f64; 12];
        if fft.process(&mut input, &mut spectrum).is_ok() {
            for &(k, pc) in &bins {
                c[pc] += f64::from(spectrum[k].norm_sqr());
            }
        }
        let peak = c.iter().copied().fold(0.0, f64::max);
        if peak > 1e-12 {
            c.iter_mut().for_each(|v| *v /= peak);
        } else {
            c = [0.0; 12];
        }
        out.push(c);
    }
    out
}

/// Harmonic change at each beat `beat_s + j·beat` for each window in [`WINDOWS_BEATS`]: the cosine
/// distance between the summed chroma of the `w` beats before and the `w` beats after. `None` where
/// a window runs off the audio or is silent. Frame `f` of `chroma` is centred on
/// `f·hop_s + window_s / 2`.
pub fn beat_change(chroma: &[[f64; 12]], hop_s: f64, window_s: f64, beat_s: f64, beat: f64) -> Vec<[Option<f64>; 3]> {
    let duration = chroma.len() as f64 * hop_s + window_s / 2.0;
    let first_frame = |t: f64| (((t - window_s / 2.0) / hop_s).ceil().max(0.0) as usize).min(chroma.len());
    let sum = |a: f64, b: f64| -> Option<[f64; 12]> {
        let (fa, fb) = (first_frame(a), first_frame(b));
        if a < 0.0 || b > duration || fb <= fa {
            return None;
        }
        let mut m = [0.0; 12];
        for c in &chroma[fa..fb] {
            m.iter_mut().zip(c).for_each(|(x, y)| *x += y);
        }
        Some(m)
    };
    let beats = ((duration - beat_s) / beat).floor().max(0.0) as usize + 1;
    (0..beats)
        .map(|j| {
            let t = beat_s + j as f64 * beat;
            let mut out = [None; 3];
            for (slot, &w) in out.iter_mut().zip(&WINDOWS_BEATS) {
                let span = w as f64 * beat;
                *slot = sum(t - span, t).zip(sum(t, t + span)).and_then(|(a, b)| cosine_distance(&a, &b));
            }
            out
        })
        .collect()
}

fn cosine_distance(a: &[f64; 12], b: &[f64; 12]) -> Option<f64> {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    (na > 1e-12 && nb > 1e-12).then(|| 1.0 - dot / (na * nb))
}

/// The bar's beat over a run of beats: each window's mean change per beat of the bar, scaled to sum
/// to 1, then summed over windows. `None` with no change measured.
pub fn profile(change: &[[Option<f64>; 3]]) -> Option<[f64; BEATS_PER_BAR]> {
    let mut total = [0.0; BEATS_PER_BAR];
    let mut any = false;
    for w in 0..WINDOWS_BEATS.len() {
        let mut sum = [0.0; BEATS_PER_BAR];
        let mut n = [0usize; BEATS_PER_BAR];
        for (j, c) in change.iter().enumerate() {
            if let Some(d) = c[w] {
                sum[j % BEATS_PER_BAR] += d;
                n[j % BEATS_PER_BAR] += 1;
            }
        }
        if n.contains(&0) {
            continue;
        }
        let mean: Vec<f64> = sum.iter().zip(&n).map(|(s, &k)| s / k as f64).collect();
        let norm: f64 = mean.iter().sum();
        if norm > 1e-12 {
            any = true;
            total.iter_mut().zip(&mean).for_each(|(t, m)| *t += m / norm);
        }
    }
    any.then_some(total)
}

fn argmax(p: &[f64; BEATS_PER_BAR]) -> usize {
    (0..BEATS_PER_BAR).fold(0, |best, i| if p[i] > p[best] { i } else { best })
}

/// The whole song's bar beat and the share of 8-bar blocks that vote for it.
pub fn vote(change: &[[Option<f64>; 3]]) -> (usize, f64) {
    let Some(whole) = profile(change) else { return (0, 0.0) };
    let best = argmax(&whole);
    let block = BLOCK_BARS * BEATS_PER_BAR;
    let votes: Vec<usize> = change.chunks(block).filter(|c| c.len() == block).filter_map(profile).map(|p| argmax(&p)).collect();
    if votes.is_empty() {
        return (best, 0.0);
    }
    (best, votes.iter().filter(|&&v| v == best).count() as f64 / votes.len() as f64)
}
