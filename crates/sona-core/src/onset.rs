//! **Onset detection + energy envelope** — the second stage of the perception
//! pipeline (spec §Pipeline 2), landed by ticket 04.
//!
//! Two outputs, both raw perception (no grid, no BPM):
//!
//! - The **energy/RMS envelope** ([`EnergyFrame`]s): a per-frame loudness track.
//!   It is a display aid in the annotation JSON (ADR-0004) *and* the raw material
//!   segmentation reads for offset steepness (ticket 05) and the confidence stage
//!   reads for the bleed guard (ticket 06).
//! - The **onsets** ([`Onset`]s): the times a new note attacks, each with a
//!   `strength` that onset-clarity confidence (ticket 06) later scores.
//!
//! ## How onsets are found
//!
//! Target material is staccato Vocaloid: notes are short, gaps are clean, and
//! attacks are sharp energy *rises* from near the noise floor (CONTEXT.md). So
//! the detector works on the **energy envelope**, not a spectrogram — a
//! half-wave-rectified difference of log-energy is the onset novelty, and its
//! prominent peaks are the onsets. Re-attacks at the *same* pitch (the case pure
//! pitch tracking misses) are exactly energy rises, so this catches them; splits
//! between two *legato* pitches at steady energy are deliberately left to
//! segmentation's pitch-plateau step (ticket 05), not forced here. See ADR-0006.
//!
//! The done-criterion is "few false positives from **bleed**": separation bleed
//! sits near the noise floor, so a candidate whose energy never rises to a
//! fraction of the stem's peak is rejected ([`OnsetDetector::bleed_floor_frac`]).

use crate::annotation::EnergyFrame;

/// A detected note attack. `time_s` is the onset in seconds from the start of
/// the stem; `strength` is the (log-energy) novelty at the peak — a relative
/// measure of how sharp the attack was, consumed by onset-clarity confidence
/// (ticket 06), not a calibrated unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Onset {
    /// Onset time, in seconds from the start of the stem.
    pub time_s: f64,
    /// Novelty (log-energy rise) at the detected peak. Higher = sharper attack.
    pub strength: f32,
}

/// The onset stage's full output: the detected [`Onset`]s and the
/// [`EnergyFrame`] envelope they were derived from. Both feed later stages
/// (segmentation, confidence) and the envelope is emitted in the annotation JSON,
/// so they are returned together to avoid recomputing the envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct OnsetResult {
    /// Note attacks, in time order.
    pub onsets: Vec<Onset>,
    /// The per-frame energy/RMS envelope (annotation `energy_envelope`).
    pub energy_envelope: Vec<EnergyFrame>,
}

/// Energy-novelty onset detector (ADR-0006). Construct with
/// [`OnsetDetector::default`] for staccato-vocal defaults, or set fields to tune.
#[derive(Debug, Clone)]
pub struct OnsetDetector {
    /// Analysis window length in samples for the RMS envelope. Shorter = tighter
    /// time localization of an attack, at the cost of a noisier envelope.
    pub frame_length: usize,
    /// Envelope frames advance by this many samples; the frame rate is
    /// `sample_rate / hop_length`.
    pub hop_length: usize,
    /// Minimum spacing between accepted onsets, in seconds. Debounces the
    /// multi-frame ramp of a single attack into one onset.
    pub min_gap_s: f64,
    /// How far a novelty peak must clear the local mean novelty to count, in
    /// log-energy units. The adaptive part of the threshold rejects steady noise.
    pub threshold_delta: f32,
    /// Half-width, in seconds, of the local mean used for the adaptive threshold.
    pub threshold_window_s: f64,
    /// Bleed guard: an onset's post-attack RMS must reach at least this fraction
    /// of the stem's peak RMS. Separation bleed sits near the floor, so this
    /// culls it (spec done-criterion) without a note-level model.
    pub bleed_floor_frac: f32,
}

impl Default for OnsetDetector {
    fn default() -> Self {
        // Defaults retuned for general (expressive) vocals, not just staccato
        // Vocaloid: real attacks are softer and quieter passages must survive, so
        // the novelty threshold and the bleed floor are both lower than the
        // original staccato tuning (threshold_delta 0.6, bleed_floor_frac 0.1).
        Self {
            // ~23 ms at 44.1 kHz: short enough to localize an attack, long enough
            // for a stable RMS.
            frame_length: 1024,
            // ~5.8 ms hop — the same hop as the pYIN default, so the envelope and
            // f0 contour share a time base.
            hop_length: 256,
            min_gap_s: 0.05, // 50 ms: below a 16th at any sane tempo, above jitter
            // Lower so soft (non-percussive) vocal attacks still clear the local
            // mean; stationary bleed stays rejected because its novelty is ~0.
            threshold_delta: 0.3,
            threshold_window_s: 0.1,
            // Lower so quiet notes (a soft verse under a loud chorus) aren't culled
            // as bleed; true separation bleed still sits below this.
            bleed_floor_frac: 0.05,
        }
    }
}

/// Small constant folded into the RMS before the log so silence maps to a finite
/// (very negative) log-energy instead of `-inf`, keeping the novelty finite.
const LOG_ENERGY_EPS: f32 = 1e-6;

impl OnsetDetector {
    /// Compute the per-frame energy/RMS envelope of `samples` (mono) at
    /// `sample_rate` Hz. Each frame's `time_s` is the **center** of its analysis
    /// window, so an energy rise is timestamped where it actually happens rather
    /// than a half-window early. Returns empty if the stem is shorter than one
    /// frame.
    pub fn energy_envelope(&self, samples: &[f32], sample_rate: u32) -> Vec<EnergyFrame> {
        let sr = sample_rate as f64;
        let frame_length = self.frame_length.max(1);
        let hop = self.hop_length.max(1);

        let mut frames = Vec::new();
        let mut start = 0;
        while start + frame_length <= samples.len() {
            let frame = &samples[start..start + frame_length];
            let sum_sq: f64 = frame.iter().map(|&x| (x as f64) * (x as f64)).sum();
            let rms = (sum_sq / frame_length as f64).sqrt() as f32;
            let center = start as f64 + frame_length as f64 / 2.0;
            frames.push(EnergyFrame {
                time_s: center / sr,
                rms,
            });
            start += hop;
        }
        frames
    }

    /// Detect onsets over `samples` and return them alongside the energy envelope
    /// they were computed from.
    pub fn detect(&self, samples: &[f32], sample_rate: u32) -> OnsetResult {
        let energy_envelope = self.energy_envelope(samples, sample_rate);
        let onsets = self.onsets_from_envelope(&energy_envelope);
        OnsetResult {
            onsets,
            energy_envelope,
        }
    }

    /// Peak-pick onsets from a precomputed energy envelope. Exposed so a caller
    /// that already has the envelope (e.g. from a different producer) can reuse
    /// it without recomputing.
    pub fn onsets_from_envelope(&self, envelope: &[EnergyFrame]) -> Vec<Onset> {
        if envelope.len() < 2 {
            return Vec::new();
        }

        // Log-compress the RMS: attacks read as steps of comparable size whether
        // the note is loud or quiet, and the between-note floor is pushed far
        // enough down that its jitter doesn't rival a real attack.
        let log_energy: Vec<f32> = envelope
            .iter()
            .map(|f| (f.rms + LOG_ENERGY_EPS).ln())
            .collect();

        // Onset novelty: the half-wave-rectified rise in log-energy. Falls
        // (offsets) rectify to 0, so only attacks survive.
        let novelty: Vec<f32> = std::iter::once(0.0)
            .chain(log_energy.windows(2).map(|w| (w[1] - w[0]).max(0.0)))
            .collect();

        let peak_rms = envelope.iter().map(|f| f.rms).fold(0.0f32, f32::max);
        let bleed_floor = peak_rms * self.bleed_floor_frac;

        // Frame spacing from the envelope's own time base (don't assume the
        // detector's hop matches the envelope passed in).
        let frame_dt = (envelope[1].time_s - envelope[0].time_s).max(f64::MIN_POSITIVE);
        let win = ((self.threshold_window_s / frame_dt).round() as usize).max(1);
        let wait = ((self.min_gap_s / frame_dt).round() as usize).max(1);
        // Local extremum half-width: tight, just enough to reject shoulder frames
        // of a single ramp without merging distinct attacks.
        let neigh = 2usize;

        let mut onsets: Vec<Onset> = Vec::new();
        let mut last_accepted: Option<usize> = None;
        for i in 0..novelty.len() {
            let n = novelty[i];
            if n <= 0.0 {
                continue;
            }
            // Local maximum of the novelty in a small neighbourhood.
            let lo = i.saturating_sub(neigh);
            let hi = (i + neigh + 1).min(novelty.len());
            if novelty[lo..hi].iter().any(|&x| x > n) {
                continue;
            }
            // Adaptive threshold: clear the local mean novelty by `delta`.
            let wlo = i.saturating_sub(win);
            let whi = (i + win + 1).min(novelty.len());
            let local_mean = novelty[wlo..whi].iter().sum::<f32>() / (whi - wlo) as f32;
            if n < local_mean + self.threshold_delta {
                continue;
            }
            // Bleed guard: the attack must reach a real note level, not just a
            // rise within the noise floor. Look at the frames right after the
            // rise (the note body), not the mid-rise frame itself.
            let body_hi = (i + neigh + 1).min(envelope.len());
            let body_rms = envelope[i..body_hi]
                .iter()
                .map(|f| f.rms)
                .fold(0.0f32, f32::max);
            if body_rms < bleed_floor {
                continue;
            }
            // Debounce: keep one onset per attack.
            if let Some(prev) = last_accepted {
                if i - prev < wait {
                    // Same attack ramp: keep the stronger peak.
                    if let Some(last) = onsets.last_mut() {
                        if n > last.strength {
                            *last = Onset {
                                time_s: envelope[i].time_s,
                                strength: n,
                            };
                            last_accepted = Some(i);
                        }
                    }
                    continue;
                }
            }
            onsets.push(Onset {
                time_s: envelope[i].time_s,
                strength: n,
            });
            last_accepted = Some(i);
        }
        onsets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const SR: u32 = 44_100;

    /// A vowel-ish tone: fundamental plus two harmonics, amplitude `amp`.
    fn tone(freq: f32, dur_s: f32, amp: f32) -> Vec<f32> {
        let n = (dur_s * SR as f32) as usize;
        (0..n)
            .map(|i| {
                let phase = 2.0 * PI * freq * i as f32 / SR as f32;
                amp * (0.6 * phase.sin() + 0.3 * (2.0 * phase).sin() + 0.1 * (3.0 * phase).sin())
            })
            .collect()
    }

    fn silence(dur_s: f32) -> Vec<f32> {
        vec![0.0f32; (dur_s * SR as f32) as usize]
    }

    /// Deterministic low-level broadband "bleed": an LCG in `[-1, 1)` scaled to
    /// `amp`. Stands in for UVR separation artefacts without a rand dependency.
    fn bleed(dur_s: f32, amp: f32) -> Vec<f32> {
        let n = (dur_s * SR as f32) as usize;
        let mut state: u32 = 0x1234_5678;
        (0..n)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let u = (state >> 8) as f32 / (1u32 << 24) as f32; // [0,1)
                amp * (2.0 * u - 1.0)
            })
            .collect()
    }

    #[test]
    fn envelope_tracks_loud_then_quiet() {
        let mut audio = tone(440.0, 0.3, 0.6);
        audio.extend(silence(0.3));
        let env = OnsetDetector::default().energy_envelope(&audio, SR);
        assert!(!env.is_empty());

        let loud: Vec<f32> = env
            .iter()
            .filter(|f| f.time_s < 0.25)
            .map(|f| f.rms)
            .collect();
        let quiet: Vec<f32> = env
            .iter()
            .filter(|f| f.time_s > 0.35)
            .map(|f| f.rms)
            .collect();
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
        assert!(
            mean(&loud) > 0.1,
            "tone region should be loud: {}",
            mean(&loud)
        );
        assert!(
            mean(&quiet) < 0.01,
            "silence should be near zero: {}",
            mean(&quiet)
        );
    }

    #[test]
    fn envelope_frame_times_are_ordered_and_centered() {
        let env = OnsetDetector::default().energy_envelope(&tone(440.0, 0.2, 0.5), SR);
        // Centered: the first frame sits half a window in, not at t=0.
        let half_window = 1024.0 / 2.0 / SR as f64;
        assert!((env[0].time_s - half_window).abs() < 1e-9);
        for pair in env.windows(2) {
            assert!(pair[1].time_s > pair[0].time_s, "times strictly increasing");
        }
    }

    #[test]
    fn detects_a_single_onset_at_note_start() {
        let mut audio = silence(0.2);
        audio.extend(tone(440.0, 0.3, 0.6));
        let result = OnsetDetector::default().detect(&audio, SR);
        assert_eq!(
            result.onsets.len(),
            1,
            "one attack -> one onset: {:?}",
            result.onsets
        );
        assert!(
            (result.onsets[0].time_s - 0.2).abs() < 0.04,
            "onset near 0.2 s, got {}",
            result.onsets[0].time_s
        );
    }

    #[test]
    fn separates_three_staccato_notes() {
        // Three notes with clean gaps — the target-material easy case.
        let mut audio = silence(0.1);
        let starts = [0.1, 0.1 + 0.25 + 0.1, 0.1 + 2.0 * (0.25 + 0.1)];
        audio.extend(tone(440.0, 0.25, 0.6));
        audio.extend(silence(0.1));
        audio.extend(tone(523.25, 0.25, 0.6));
        audio.extend(silence(0.1));
        audio.extend(tone(587.33, 0.25, 0.6));

        let result = OnsetDetector::default().detect(&audio, SR);
        assert_eq!(
            result.onsets.len(),
            3,
            "expected 3 onsets: {:?}",
            result.onsets
        );
        for (onset, &start) in result.onsets.iter().zip(&starts) {
            assert!(
                (onset.time_s - start).abs() < 0.04,
                "onset {} should be near {}",
                onset.time_s,
                start
            );
        }
    }

    #[test]
    fn repeated_same_pitch_notes_each_onset() {
        // Two notes at the *same* pitch with a gap: pure pitch tracking would see
        // one plateau, but the re-attack is an energy rise the onset stage must
        // catch.
        let mut audio = silence(0.1);
        audio.extend(tone(440.0, 0.2, 0.6));
        audio.extend(silence(0.1));
        audio.extend(tone(440.0, 0.2, 0.6));
        let result = OnsetDetector::default().detect(&audio, SR);
        assert_eq!(result.onsets.len(), 2, "two attacks: {:?}", result.onsets);
    }

    #[test]
    fn low_level_bleed_produces_no_onsets() {
        // Stationary near-floor bleed with no note must not fire the detector.
        let audio = bleed(1.0, 0.02);
        let result = OnsetDetector::default().detect(&audio, SR);
        assert!(
            result.onsets.is_empty(),
            "bleed-only stem should yield no onsets: {:?}",
            result.onsets
        );
    }

    #[test]
    fn note_over_bleed_fires_only_on_the_note() {
        // Bleed throughout, one real note in the middle: exactly one onset, at
        // the note — the done-criterion in miniature.
        let mut audio = bleed(0.9, 0.02);
        let note = tone(440.0, 0.25, 0.6);
        let onset_sample = (0.3 * SR as f32) as usize;
        for (i, s) in note.iter().enumerate() {
            audio[onset_sample + i] += s;
        }
        let result = OnsetDetector::default().detect(&audio, SR);
        assert_eq!(result.onsets.len(), 1, "only the note: {:?}", result.onsets);
        assert!((result.onsets[0].time_s - 0.3).abs() < 0.04);
    }

    #[test]
    fn audio_shorter_than_a_frame_yields_nothing() {
        let result = OnsetDetector::default().detect(&[0.0; 100], SR);
        assert!(result.energy_envelope.is_empty());
        assert!(result.onsets.is_empty());
    }
}
