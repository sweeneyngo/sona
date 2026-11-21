//! **Harmonic-sum pitch salience** — several pitch candidates per frame, for
//! stems where more than one voice sings.
//!
//! YIN reads one period from a frame; two voices make the frame aperiodic, so
//! pYIN either unvoices it or picks one voice. Harmonic summation (Klapuri 2006;
//! the salience function of Salamon & Gómez's Melodia, 2012) instead scores
//! every f0 by the spectral peaks at its harmonics. Each voice's harmonic series
//! raises its own f0, so two voices give two salience peaks, and a decoder with
//! a continuity prior can follow one of them.
//!
//! Per frame: a Hann window, a zero-padded FFT, spectral peaks (parabolically
//! interpolated, within `peak_floor_db` of the frame's loudest), and, for each
//! peak at `f` and each harmonic number `h`, a vote `a^β · α^(h−1)` spread with
//! a `cos²` kernel over ±1 semitone around `f / h` on a `1/bins_per_semitone`
//! semitone axis. The salience function's local maxima are the candidates.
//!
//! Frames align one-to-one with [`PYin`](super::PYin)'s at the same
//! `frame_length` and `hop_length` (a frame starts at `k · hop_length`).
//! Pure Rust: the FFT is a small radix-2 transform, no dependency.

use super::PitchCandidate;

/// Harmonic-sum salience over a semitone-resolution f0 axis.
#[derive(Debug, Clone, PartialEq)]
pub struct HarmonicSalience {
    /// Lowest f0 scored, Hz.
    pub fmin: f32,
    /// Highest f0 scored, Hz.
    pub fmax: f32,
    /// Analysis window, samples (match pYIN's to align frames).
    pub frame_length: usize,
    /// Hop, samples (match pYIN's).
    pub hop_length: usize,
    /// FFT size (power of two, `>= frame_length`): zero padding for finer peaks.
    pub fft_size: usize,
    /// Harmonics a peak votes for.
    pub n_harmonics: usize,
    /// Weight decay per harmonic number, `α`.
    pub harmonic_decay: f32,
    /// Exponent on a peak's magnitude, `β`.
    pub magnitude_power: f32,
    /// Peaks quieter than the frame's loudest by more than this are ignored.
    pub peak_floor_db: f32,
    /// Salience axis resolution.
    pub bins_per_semitone: usize,
    /// Candidates kept per frame, strongest first.
    pub max_candidates: usize,
}

impl Default for HarmonicSalience {
    fn default() -> Self {
        Self {
            fmin: 65.0,
            fmax: 1400.0,
            frame_length: 2048,
            hop_length: 256,
            fft_size: 4096,
            n_harmonics: 10,
            harmonic_decay: 0.8,
            magnitude_power: 1.0,
            peak_floor_db: 40.0,
            bins_per_semitone: 5,
            max_candidates: 5,
        }
    }
}

impl HarmonicSalience {
    /// Candidates for every frame (`k · hop_length` onward while a whole window
    /// fits), strongest first. A silent frame has none.
    pub fn track(&self, samples: &[f32], sample_rate: u32) -> Vec<Vec<PitchCandidate>> {
        let sr = sample_rate as f32;
        let n = self.fft_size.max(self.frame_length).next_power_of_two();
        let fft = Fft::new(n);
        let window: Vec<f32> = (0..self.frame_length)
            .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / self.frame_length as f32).cos())
            .collect();
        let bps = self.bins_per_semitone.max(1) as f32;
        let n_bins = (12.0 * bps * (self.fmax / self.fmin).log2()).ceil().max(1.0) as usize + 1;
        let bin_of = |f: f32| 12.0 * bps * (f / self.fmin).log2();
        let hz_of = |b: f32| self.fmin * 2f32.powf(b / (12.0 * bps));

        let mut re = vec![0.0f32; n];
        let mut im = vec![0.0f32; n];
        let mut mag = vec![0.0f32; n / 2 + 1];
        let mut sal = vec![0.0f32; n_bins];
        let mut out = Vec::new();
        let mut start = 0;
        while start + self.frame_length <= samples.len() {
            for (i, slot) in re.iter_mut().enumerate() {
                *slot = if i < self.frame_length { samples[start + i] * window[i] } else { 0.0 };
            }
            im.iter_mut().for_each(|v| *v = 0.0);
            fft.run(&mut re, &mut im);
            for (k, m) in mag.iter_mut().enumerate() {
                *m = (re[k] * re[k] + im[k] * im[k]).sqrt();
            }
            let peaks = spectral_peaks(&mag, sr / n as f32, self.peak_floor_db, self.fmin, self.fmax * self.n_harmonics as f32);
            sal.iter_mut().for_each(|v| *v = 0.0);
            for &(f, a) in &peaks {
                let amp = a.powf(self.magnitude_power);
                let mut weight = amp;
                for h in 1..=self.n_harmonics {
                    let bc = bin_of(f / h as f32);
                    if bc < -bps {
                        break;
                    }
                    let lo = (bc - bps).ceil().max(0.0) as usize;
                    let hi = ((bc + bps).floor() as isize).min(n_bins as isize - 1);
                    for b in lo as isize..=hi {
                        let d = (b as f32 - bc).abs() / bps;
                        let g = (d * std::f32::consts::FRAC_PI_2).cos();
                        sal[b as usize] += weight * g * g;
                    }
                    weight *= self.harmonic_decay;
                }
            }
            let mut cands: Vec<PitchCandidate> = (1..n_bins.saturating_sub(1))
                .filter(|&b| sal[b] > 0.0 && sal[b] >= sal[b - 1] && sal[b] > sal[b + 1])
                .map(|b| {
                    let (a, m, c) = (sal[b - 1], sal[b], sal[b + 1]);
                    let den = a + c - 2.0 * m;
                    let off = if den.abs() > 1e-12 { (0.5 * (a - c) / den).clamp(-0.5, 0.5) } else { 0.0 };
                    PitchCandidate { frequency_hz: hz_of(b as f32 + off), salience: m }
                })
                .filter(|c| c.frequency_hz <= self.fmax)
                .collect();
            cands.sort_by(|a, b| b.salience.total_cmp(&a.salience));
            cands.truncate(self.max_candidates);
            out.push(cands);
            start += self.hop_length;
        }
        out
    }
}

/// Local maxima of a magnitude spectrum within `[f_lo, f_hi]` Hz and within
/// `floor_db` of the loudest, as `(Hz, magnitude)` with parabolic refinement on
/// the log magnitude.
fn spectral_peaks(mag: &[f32], bin_hz: f32, floor_db: f32, f_lo: f32, f_hi: f32) -> Vec<(f32, f32)> {
    let k0 = ((f_lo * 0.9 / bin_hz).floor() as usize).max(1);
    let k1 = ((f_hi / bin_hz).ceil() as usize).min(mag.len().saturating_sub(2));
    if k1 <= k0 {
        return Vec::new();
    }
    let peak = mag[k0..=k1].iter().copied().fold(0.0f32, f32::max);
    if peak <= 1e-9 {
        return Vec::new();
    }
    let floor = peak * 10f32.powf(-floor_db / 20.0);
    let mut out = Vec::new();
    for k in k0..=k1 {
        let m = mag[k];
        if m >= floor && m > mag[k - 1] && m >= mag[k + 1] {
            let (a, b, c) = (mag[k - 1].max(1e-12).ln(), m.ln(), mag[k + 1].max(1e-12).ln());
            let den = a + c - 2.0 * b;
            let off = if den.abs() > 1e-12 { (0.5 * (a - c) / den).clamp(-0.5, 0.5) } else { 0.0 };
            let amp = (b - 0.25 * (a - c) * off).exp();
            out.push(((k as f32 + off) * bin_hz, amp));
        }
    }
    out
}

/// An in-place iterative radix-2 complex FFT of one fixed power-of-two size.
struct Fft {
    n: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    fn new(n: usize) -> Self {
        let bits = n.trailing_zeros();
        let rev = (0..n).map(|i| if bits == 0 { 0 } else { i.reverse_bits() >> (usize::BITS - bits) }).collect();
        let (cos, sin) = (0..n / 2)
            .map(|k| {
                let a = -std::f64::consts::TAU * k as f64 / n as f64;
                (a.cos() as f32, a.sin() as f32)
            })
            .unzip();
        Fft { n, cos, sin, rev }
    }

    fn run(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        for i in 0..n {
            let j = self.rev[i];
            if j > i {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for s in (0..n).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], self.sin[k * step]);
                    let (a, b) = (s + k, s + k + half);
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 44_100;

    fn voice(freq: f32, amp: f32, dur_s: f32) -> Vec<f32> {
        let n = (dur_s * SR as f32) as usize;
        (0..n)
            .map(|i| {
                let ph = 2.0 * PI * freq * i as f32 / SR as f32;
                amp * (0.6 * ph.sin() + 0.3 * (2.0 * ph).sin() + 0.15 * (3.0 * ph).sin() + 0.08 * (4.0 * ph).sin())
            })
            .collect()
    }

    fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
        a.iter().zip(b).map(|(x, y)| x + y).collect()
    }

    fn semis(f: f32) -> f32 {
        69.0 + 12.0 * (f / 440.0).log2()
    }

    /// Fraction of frames where some candidate lies within 0.5 semitone of `midi`.
    fn found(frames: &[Vec<PitchCandidate>], midi: f32) -> f32 {
        let hit = frames.iter().filter(|c| c.iter().any(|c| (semis(c.frequency_hz) - midi).abs() < 0.5)).count();
        hit as f32 / frames.len().max(1) as f32
    }

    #[test]
    fn fft_matches_a_direct_dft() {
        let n = 16;
        let x: Vec<f32> = (0..n).map(|i| ((i * 7 % 5) as f32 - 2.0) * 0.3).collect();
        let (mut re, mut im) = (x.clone(), vec![0.0; n]);
        Fft::new(n).run(&mut re, &mut im);
        for k in 0..n {
            let (mut r, mut i) = (0.0f64, 0.0f64);
            for (t, &v) in x.iter().enumerate() {
                let a = -std::f64::consts::TAU * (k * t) as f64 / n as f64;
                r += v as f64 * a.cos();
                i += v as f64 * a.sin();
            }
            assert!((re[k] as f64 - r).abs() < 1e-4 && (im[k] as f64 - i).abs() < 1e-4, "bin {k}");
        }
    }

    #[test]
    fn a_single_voice_tops_the_salience_at_its_f0() {
        let frames = HarmonicSalience::default().track(&voice(330.0, 0.5, 0.3), SR);
        assert!(!frames.is_empty());
        let top_ok = frames.iter().filter(|c| c.first().is_some_and(|c| (semis(c.frequency_hz) - semis(330.0)).abs() < 0.3)).count();
        assert!(top_ok as f32 > 0.9 * frames.len() as f32, "{top_ok} of {}", frames.len());
    }

    #[test]
    fn two_voices_a_fifth_apart_each_keep_a_candidate() {
        // A4 lead over a D4 harmony (a fifth below), equally loud: both f0s
        // must survive as candidates in nearly every frame.
        let (lead, harm) = (voice(440.0, 0.4, 0.3), voice(293.66, 0.4, 0.3));
        let frames = HarmonicSalience::default().track(&mix(&lead, &harm), SR);
        assert!(found(&frames, 69.0) > 0.9, "lead {}", found(&frames, 69.0));
        assert!(found(&frames, 62.0) > 0.9, "harmony {}", found(&frames, 62.0));
    }

    #[test]
    fn silence_has_no_candidates() {
        let frames = HarmonicSalience::default().track(&vec![0.0; 8192], SR);
        assert!(!frames.is_empty() && frames.iter().all(|c| c.is_empty()));
    }

    #[test]
    fn frames_align_with_pyin() {
        let x = voice(220.0, 0.5, 0.5);
        let pyin = super::super::PYin::default();
        use super::super::PitchTracker;
        assert_eq!(HarmonicSalience::default().track(&x, SR).len(), pyin.track(&x, SR).len());
    }
}
