//! Song tempo from a full mix (ADR-0022): a spectral-flux novelty curve, its
//! autocorrelation, and a comb over the beat and its 8th/16th/32nd subdivisions,
//! searched in a fast window. The window is the point: a beat tracker's natural
//! answer for a 240 BPM song is 120, which makes a 16th grid too coarse for its
//! short notes, while doubling a genuinely slow song only makes the grid finer.
//! So the estimate errs towards double, never towards half.

use realfft::RealFftPlanner;

pub const MIN_BPM: f64 = 140.0;
pub const MAX_BPM: f64 = 280.0;
const PRIOR_CENTRE_BPM: f64 = 190.0;
const PRIOR_SD_OCTAVES: f64 = 1.0;
const SUBDIVISIONS: [f64; 4] = [1.0, 2.0, 4.0, 8.0];
const N_FFT: usize = 2048;
const HOP: usize = 512;
const TREND_FRAMES: usize = 16;
const SEARCH_STEPS: usize = 1200;
const REFINE_SPAN: f64 = 0.015;
const REFINE_STEPS: usize = 61;
const INTRO_S: f64 = 20.0;
const READ_S: f64 = 120.0;

/// The tempo in BPM, or `None` when there is too little audio to hear a pulse.
pub fn estimate_bpm(samples: &[f32], sample_rate: u32) -> Option<f64> {
    let sr = f64::from(sample_rate);
    let excerpt = if samples.len() as f64 > (INTRO_S + READ_S) * sr {
        let a = (INTRO_S * sr) as usize;
        &samples[a..a + (READ_S * sr) as usize]
    } else {
        samples
    };
    let fps = sr / HOP as f64;
    let novelty = novelty(excerpt);
    let max_lag = (SUBDIVISIONS[SUBDIVISIONS.len() - 1] * 60.0 * fps / MIN_BPM).ceil() as usize + 2;
    if novelty.len() < 2 * max_lag {
        return None;
    }
    let ac = autocorrelation(&novelty, max_lag)?;
    let comb = |bpm: f64| -> f64 {
        let lag = 60.0 * fps / bpm;
        SUBDIVISIONS.iter().map(|k| interp(&ac, k * lag) / k).sum()
    };
    let (lo, hi) = (MIN_BPM.ln(), MAX_BPM.ln());
    let best = (0..SEARCH_STEPS)
        .map(|i| (lo + (hi - lo) * i as f64 / (SEARCH_STEPS - 1) as f64).exp())
        .map(|bpm| {
            let prior = (-0.5 * ((bpm / PRIOR_CENTRE_BPM).log2() / PRIOR_SD_OCTAVES).powi(2)).exp();
            (bpm, comb(bpm) * prior)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))?
        .0;
    (0..REFINE_STEPS)
        .map(|i| best * (1.0 - REFINE_SPAN + 2.0 * REFINE_SPAN * i as f64 / (REFINE_STEPS - 1) as f64))
        .map(|bpm| (bpm, comb(bpm)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(bpm, _)| bpm)
}

fn novelty(x: &[f32]) -> Vec<f64> {
    if x.len() < N_FFT + HOP {
        return Vec::new();
    }
    let frames = (x.len() - N_FFT) / HOP;
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);
    let window: Vec<f32> = (0..N_FFT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (N_FFT - 1) as f32).cos()).collect();
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let mut prev: Vec<f64> = Vec::new();
    let mut flux = Vec::with_capacity(frames.saturating_sub(1));
    for f in 0..frames {
        for (i, v) in input.iter_mut().enumerate() {
            *v = x[f * HOP + i] * window[i];
        }
        if fft.process(&mut input, &mut spectrum).is_err() {
            return Vec::new();
        }
        let log: Vec<f64> = spectrum.iter().map(|c| (1.0 + 1000.0 * f64::from(c.norm())).ln()).collect();
        if !prev.is_empty() {
            flux.push(log.iter().zip(&prev).map(|(a, b)| (a - b).max(0.0)).sum());
        }
        prev = log;
    }
    let trend = moving_mean(&flux, TREND_FRAMES);
    flux.iter().zip(trend).map(|(f, t)| (f - t).max(0.0)).collect()
}

/// The mean over a `width`-frame window from `i - width/2 + 1` to `i + width/2`, counting frames past
/// either end as zero. (numpy's `convolve(x, ones(w)/w, "same")` centres one frame earlier; the measured
/// numbers in ADR-0022 are for this window.)
fn moving_mean(x: &[f64], width: usize) -> Vec<f64> {
    let half = width / 2;
    (0..x.len())
        .map(|i| {
            let a = i.saturating_sub(half - 1);
            let b = (i + half).min(x.len() - 1);
            x[a..=b].iter().sum::<f64>() / width as f64
        })
        .collect()
}

fn autocorrelation(x: &[f64], max_lag: usize) -> Option<Vec<f64>> {
    let mean = x.iter().sum::<f64>() / x.len() as f64;
    let d: Vec<f64> = x.iter().map(|v| v - mean).collect();
    let zero: f64 = d.iter().map(|v| v * v).sum();
    if zero <= 0.0 {
        return None;
    }
    Some((0..=max_lag).map(|lag| d[lag..].iter().zip(&d).map(|(a, b)| a * b).sum::<f64>() / zero).collect())
}

fn interp(ac: &[f64], at: f64) -> f64 {
    let i = at.floor() as usize;
    if i + 1 >= ac.len() {
        return ac[ac.len() - 1];
    }
    let w = at - i as f64;
    ac[i] * (1.0 - w) + ac[i + 1] * w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click_track(bpm: f64, seconds: f64, sr: u32) -> Vec<f32> {
        let n = (seconds * f64::from(sr)) as usize;
        let mut x = vec![0.0f32; n];
        let mut seed = 12345u32;
        for (i, v) in x.iter_mut().enumerate() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            *v = ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.01;
            let t = i as f64 / f64::from(sr);
            let eighth = 30.0 / bpm;
            let since = t % eighth;
            let beat = ((t / eighth).floor() as i64) % 2 == 0;
            if since < 0.03 {
                let env = (-since * 120.0).exp() as f32;
                let tone = if beat { (2.0 * std::f64::consts::PI * 60.0 * t).sin() } else { (2.0 * std::f64::consts::PI * 7000.0 * t).sin() * 0.4 };
                *v += env * tone as f32;
            }
        }
        x
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() / b <= 0.02
    }

    #[test]
    fn hears_a_fast_click_track_at_its_tempo() {
        for bpm in [150.0, 175.0, 200.0, 240.0] {
            let got = estimate_bpm(&click_track(bpm, 30.0, 22050), 22050).unwrap();
            assert!(near(got, bpm), "{bpm} → {got}");
        }
    }

    #[test]
    fn reads_a_slow_song_at_double_never_at_half() {
        for bpm in [100.0, 120.0] {
            let got = estimate_bpm(&click_track(bpm, 30.0, 22050), 22050).unwrap();
            assert!(near(got, 2.0 * bpm), "{bpm} → {got}");
        }
    }

    #[test]
    fn stays_inside_its_window() {
        for bpm in [90.0, 130.0, 210.0, 270.0] {
            let got = estimate_bpm(&click_track(bpm, 20.0, 16000), 16000).unwrap();
            assert!((MIN_BPM * 0.985..=MAX_BPM * 1.015).contains(&got), "{bpm} → {got}");
        }
    }

    #[test]
    fn says_nothing_about_silence_or_a_blip() {
        assert_eq!(estimate_bpm(&vec![0.0; 22050 * 10], 22050), None);
        assert_eq!(estimate_bpm(&vec![0.1; 1000], 22050), None);
    }
}
