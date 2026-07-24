//! **Fixture synthesis** — the single source of truth for the golden fixtures.
//!
//! Each fixture is described declaratively as a list of [`Segment`]s (silence or
//! a voiced tone). From one spec we render *both* the audio and its ground-truth
//! [`Reference`], so the WAV and the expected notes/f0 can never drift apart —
//! regenerate with the `sona-eval-gen-fixtures` binary.
//!
//! The scenarios mirror the core's own hard cases so the harness exercises what
//! segmentation actually struggles with (ADR-0010/0011): **legato** (pitch
//! change at steady energy — no re-attack), **staccato** (clean gaps),
//! **vibrato** (one note that must not over-split), and **bleed** (a note over
//! near-floor broadband noise that must not spawn spurious notes).

use crate::reference::{RefF0, RefNote, Reference};

/// Fixture sample rate. The real datasets are 44.1 kHz; match them.
pub const SAMPLE_RATE: u32 = 44_100;
/// Ground-truth f0 hop, in samples — the shared pYIN/onset hop, so the reference
/// contour lands on the same time base the pipeline reports.
pub const HOP: usize = 256;

/// A voiced tone: a fundamental (plus two harmonics for a vowel-ish timbre),
/// optionally with sinusoidal vibrato.
#[derive(Debug, Clone, Copy)]
pub struct Voice {
    /// Fundamental frequency, in Hz.
    pub hz: f32,
    /// Peak amplitude in `0.0..=1.0`.
    pub amp: f32,
    /// Vibrato rate, in Hz (`0.0` for none).
    pub vibrato_hz: f32,
    /// Vibrato depth, in cents (peak deviation; `0.0` for none).
    pub vibrato_cents: f32,
}

impl Voice {
    /// A steady (vibrato-free) tone.
    pub fn steady(hz: f32, amp: f32) -> Self {
        Voice { hz, amp, vibrato_hz: 0.0, vibrato_cents: 0.0 }
    }

    /// Instantaneous frequency at time `t`, applying vibrato.
    fn freq_at(&self, t: f64) -> f32 {
        if self.vibrato_hz == 0.0 || self.vibrato_cents == 0.0 {
            return self.hz;
        }
        let cents = self.vibrato_cents as f64
            * (std::f64::consts::TAU * self.vibrato_hz as f64 * t).sin();
        (self.hz as f64 * 2f64.powf(cents / 1200.0)) as f32
    }
}

/// A span of the fixture: either silence (`voice = None`) or a voiced tone.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    /// Start time, in seconds.
    pub start_s: f64,
    /// End time, in seconds.
    pub end_s: f64,
    /// The tone sounding across this span, or `None` for silence.
    pub voice: Option<Voice>,
}

impl Segment {
    fn silence(start_s: f64, end_s: f64) -> Self {
        Segment { start_s, end_s, voice: None }
    }

    fn tone(start_s: f64, end_s: f64, voice: Voice) -> Self {
        Segment { start_s, end_s, voice: Some(voice) }
    }
}

/// A complete fixture: a named list of segments plus optional broadband bleed.
#[derive(Debug, Clone)]
pub struct FixtureSpec {
    /// Fixture name (the committed file stem).
    pub name: &'static str,
    /// Ordered, contiguous segments covering `[0, duration_s]`.
    pub segments: Vec<Segment>,
    /// Peak amplitude of deterministic broadband bleed added across the whole
    /// clip (`0.0` for none) — stands in for separation artefacts.
    pub bleed_amp: f32,
}

impl FixtureSpec {
    /// Total duration, in seconds (the last segment's end).
    pub fn duration_s(&self) -> f64 {
        self.segments.last().map(|s| s.end_s).unwrap_or(0.0)
    }

    /// Render the audio for this fixture as mono `f32` samples in `-1.0..=1.0`.
    ///
    /// Phase is continuous across adjacent voiced segments (so a legato pitch
    /// change carries no attack transient) and reset through silence (so each
    /// note after a rest starts with a real attack).
    pub fn render_samples(&self) -> Vec<f32> {
        let sr = SAMPLE_RATE as f64;
        let n = (self.duration_s() * sr).round() as usize;
        let mut samples = vec![0.0f32; n];
        let mut phase = 0.0f64; // fundamental phase, radians
        let mut noise = Lcg::new(0x5011_A93D);

        for (i, out) in samples.iter_mut().enumerate() {
            let t = i as f64 / sr;
            let mut value = 0.0f32;
            match self.voice_at(t) {
                Some(voice) => {
                    let f = voice.freq_at(t) as f64;
                    phase += std::f64::consts::TAU * f / sr;
                    let p = phase;
                    value = voice.amp
                        * (0.6 * p.sin() + 0.3 * (2.0 * p).sin() + 0.1 * (3.0 * p).sin()) as f32;
                }
                None => phase = 0.0, // silence: next attack starts clean
            }
            if self.bleed_amp > 0.0 {
                value += self.bleed_amp * noise.next_bipolar();
            }
            *out = value.clamp(-1.0, 1.0);
        }
        samples
    }

    /// The ground-truth [`Reference`] for this fixture: one note per voiced
    /// segment, and an f0 frame every [`HOP`] samples.
    pub fn reference(&self) -> Reference {
        let notes = self
            .segments
            .iter()
            .filter_map(|seg| seg.voice.map(|v| RefNote::from_hz(seg.start_s, seg.end_s, v.hz)))
            .collect();

        let sr = SAMPLE_RATE as f64;
        let n = (self.duration_s() * sr).round() as usize;
        let mut f0 = Vec::new();
        let mut i = 0;
        while i < n {
            let t = i as f64 / sr;
            let frame = match self.voice_at(t) {
                Some(voice) => RefF0 { time_s: t, frequency_hz: voice.freq_at(t), voiced: true },
                None => RefF0 { time_s: t, frequency_hz: 0.0, voiced: false },
            };
            f0.push(frame);
            i += HOP;
        }
        Reference { notes, f0 }
    }

    /// The voice sounding at time `t`, if any.
    fn voice_at(&self, t: f64) -> Option<Voice> {
        self.segments
            .iter()
            .find(|s| t >= s.start_s && t < s.end_s)
            .and_then(|s| s.voice)
    }
}

/// The four committed golden fixtures (ADR-0011).
pub fn fixture_specs() -> Vec<FixtureSpec> {
    let a4 = 440.0;
    let b4 = 493.88;
    let c5 = 523.25;
    let d5 = 587.33;

    vec![
        // Legato: two slurred notes, steady energy, no gap — the pitch change
        // has no onset, so segmentation must split it on pitch alone.
        FixtureSpec {
            name: "legato",
            segments: vec![
                Segment::tone(0.0, 0.5, Voice::steady(a4, 0.5)),
                Segment::tone(0.5, 1.0, Voice::steady(b4, 0.5)),
            ],
            bleed_amp: 0.0,
        },
        // Staccato: three short notes with clean rests — the easy case.
        FixtureSpec {
            name: "staccato",
            segments: vec![
                Segment::silence(0.0, 0.1),
                Segment::tone(0.1, 0.3, Voice::steady(a4, 0.5)),
                Segment::silence(0.3, 0.45),
                Segment::tone(0.45, 0.65, Voice::steady(c5, 0.5)),
                Segment::silence(0.65, 0.8),
                Segment::tone(0.8, 1.0, Voice::steady(d5, 0.5)),
            ],
            bleed_amp: 0.0,
        },
        // Vibrato: one sustained note with ±40-cent, 6 Hz vibrato — must stay one
        // note, not a stutter of semitone fragments.
        FixtureSpec {
            name: "vibrato",
            segments: vec![
                Segment::silence(0.0, 0.1),
                Segment::tone(
                    0.1,
                    1.0,
                    Voice { hz: a4, amp: 0.5, vibrato_hz: 6.0, vibrato_cents: 40.0 },
                ),
            ],
            bleed_amp: 0.0,
        },
        // Bleed: one clear note over near-floor broadband noise across the whole
        // clip — the noise must not spawn spurious notes.
        FixtureSpec {
            name: "bleed",
            segments: vec![
                Segment::silence(0.0, 0.35),
                Segment::tone(0.35, 0.65, Voice::steady(a4, 0.5)),
                Segment::silence(0.65, 1.0),
            ],
            bleed_amp: 0.02,
        },
    ]
}

/// A tiny linear-congruential generator for deterministic broadband bleed — no
/// `rand` dependency, and reproducible fixtures across machines.
struct Lcg {
    state: u32,
}

impl Lcg {
    fn new(seed: u32) -> Self {
        Lcg { state: seed }
    }

    /// Next value in `[-1.0, 1.0)`.
    fn next_bipolar(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let u = (self.state >> 8) as f32 / (1u32 << 24) as f32; // [0, 1)
        2.0 * u - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_render_expected_durations() {
        for spec in fixture_specs() {
            let samples = spec.render_samples();
            let expected = (spec.duration_s() * SAMPLE_RATE as f64).round() as usize;
            assert_eq!(samples.len(), expected, "{}", spec.name);
            assert!(samples.iter().all(|s| (-1.0..=1.0).contains(s)));
        }
    }

    #[test]
    fn reference_note_counts_match_voiced_segments() {
        let by_name = |name: &str| fixture_specs().into_iter().find(|s| s.name == name).unwrap();
        assert_eq!(by_name("legato").reference().notes.len(), 2);
        assert_eq!(by_name("staccato").reference().notes.len(), 3);
        assert_eq!(by_name("vibrato").reference().notes.len(), 1);
        assert_eq!(by_name("bleed").reference().notes.len(), 1);
    }

    #[test]
    fn legato_notes_are_a4_then_b4() {
        let reference = fixture_specs().into_iter().find(|s| s.name == "legato").unwrap().reference();
        assert_eq!(reference.notes[0].midi_pitch, 69); // A4
        assert_eq!(reference.notes[1].midi_pitch, 71); // B4
        // Seamless: note 0 ends where note 1 begins.
        assert!((reference.notes[0].offset_s - reference.notes[1].onset_s).abs() < 1e-9);
    }

    #[test]
    fn vibrato_f0_wobbles_but_stays_near_a4() {
        let reference =
            fixture_specs().into_iter().find(|s| s.name == "vibrato").unwrap().reference();
        let voiced: Vec<f32> =
            reference.f0.iter().filter(|f| f.voiced).map(|f| f.frequency_hz).collect();
        assert!(voiced.len() > 10);
        let min = voiced.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = voiced.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        // Vibrato is present (spread > 0) but bounded within a semitone. ±40
        // cents spans ~4.7% peak-to-peak; a semitone is ~5.95% (ratio 1.0595).
        assert!(max > min, "vibrato should modulate f0");
        assert!(max / min < 1.0595, "±40 cents stays under a semitone span");
    }
}
