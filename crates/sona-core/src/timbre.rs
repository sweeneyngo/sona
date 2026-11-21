//! **Timbre frames** for the grid decoder's spectral re-articulation cue
//! ([`CueModel::Spectral`](crate::grid::CueModel)).
//!
//! A new syllable on a held pitch rarely dips the voicing or the energy far
//! enough to see (AUC about 0.7 on dev), but it changes the *spectrum*: the
//! consonant's burst raises spectral flux and the new vowel moves the spectral
//! envelope. These frames carry the three per-frame series the cue reads:
//! spectral flux over all bins and below 1 kHz, each divided by its median over
//! the stem, and the low real-cepstrum coefficients (the envelope).
//!
//! The struct is plain data, so the decoder needs no FFT and tests can build
//! frames directly. [`TimbreFrames::from_samples`] computes them from a stem and
//! needs the `spectral` feature (realfft).

/// Number of real-cepstrum coefficients kept (quefrency 1..=N_CEPS).
pub const N_CEPS: usize = 20;

/// Per-frame timbre series, frame `i` centred at `time_s[i]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimbreFrames {
    /// Frame centres, seconds, strictly increasing.
    pub time_s: Vec<f64>,
    /// Half-wave-rectified log-magnitude flux over all bins, divided by its
    /// median over the stem (so `1.0` is a typical frame).
    pub flux: Vec<f32>,
    /// The same below 1 kHz, divided by its own median.
    pub flux_lo: Vec<f32>,
    /// Real cepstrum `c[1..=N_CEPS]` of the log-magnitude spectrum.
    pub ceps: Vec<[f32; N_CEPS]>,
}

/// Floor added to a bin's magnitude before the log.
#[cfg(feature = "spectral")]
const MAG_EPS: f32 = 1e-5;

impl TimbreFrames {
    /// Analyse a mono stem with `frame_length`-sample Hann frames every `hop`
    /// samples (frame centres as in the energy envelope). Empty when the stem is
    /// shorter than one frame.
    #[cfg(feature = "spectral")]
    pub fn from_samples(samples: &[f32], sample_rate: u32, frame_length: usize, hop: usize) -> Self {
        let mut out = Self::raw_from_samples(samples, sample_rate, frame_length, hop);
        normalise_by_median(&mut out.flux);
        normalise_by_median(&mut out.flux_lo);
        out
    }

    /// [`from_samples`](Self::from_samples) before the flux is normalised.
    #[cfg(feature = "spectral")]
    fn raw_from_samples(samples: &[f32], sample_rate: u32, frame_length: usize, hop: usize) -> Self {
        use realfft::RealFftPlanner;
        let (n, hop) = (frame_length.max(2), hop.max(1));
        if samples.len() < n || sample_rate == 0 {
            return Self::default();
        }
        // Symmetric Hann (numpy's `hanning`), the window the cue was fitted with.
        let window: Vec<f32> =
            (0..n).map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos() as f32).collect();
        let r2c = RealFftPlanner::<f32>::new().plan_fft_forward(n);
        let mut input = r2c.make_input_vec();
        let mut spectrum = r2c.make_output_vec();
        let n_bins = spectrum.len();
        let lo_bins = n_bins.min((1000.0 * n as f64 / sample_rate as f64).ceil() as usize);
        // c[q] = (1/n)·Σ_k L_k·cos(2πkq/n) over the full (even) spectrum.
        let basis: Vec<Vec<f32>> = (1..=N_CEPS)
            .map(|q| {
                (0..n_bins)
                    .map(|k| {
                        let edge = k == 0 || (n % 2 == 0 && k == n_bins - 1);
                        let m = if edge { 1.0 } else { 2.0 };
                        (m * (std::f64::consts::TAU * (k * q) as f64 / n as f64).cos() / n as f64) as f32
                    })
                    .collect()
            })
            .collect();

        let mut out = Self::default();
        let mut prev: Option<Vec<f32>> = None;
        let mut start = 0;
        while start + n <= samples.len() {
            for (d, (&s, &w)) in input.iter_mut().zip(samples[start..start + n].iter().zip(&window)) {
                *d = s * w;
            }
            r2c.process(&mut input, &mut spectrum).expect("vecs sized by the planner");
            let log_mag: Vec<f32> = spectrum.iter().map(|c| (c.norm() + MAG_EPS).ln()).collect();
            let (all, lo) = match &prev {
                None => (0.0, 0.0),
                Some(p) => {
                    let rise = |k: usize| (log_mag[k] - p[k]).max(0.0);
                    ((0..n_bins).map(rise).sum(), (0..lo_bins).map(rise).sum())
                }
            };
            let mut c = [0.0f32; N_CEPS];
            for (cq, b) in c.iter_mut().zip(&basis) {
                *cq = b.iter().zip(&log_mag).map(|(x, l)| x * l).sum();
            }
            out.time_s.push((start as f64 + n as f64 / 2.0) / sample_rate as f64);
            out.flux.push(all);
            out.flux_lo.push(lo);
            out.ceps.push(c);
            prev = Some(log_mag);
            start += hop;
        }
        out
    }
}

#[cfg(feature = "spectral")]
/// Divide by the median of the non-zero values: digitally silent frames have
/// zero flux and would otherwise drag the median to zero.
///
/// The divisor is floored at 1% of the 99th percentile. On a clean signal
/// (synthetic tones, digital silence) a steady note's flux is rounding noise, so
/// the median is too, and ordinary flux would read as a huge burst. Real stems'
/// medians sit far above the floor.
fn normalise_by_median(v: &mut [f32]) {
    let d = flux_divisor(v);
    v.iter_mut().for_each(|x| *x /= d);
}

#[cfg(feature = "spectral")]
fn flux_divisor(v: &[f32]) -> f32 {
    let mut s: Vec<f32> = v.iter().copied().filter(|&x| x > 0.0).collect();
    if s.is_empty() {
        return 1.0;
    }
    s.sort_by(f32::total_cmp);
    let median = s[s.len() / 2];
    let p99 = s[((s.len() - 1) as f64 * 0.99).round() as usize];
    median.max(0.01 * p99)
}


#[cfg(all(test, feature = "spectral"))]
mod tests {
    use super::*;

    fn tone(f: f32, secs: f32, sr: u32) -> Vec<f32> {
        (0..(secs * sr as f32) as usize).map(|i| 0.3 * (std::f32::consts::TAU * f * i as f32 / sr as f32).sin()).collect()
    }

    #[test]
    fn frames_follow_the_envelope_grid_and_flux_peaks_where_the_timbre_changes() {
        let sr = 44_100;
        // A 220 Hz tone, then (at 0.5 s) the same pitch with strong upper harmonics.
        let mut x = tone(220.0, 0.5, sr);
        let bright: Vec<f32> = (0..(0.5 * sr as f32) as usize)
            .map(|i| {
                let t = i as f32 / sr as f32;
                (1..8).map(|h| 0.3 / h as f32 * (std::f32::consts::TAU * 220.0 * h as f32 * t).sin()).sum::<f32>()
            })
            .collect();
        x.extend(bright);
        let fr = TimbreFrames::from_samples(&x, sr, 1024, 256);
        assert_eq!(fr.time_s.len(), 1 + (x.len() - 1024) / 256);
        assert!((fr.time_s[0] - 512.0 / sr as f64).abs() < 1e-12);
        let peak = (1..fr.flux.len()).max_by(|&a, &b| fr.flux[a].total_cmp(&fr.flux[b])).expect("frames");
        assert!((fr.time_s[peak] - 0.5).abs() < 0.03, "flux peak at {}", fr.time_s[peak]);
        let d = |a: usize, b: usize| fr.ceps[a].iter().zip(&fr.ceps[b]).map(|(x, y)| (x - y) * (x - y)).sum::<f32>().sqrt();
        let (early, mid, late) = (20, 40, 120);
        assert!(d(early, late) > 3.0 * d(early, mid), "envelope moves across the change, not within a segment");
    }

    #[test]
    fn a_stem_shorter_than_a_frame_has_no_frames() {
        assert_eq!(TimbreFrames::from_samples(&[0.0; 100], 44_100, 1024, 256), TimbreFrames::default());
    }
}
