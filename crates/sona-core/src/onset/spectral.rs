//! **Spectral-flux onset novelty** ([ADR-0013]) — behind the `spectral` feature.
//!
//! [ADR-0013]: ../../../docs/adr/0013-spectral-flux-onsets.md
//!
//! The energy-rise novelty (ADR-0006) only sees an attack that *raises energy*.
//! A legato attack — a new syllable at a similar loudness, or a slurred note
//! change — changes the *spectrum* (new harmonics, a consonant's noise burst)
//! while energy barely moves, so the energy detector misses it (the measured
//! recall hole: ~half the Vocadito onsets undetected). **Spectral flux** measures
//! the frame-to-frame growth of the magnitude spectrum, so it fires on that
//! harmonic/timbral change.
//!
//! The novelty is computed on the **same frame grid as the energy envelope**
//! (same `frame_length` / `hop_length`, same frame starts), so the returned curve
//! aligns index-for-index with the envelope and the shared peak-picker
//! ([`OnsetDetector::peak_pick`]) — including its RMS bleed guard — works
//! unchanged. Log-magnitude differencing keeps the novelty in the same units as
//! the energy path, so the peak-picker's `threshold_delta` stays in a comparable
//! range (its exact value is still the harness's to tune).
//!
//! ## SuperFlux (vibrato robustness)
//!
//! Plain flux over-fires on continuous spectral motion — a vibrato'd partial
//! sweeping across bins reads as a rise every frame — so its recovered recall
//! comes at a precision cost. **SuperFlux** (Böck & Widmer 2013) max-filters the
//! *previous* frame's spectrum across ±`max_filter_bins` frequency bins before
//! differencing: a partial that drifts within that width stays covered by the
//! filtered reference and produces no flux, while a sharp broadband attack (energy
//! appearing where the filtered reference is still low) survives. `max_filter_bins
//! = 0` recovers plain spectral flux.

use realfft::RealFftPlanner;

use super::OnsetDetector;

/// Floor added before the log so a silent bin maps to a finite (very negative)
/// log-magnitude rather than `-inf`, keeping the flux finite. Mirrors the energy
/// path's `LOG_ENERGY_EPS`.
const LOG_MAG_EPS: f32 = 1e-6;

/// Per-frame log-magnitude spectral-flux novelty, aligned to the energy envelope
/// (`n_frames` values, `novelty[0] == 0`). The mean over bins of the half-wave-
/// rectified rise in log-magnitude between consecutive short-time frames; the
/// previous frame is first max-filtered over ±`max_filter_bins` frequency bins
/// (SuperFlux — `0` disables it, giving plain flux).
pub(super) fn flux_novelty(
    detector: &OnsetDetector,
    samples: &[f32],
    n_frames: usize,
    max_filter_bins: usize,
) -> Vec<f32> {
    let frame_length = detector.frame_length.max(1);
    let hop = detector.hop_length.max(1);
    if n_frames == 0 || samples.len() < frame_length {
        return vec![0.0; n_frames];
    }

    let window = hann(frame_length);
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(frame_length);
    let mut input = r2c.make_input_vec();
    let mut spectrum = r2c.make_output_vec();

    let mut novelty = Vec::with_capacity(n_frames);
    let mut prev_log_mag: Option<Vec<f32>> = None;

    let mut start = 0;
    // The same loop condition as `energy_envelope`, so this yields exactly the
    // same frame count and timing as the envelope it aligns to.
    while start + frame_length <= samples.len() && novelty.len() < n_frames {
        for (dst, (&s, &w)) in input.iter_mut().zip(samples[start..start + frame_length].iter().zip(&window)) {
            *dst = s * w;
        }
        // realfft uses `input` as scratch; it is fully overwritten next frame.
        r2c.process(&mut input, &mut spectrum).expect("input/output vecs sized by the planner");

        let log_mag: Vec<f32> = spectrum.iter().map(|c| (c.norm() + LOG_MAG_EPS).ln()).collect();
        let flux = match &prev_log_mag {
            None => 0.0, // first frame: no predecessor
            Some(prev) => {
                // SuperFlux: compare against the max-filtered previous frame, so a
                // vibrato'd partial drifting within ±max_filter_bins is still
                // "covered" and yields no rise.
                let sum: f32 = log_mag
                    .iter()
                    .enumerate()
                    .map(|(k, &m)| (m - max_around(prev, k, max_filter_bins)).max(0.0))
                    .sum();
                sum / log_mag.len() as f32
            }
        };
        novelty.push(flux);
        prev_log_mag = Some(log_mag);
        start += hop;
    }

    // Pad in the unlikely event of a short tail so the curve matches the envelope
    // length the peak-picker expects.
    novelty.resize(n_frames, 0.0);
    novelty
}

/// Maximum of `spectrum` over the frequency bins `[k−radius, k+radius]`, clamped
/// to the spectrum's bounds. `radius == 0` returns `spectrum[k]` (plain flux).
fn max_around(spectrum: &[f32], k: usize, radius: usize) -> f32 {
    let lo = k.saturating_sub(radius);
    let hi = (k + radius + 1).min(spectrum.len());
    spectrum[lo..hi].iter().copied().fold(f32::NEG_INFINITY, f32::max)
}

/// A periodic Hann window of length `n` (DFT-symmetric), to taper each analysis
/// frame before the FFT and suppress spectral leakage that would otherwise read
/// as spurious flux.
fn hann(n: usize) -> Vec<f32> {
    if n <= 1 {
        return vec![1.0; n];
    }
    use std::f32::consts::PI;
    (0..n)
        .map(|i| {
            let x = (PI * i as f32 / n as f32).sin();
            x * x
        })
        .collect()
}
