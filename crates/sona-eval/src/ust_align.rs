//! **Phase alignment** of a UST to its original's stems (UST corpus gate, 05).
//!
//! BPM is exact (from the UST), so alignment is a single offset:
//! `stem_time = ust_time + offset`. It is found **without sona's onset
//! detector**, which would be circular (it would absorb the detector's bias into
//! the ground truth it is then scored against):
//!
//! 1. **Coarse** — cross-correlate the UST's *any-voice-singing* mask with the
//!    vocal stem's log-RMS envelope (10 ms frames). Long-range rest structure pins
//!    the offset to within a fraction of a beat.
//! 2. **Fine** — within ±½ grid cell of the coarse offset, pick the phase whose
//!    UST grid lines (every 16th, or 16th-triplet) best coincide with **drum
//!    onsets** (an energy-rise novelty on the separated drum stem, 1 ms
//!    resolution). Drums mark the grid independently of the vocal and of sona.
//!    The search is deliberately ±½ *cell*, not ±½ *beat*: a UST's notes sit on
//!    its 16th grid, but its tick 0 need not be a downbeat (a pickup), so
//!    "every 480 ticks" is not reliably a musical beat — matching those lines to
//!    kick/snare hits shifted one song by a whole 16th. When the drum grid is too
//!    weak to trust, the coarse offset stands and the song is marked as such.
//!
//! **Verification** (the spec's onset-delta distributions) is separate and may
//! use sona's onsets — checking an alignment with sona is not circular; setting
//! it with sona would be.

/// Coarse envelope frame, seconds.
pub const COARSE_HOP_S: f64 = 0.010;
/// Fine novelty frame, seconds.
pub const FINE_HOP_S: f64 = 0.001;
/// A drum-phase peak must beat the mean phase score by this factor to be
/// trusted over the coarse offset.
pub const MIN_DRUM_PEAK_RATIO: f64 = 1.3;

/// Frame-wise log-RMS (natural log of RMS + ε) of `samples` at `hop_s`, using a
/// `win_s` window centred on each frame. Prefix sums keep it O(n).
pub fn log_rms(samples: &[f32], sample_rate: u32, hop_s: f64, win_s: f64) -> Vec<f64> {
    let sr = sample_rate as f64;
    let n = samples.len();
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0f64);
    let mut acc = 0.0f64;
    for &s in samples {
        acc += (s as f64) * (s as f64);
        prefix.push(acc);
    }
    let half = ((win_s * sr) / 2.0).round().max(1.0) as usize;
    let frames = ((n as f64 / sr) / hop_s).floor() as usize;
    (0..frames)
        .map(|f| {
            let c = (f as f64 * hop_s * sr).round() as usize;
            let (a, b) = (c.saturating_sub(half), (c + half).min(n));
            let energy = if b > a { (prefix[b] - prefix[a]) / (b - a) as f64 } else { 0.0 };
            (energy.sqrt() + 1e-6).ln()
        })
        .collect()
}

/// Z-score in place (zero mean, unit variance; constant input becomes zeros).
pub fn zscore(xs: &mut [f64]) {
    let n = xs.len().max(1) as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let sd = (xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    for x in xs.iter_mut() {
        *x = if sd > 0.0 { (*x - mean) / sd } else { 0.0 };
    }
}

/// Coarse offset: the lag (seconds, a multiple of `hop_s`) maximising the
/// correlation of `spans` (UST-time singing spans, as a zero-mean 0/1 mask) with
/// the z-scored envelope `env` (stem frames at `hop_s`). Only lags that keep
/// every span inside the stem (± `slack_s`) are tried.
pub fn coarse_offset(spans: &[(f64, f64)], env: &[f64], hop_s: f64, slack_s: f64) -> Option<f64> {
    let first = spans.iter().map(|s| s.0).reduce(f64::min)?;
    let last = spans.iter().map(|s| s.1).reduce(f64::max)?;
    let stem_s = env.len() as f64 * hop_s;
    let (lo, hi) = (-first - slack_s, stem_s - last + slack_s);
    if hi < lo {
        return None;
    }
    // Mask over UST time [0, last], zero-mean so silence counts against a lag.
    let m_len = (last / hop_s).ceil() as usize + 1;
    let mut mask = vec![0.0f64; m_len];
    for &(s, e) in spans {
        let (a, b) = ((s / hop_s).round() as usize, ((e / hop_s).round() as usize).min(m_len));
        for v in &mut mask[a.min(b)..b] {
            *v = 1.0;
        }
    }
    let mean = mask.iter().sum::<f64>() / m_len as f64;
    for v in &mut mask {
        *v -= mean;
    }
    let (lo_f, hi_f) = ((lo / hop_s).floor() as i64, (hi / hop_s).ceil() as i64);
    let mut best: Option<(f64, i64)> = None;
    for lag in lo_f..=hi_f {
        let mut score = 0.0;
        for (i, &m) in mask.iter().enumerate() {
            let j = i as i64 + lag;
            if j >= 0 && (j as usize) < env.len() {
                score += m * env[j as usize];
            }
        }
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, lag));
        }
    }
    best.map(|(_, lag)| lag as f64 * hop_s)
}

/// Onset novelty of a drum stem at [`FINE_HOP_S`]: half-wave-rectified rise of
/// log-RMS over ~10 ms, smoothed with a ~4 ms Gaussian.
pub fn drum_novelty(samples: &[f32], sample_rate: u32) -> Vec<f64> {
    let e = log_rms(samples, sample_rate, FINE_HOP_S, 0.010);
    let lag = 10;
    let raw: Vec<f64> = (0..e.len()).map(|i| if i >= lag { (e[i] - e[i - lag]).max(0.0) } else { 0.0 }).collect();
    gaussian_smooth(&raw, 4.0)
}

fn gaussian_smooth(xs: &[f64], sigma: f64) -> Vec<f64> {
    let r = (3.0 * sigma).ceil() as i64;
    let kernel: Vec<f64> = (-r..=r).map(|k| (-(k as f64).powi(2) / (2.0 * sigma * sigma)).exp()).collect();
    let norm: f64 = kernel.iter().sum();
    (0..xs.len() as i64)
        .map(|i| {
            kernel
                .iter()
                .enumerate()
                .map(|(k, w)| {
                    let j = i + k as i64 - r;
                    if j >= 0 && (j as usize) < xs.len() { w * xs[j as usize] } else { 0.0 }
                })
                .sum::<f64>()
                / norm
        })
        .collect()
}

/// The fine phase search's outcome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FineAlignment {
    /// Best offset found (seconds).
    pub offset_s: f64,
    /// Peak phase score / mean phase score over the ±½-beat search: how clearly
    /// the drums mark the grid (≈1 for drums with no beat structure).
    pub peak_ratio: f64,
}

/// Fine offset: search `coarse ± cell/2` in [`FINE_HOP_S`] steps for the offset
/// whose grid `lines` (UST-time, seconds, one per `cell_s`) land on the most
/// drum `novelty`.
pub fn fine_offset(coarse_s: f64, cell_s: f64, lines: &[f64], novelty: &[f64]) -> FineAlignment {
    let steps = (cell_s / 2.0 / FINE_HOP_S).round() as i64;
    let score = |offset: f64| -> f64 {
        lines
            .iter()
            .map(|&t| {
                let i = ((t + offset) / FINE_HOP_S).round();
                if i >= 0.0 && (i as usize) < novelty.len() { novelty[i as usize] } else { 0.0 }
            })
            .sum()
    };
    let scored: Vec<(f64, f64)> = (-steps..=steps)
        .map(|k| {
            let o = coarse_s + k as f64 * FINE_HOP_S;
            (o, score(o))
        })
        .collect();
    let (offset_s, best) = scored.iter().copied().fold((coarse_s, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
    // Mean, not median: with clean, sparse drum hits most phases score ~0.
    let mean = scored.iter().map(|s| s.1).sum::<f64>() / scored.len() as f64;
    FineAlignment { offset_s, peak_ratio: if mean > 0.0 { best / mean } else { 0.0 } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coarse_recovers_a_known_lag() {
        // Singing in UST time at [1,2) and [3,5); the stem has it 2.37 s later.
        let spans = [(1.0, 2.0), (3.0, 5.0)];
        let hop = 0.01;
        let lag = 2.37;
        let mut env: Vec<f64> = (0..1000)
            .map(|i| {
                let t = i as f64 * hop - lag;
                if spans.iter().any(|&(s, e)| t >= s && t < e) { 1.0 } else { 0.0 }
            })
            .collect();
        zscore(&mut env);
        let got = coarse_offset(&spans, &env, hop, 1.0).unwrap();
        assert!((got - lag).abs() < 0.011, "got {got}");
    }

    #[test]
    fn fine_recovers_drum_phase_within_half_a_cell() {
        // Drums hit every 16th (0.125 s) starting at 0.123 s; grid lines in UST time
        // every 0.125 s from 0. True offset 0.123; coarse guess 0.16 (37 ms off,
        // inside ±½ cell = ±62.5 ms).
        let cell = 0.125;
        let mut nov = vec![0.0f64; 20_000];
        for k in 0..150 {
            // Accent the beat (every 4th hit) like a kick/snare over hats.
            let i = ((0.123 + k as f64 * cell) / FINE_HOP_S).round() as usize;
            nov[i] = if k % 4 == 0 { 1.0 } else { 0.4 };
        }
        let nov = gaussian_smooth(&nov, 4.0);
        let lines: Vec<f64> = (0..150).map(|k| k as f64 * cell).collect();
        let fine = fine_offset(0.16, cell, &lines, &nov);
        assert!((fine.offset_s - 0.123).abs() < 0.0015, "{fine:?}");
        assert!(fine.peak_ratio > MIN_DRUM_PEAK_RATIO);
    }

    #[test]
    fn a_pickup_does_not_shift_the_phase_by_a_cell() {
        // Regression: the UST's tick 0 sits one 16th BEFORE a downbeat (a pickup),
        // so its every-480-ticks "beats" fall a 16th off the real kicks. Searching
        // only ±½ cell around a correct coarse offset must keep the true phase
        // rather than slide a whole cell to put those lines on the accented hits.
        let cell = 0.125;
        let true_offset = 1.0;
        let mut nov = vec![0.0f64; 30_000];
        for k in 0..150 {
            let i = ((true_offset + k as f64 * cell) / FINE_HOP_S).round() as usize;
            // Accents on k ≡ 1 (mod 4): real beats, one cell after the UST's "beats".
            nov[i] = if k % 4 == 1 { 1.0 } else { 0.4 };
        }
        let nov = gaussian_smooth(&nov, 4.0);
        let lines: Vec<f64> = (0..150).map(|k| k as f64 * cell).collect();
        let fine = fine_offset(true_offset + 0.010, cell, &lines, &nov);
        assert!((fine.offset_s - true_offset).abs() < 0.0015, "{fine:?}");
    }

    #[test]
    fn flat_drums_give_no_confident_peak() {
        let nov = vec![1.0f64; 10_000];
        let beats: Vec<f64> = (0..10).map(|k| k as f64 * 0.5).collect();
        let fine = fine_offset(1.0, 0.5, &beats, &nov);
        assert!(fine.peak_ratio < MIN_DRUM_PEAK_RATIO);
    }

    #[test]
    fn log_rms_tracks_loudness() {
        let sr = 1000;
        let mut s = vec![0.0f32; 2000];
        for v in &mut s[1000..] {
            *v = 0.5;
        }
        let e = log_rms(&s, sr, 0.01, 0.02);
        assert!(e[150] > e[50] + 5.0);
    }
}
