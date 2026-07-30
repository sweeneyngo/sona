//! **pYIN** — the default DSP pitch tracker (ADR-0001), hand-rolled in pure Rust.
//!
//! ## Why hand-rolled
//!
//! ADR-0001 specifies pYIN as *pure-Rust DSP*. The obvious crates don't fit:
//! `pitch-detection` implements the McLeod Pitch Method (autocorrelation), not
//! YIN, and `aubio-rs` binds a system C library. So this is a direct
//! implementation of YIN (de Cheveigné & Kawahara, 2002) with pYIN's
//! probabilistic threshold step (Mauch & Dixon, 2014). See ADR-0005.
//!
//! ## What it does
//!
//! Per frame: the YIN cumulative-mean-normalized difference function gives a set
//! of candidate lags (its local minima). pYIN's insight is that YIN's single
//! absolute threshold is brittle, so instead we integrate over *many* thresholds
//! drawn from a Beta prior (mean ~0.1): each threshold picks the first candidate
//! below it, and the Beta mass that lands on a candidate becomes its
//! probability. A frame's f0 is the highest-probability candidate; its voicing
//! is the total Beta mass that found *any* candidate — low when the signal is
//! too aperiodic (silence, bleed, noise) for even a lax threshold to bite.
//!
//! The full pYIN then runs an HMM/Viterbi over candidates. We deliberately
//! substitute the spec's median filter (ADR-0005): it removes the isolated
//! octave errors and jitter the HMM targets, at the contour stage, without the
//! state machine. Vibrato survives it as a slow ripple — splitting a vibrato'd
//! note is segmentation's problem, not the tracker's.

use crate::annotation::F0Frame;

use super::{median_filter_positive, PitchTracker};

/// pYIN pitch tracker. Construct with [`PYin::default`] for sensible vocal
/// defaults, or set fields directly to tune the search range / smoothing.
#[derive(Debug, Clone)]
pub struct PYin {
    /// Lowest f0 to search for, in Hz (sets the maximum analysis lag).
    pub fmin: f32,
    /// Highest f0 to search for, in Hz (sets the minimum analysis lag).
    pub fmax: f32,
    /// Analysis window / buffer length in samples. The difference function
    /// integrates over `frame_length / 2` samples and searches lags up to it.
    pub frame_length: usize,
    /// Frames advance by this many samples; the frame rate is `sample_rate /
    /// hop_length`.
    pub hop_length: usize,
    /// Number of Beta-distributed thresholds integrated per frame (pYIN's `p`).
    pub n_thresholds: usize,
    /// Shape `a` of the Beta threshold prior. `Beta(2, 18)` has mean `0.1`.
    pub beta_a: f32,
    /// Shape `b` of the Beta threshold prior.
    pub beta_b: f32,
    /// A frame counts as voiced (its f0 kept, then median-filtered) when its
    /// voicing probability reaches this, in `0.0..=1.0`.
    pub voiced_threshold: f32,
    /// Width, in frames, of the median filter applied to the voiced f0 track.
    pub median_window: usize,
}

impl Default for PYin {
    fn default() -> Self {
        Self {
            fmin: 65.0,   // ~C2 — low end of a sung vocal range
            fmax: 2093.0, // ~C7 — comfortably above soprano fundamentals
            frame_length: 2048,
            hop_length: 256,
            n_thresholds: 100,
            beta_a: 2.0,
            beta_b: 18.0, // Beta(2, 18): mean 0.1, the pYIN threshold prior
            voiced_threshold: 0.5,
            median_window: 5,
        }
    }
}

impl PitchTracker for PYin {
    fn track(&self, samples: &[f32], sample_rate: u32) -> Vec<F0Frame> {
        let sr = sample_rate as f32;
        // The difference function sums over `w` samples and searches lags up to
        // `tau_max`, so it needs `w + tau_max <= frame_length`. With `w =
        // frame_length / 2`, capping `tau_max` at `w` guarantees that and bounds
        // the effective fmin at high sample rates.
        let w = self.frame_length / 2;
        let tau_max = ((sr / self.fmin).ceil() as usize).clamp(2, w);
        let tau_min = ((sr / self.fmax).floor() as usize).max(1);
        let thresholds = self.threshold_weights();

        // The raw, pre-smoothing estimate for each frame.
        let mut raw: Vec<RawFrame> = Vec::new();
        let mut start = 0;
        while start + self.frame_length <= samples.len() {
            let frame = &samples[start..start + self.frame_length];
            let (freq_hz, voicing) = analyse_frame(frame, w, tau_min, tau_max, sr, &thresholds);
            raw.push(RawFrame {
                time_s: start as f64 / sr as f64,
                freq_hz,
                voicing,
            });
            start += self.hop_length;
        }

        // Median-filter the *voiced* f0 track to kill jitter / octave errors
        // (spec + ADR-0005). Unvoiced frames enter as 0.0 and are skipped by the
        // filter so they neither get a spurious pitch nor drag their neighbours.
        let voiced_track: Vec<f32> = raw
            .iter()
            .map(|f| {
                if f.voicing >= self.voiced_threshold {
                    f.freq_hz
                } else {
                    0.0
                }
            })
            .collect();
        let smoothed = median_filter_positive(&voiced_track, self.median_window);

        raw.iter()
            .zip(smoothed)
            .map(|(f, smoothed_hz)| F0Frame {
                time_s: f.time_s,
                // Keep the invariant the F0Frame contract implies: a voiced frame
                // carries its (smoothed) f0, an unvoiced frame carries none. The
                // median window can bleed a neighbour's pitch into an unvoiced
                // boundary frame, so re-gate on voicing here.
                frequency_hz: if f.voicing >= self.voiced_threshold {
                    smoothed_hz
                } else {
                    0.0
                },
                voicing: f.voicing,
            })
            .collect()
    }
}

impl PYin {
    /// The Beta-distributed threshold grid: `(threshold, weight)` pairs with
    /// weights normalized to sum to 1, so a frame's accumulated voiced weight is
    /// directly a probability in `0.0..=1.0`.
    fn threshold_weights(&self) -> Vec<(f32, f32)> {
        let n = self.n_thresholds.max(1);
        let mut grid: Vec<(f32, f32)> = (0..n)
            .map(|k| {
                let s = (k as f32 + 0.5) / n as f32; // bin center in (0, 1)
                (s, beta_pdf(s, self.beta_a, self.beta_b))
            })
            .collect();
        let total: f32 = grid.iter().map(|&(_, w)| w).sum();
        if total > 0.0 {
            for (_, w) in &mut grid {
                *w /= total;
            }
        }
        grid
    }
}

/// One frame's raw estimate, before the voiced track is median-filtered.
/// `freq_hz` is the MAP candidate as measured; the emitted [`F0Frame`] carries
/// the smoothed, voicing-gated value.
struct RawFrame {
    time_s: f64,
    freq_hz: f32,
    voicing: f32,
}

/// Estimate `(f0_hz, voicing)` for one frame. `voicing == 0.0` (with
/// `f0 == 0.0`) means no periodic candidate was found.
fn analyse_frame(
    frame: &[f32],
    w: usize,
    tau_min: usize,
    tau_max: usize,
    sr: f32,
    thresholds: &[(f32, f32)],
) -> (f32, f32) {
    let diff = difference_function(frame, w, tau_max);
    let cmnd = cumulative_mean_normalized(&diff);
    let troughs = find_troughs(&cmnd, tau_min, tau_max);
    if troughs.is_empty() {
        return (0.0, 0.0);
    }

    // For each threshold, the first (lowest-lag) trough below it wins that
    // threshold's Beta mass — YIN's "first dip under the threshold" rule, run
    // across the whole prior instead of at one fixed value.
    let mut trough_prob = vec![0.0f32; troughs.len()];
    let mut voiced_prob = 0.0f32;
    for &(threshold, weight) in thresholds {
        if let Some(idx) = troughs.iter().position(|&t| cmnd[t] < threshold) {
            trough_prob[idx] += weight;
            voiced_prob += weight;
        }
    }
    if voiced_prob <= 0.0 {
        return (0.0, 0.0);
    }

    // MAP estimate: the candidate that accumulated the most probability.
    let best = (0..troughs.len())
        .max_by(|&a, &b| {
            trough_prob[a]
                .partial_cmp(&trough_prob[b])
                .expect("no NaNs")
        })
        .expect("troughs non-empty");
    let lag = parabolic_refine(&cmnd, troughs[best]);
    (sr / lag, voiced_prob)
}

/// YIN difference function `d(tau)` over the first `w` samples, for lags
/// `1..=tau_max`. `d[0]` is unused (`0.0`). Requires `frame.len() >= w +
/// tau_max`.
fn difference_function(frame: &[f32], w: usize, tau_max: usize) -> Vec<f32> {
    let mut d = vec![0.0f32; tau_max + 1];
    for (tau, slot) in d.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0f32;
        for j in 0..w {
            let delta = frame[j] - frame[j + tau];
            sum += delta * delta;
        }
        *slot = sum;
    }
    d
}

/// YIN cumulative-mean-normalized difference: `d'(tau) = d(tau) * tau /
/// sum_{k=1..=tau} d(k)`, with `d'(0) = 1`. Values near 0 mark strong
/// periodicity at that lag.
fn cumulative_mean_normalized(d: &[f32]) -> Vec<f32> {
    let mut cmnd = vec![1.0f32; d.len()];
    let mut running = 0.0f32;
    for tau in 1..d.len() {
        running += d[tau];
        cmnd[tau] = if running > 0.0 {
            d[tau] * tau as f32 / running
        } else {
            1.0
        };
    }
    cmnd
}

/// Local minima of the CMND within `[tau_min, tau_max]`, in ascending-lag
/// (descending-frequency) order — so "first trough below threshold" prefers the
/// highest plausible pitch, matching YIN.
fn find_troughs(cmnd: &[f32], tau_min: usize, tau_max: usize) -> Vec<usize> {
    let lo = tau_min.max(1);
    let hi = tau_max.min(cmnd.len().saturating_sub(2));
    let mut troughs = Vec::new();
    for tau in lo..=hi {
        if cmnd[tau] < cmnd[tau - 1] && cmnd[tau] <= cmnd[tau + 1] {
            troughs.push(tau);
        }
    }
    troughs
}

/// Parabolic interpolation around a trough for sub-sample lag precision. Fits a
/// parabola through `(tau-1, tau, tau+1)` and returns the refined lag.
fn parabolic_refine(cmnd: &[f32], tau: usize) -> f32 {
    if tau == 0 || tau + 1 >= cmnd.len() {
        return tau as f32;
    }
    let (a, b, c) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
    let denom = a + c - 2.0 * b;
    if denom.abs() < 1e-12 {
        tau as f32
    } else {
        tau as f32 + 0.5 * (a - c) / denom
    }
}

/// Unnormalized Beta(`a`, `b`) density at `x` — the threshold grid renormalizes,
/// so the missing `1 / B(a, b)` constant doesn't matter. `0` outside `(0, 1)`.
fn beta_pdf(x: f32, a: f32, b: f32) -> f32 {
    if x <= 0.0 || x >= 1.0 {
        return 0.0;
    }
    x.powf(a - 1.0) * (1.0 - x).powf(b - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 44_100;

    /// A vowel-ish tone: fundamental plus a couple of harmonics. No vibrato, so
    /// steady-tone assertions can be tight.
    fn tone(freq: f32, dur_s: f32, sr: u32) -> Vec<f32> {
        let n = (dur_s * sr as f32) as usize;
        (0..n)
            .map(|i| {
                let phase = 2.0 * PI * freq * i as f32 / sr as f32;
                0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin()
            })
            .collect()
    }

    /// Median frequency over frames the tracker considers voiced.
    fn median_voiced_hz(frames: &[F0Frame]) -> f32 {
        let mut voiced: Vec<f32> = frames
            .iter()
            .filter(|f| f.voicing >= 0.5 && f.frequency_hz > 0.0)
            .map(|f| f.frequency_hz)
            .collect();
        assert!(!voiced.is_empty(), "expected some voiced frames");
        voiced.sort_by(|a, b| a.partial_cmp(b).unwrap());
        voiced[voiced.len() / 2]
    }

    fn voiced_fraction(frames: &[F0Frame]) -> f32 {
        let voiced = frames.iter().filter(|f| f.voicing >= 0.5).count();
        voiced as f32 / frames.len().max(1) as f32
    }

    #[test]
    fn cmnd_first_trough_sits_at_the_period() {
        // The raw difference function dips near-zero at *every* period multiple,
        // so YIN normalizes (CMND) and takes the *first* trough below threshold
        // — that's the lag the pitch is read from, and it must be one period.
        let sr = SR as f32;
        let freq = 440.0;
        let frame: Vec<f32> = (0..2048)
            .map(|i| (2.0 * PI * freq * i as f32 / sr).sin())
            .collect();
        let cmnd = cumulative_mean_normalized(&difference_function(&frame, 1024, 1024));
        let troughs = find_troughs(&cmnd, 21, 1024);
        let first = *troughs
            .iter()
            .find(|&&t| cmnd[t] < 0.15)
            .expect("a strong trough exists");
        let period = (sr / freq).round() as usize; // ~100
        assert!(
            (first as i32 - period as i32).abs() <= 2,
            "first trough {first}, period {period}"
        );
    }

    #[test]
    fn cmnd_starts_at_one() {
        let d = vec![0.0, 4.0, 1.0, 9.0];
        let cmnd = cumulative_mean_normalized(&d);
        assert_eq!(cmnd[0], 1.0);
    }

    #[test]
    fn tracks_a_steady_tone() {
        let frames = PYin::default().track(&tone(440.0, 0.3, SR), SR);
        assert!(voiced_fraction(&frames) > 0.8, "mostly voiced");
        let hz = median_voiced_hz(&frames);
        assert!(
            (hz - 440.0).abs() / 440.0 < 0.02,
            "median {hz} Hz, want ~440"
        );
    }

    #[test]
    fn tracks_a_higher_tone() {
        // Guards against the search range / lag math only working near 440.
        let frames = PYin::default().track(&tone(880.0, 0.3, SR), SR);
        let hz = median_voiced_hz(&frames);
        assert!(
            (hz - 880.0).abs() / 880.0 < 0.02,
            "median {hz} Hz, want ~880"
        );
    }

    #[test]
    fn silence_reads_as_unvoiced() {
        let silence = vec![0.0f32; (0.3 * SR as f32) as usize];
        let frames = PYin::default().track(&silence, SR);
        assert!(!frames.is_empty());
        assert!(
            frames
                .iter()
                .all(|f| f.voicing < 0.5 && f.frequency_hz == 0.0),
            "silence must be fully unvoiced"
        );
    }

    #[test]
    fn separates_two_notes_across_a_rest() {
        // 440 (A4), a rest, then 587.33 (D5): the contour should show a voiced
        // cluster near each pitch and an unvoiced gap between.
        let mut audio = tone(440.0, 0.3, SR);
        audio.extend(vec![0.0f32; (0.15 * SR as f32) as usize]);
        audio.extend(tone(587.33, 0.3, SR));

        let frames = PYin::default().track(&audio, SR);
        let near = |target: f32| {
            frames
                .iter()
                .any(|f| f.voicing >= 0.5 && (f.frequency_hz - target).abs() / target < 0.03)
        };
        assert!(near(440.0), "expected a frame near 440 Hz");
        assert!(near(587.33), "expected a frame near 587 Hz");
        assert!(
            frames.iter().any(|f| f.voicing < 0.5),
            "expected an unvoiced rest between the notes"
        );
    }

    #[test]
    fn usable_as_a_trait_object() {
        let tracker: Box<dyn PitchTracker> = Box::new(PYin::default());
        let frames = tracker.track(&tone(440.0, 0.15, SR), SR);
        assert!(!frames.is_empty());
    }

    #[test]
    fn audio_shorter_than_a_frame_yields_no_frames() {
        assert!(PYin::default().track(&[0.0; 100], SR).is_empty());
    }
}
