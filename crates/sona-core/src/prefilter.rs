//! **Pitch pre-filter** — prepares the contour the note-HMM decodes over
//! (Phase B, ticket 04; [ADR-0010]). It sits between the [`PitchTracker`] and the
//! forthcoming note-HMM: `f0 contour → (time, semitone, voicing) frames`.
//!
//! [`PitchTracker`]: crate::pitch::PitchTracker
//! [ADR-0010]: ../../docs/adr/0010-note-hmm-segmentation.md
//!
//! Two jobs, both pure DSP:
//!
//! 1. A short **3-tap median** on the *voiced* f0 track. Isolated octave errors
//!    and single-frame jitter are lone outliers among their neighbours, so the
//!    window median replaces them — exactly what keeps the HMM's interval prior
//!    from having to reject them one by one downstream.
//! 2. Convert to the **semitone** representation the emission model reads: a
//!    Gaussian on the frame's deviation (in semitones) from a state's centre
//!    pitch (ADR-0010) needs its input already in semitone space, not Hz.
//!
//! ## Why a dedicated 3-tap, not pYIN's `median_window: 5`
//!
//! pYIN already median-filters its contour at width **5** ([ADR-0005]), so why
//! re-filter here? Two reasons, and the ticket asked this be recorded:
//!
//! - **SiPTH's caution (§5.3): wide medians eat short notes.** A width-5 window
//!   spans ~29 ms at the shared 256-sample hop; a width-3 spans ~17 ms. pYIN's
//!   5 is tuned for a clean *display* contour and the old plateau segmenter; the
//!   note-HMM wants the *narrowest* smoothing that still kills octave spikes, so
//!   a genuine short note survives to reach a state. 3 is that width.
//! - **Backend-independence.** This stage is defined over the [`F0Frame`]
//!   contract, not over pYIN. CREPE (ticket 11) emits the same contour but does
//!   **no** median filtering of its own; a basic-pitch adapter emits notes, not a
//!   raw contour. Owning the 3-tap median here means the HMM sees a
//!   bounded-jitter contour from *any* producer, instead of depending on a
//!   smoothing choice that only pYIN happens to make. For pYIN specifically the
//!   contour arrives already smoothed at 5, so the extra 3-tap is mostly a no-op
//!   (a narrower median over already-median-filtered data) — a cheap, harmless
//!   guarantee, not a second heavy smoothing pass.
//!
//! [ADR-0005]: ../../docs/adr/0005-pyin-hand-rolled-median-filtered.md
//!
//! ## Bilateral filter: deferred (seam only)
//!
//! ADR-0010 rejects a bilateral pitch pre-filter for now — unproven in this
//! domain, and its range kernel can freeze real fast transitions. It is **not
//! built here**; if 08's sweep asks for it, it slots in as an extra step between
//! the median and the semitone conversion (see [`PitchPreFilter::run`]). The
//! median is the only smoothing this stage performs.
//!
//! ## No behaviour change yet
//!
//! Ticket 04 stops at *making the pre-filtered contour available*. The
//! [`Segmenter`](crate::segment::Segmenter) still runs on the raw pYIN contour
//! ([`pipeline`](crate::pipeline) is untouched); the note-HMM that consumes these
//! [`PitchFrame`]s lands in tickets 05–07.

use crate::annotation::F0Frame;
use crate::pitch::median_filter_positive;
use crate::segment::hz_to_midi;

/// One frame of the pre-filtered contour: the note-HMM's input unit.
///
/// `semitone` is `None` where the frame has no pitch (unvoiced / the tracker
/// emitted no f0); a voiced frame carries its continuous MIDI semitone (69.0 =
/// A4). `voicing` is passed through **verbatim** from the source [`F0Frame`] so
/// the emission model's voicing prior (ADR-0010) can split probability mass to
/// the silence state — deciding voiced-vs-silence is the HMM's job, not this
/// stage's, so the pre-filter never thresholds it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchFrame {
    /// Frame time, in seconds from the start of the stem (carried through).
    pub time_s: f64,
    /// Continuous MIDI semitone of the (median-smoothed) f0, or `None` when the
    /// frame has no pitch.
    pub semitone: Option<f32>,
    /// Voiced/unvoiced confidence in `0.0..=1.0`, verbatim from the source frame.
    pub voicing: f32,
}

/// The pitch pre-filter (ticket 04, ADR-0010). Construct with
/// [`PitchPreFilter::default`] for the literature 3-tap width, or set
/// [`median_window`](PitchPreFilter::median_window) to tune.
#[derive(Debug, Clone)]
pub struct PitchPreFilter {
    /// Width, in frames, of the median filter applied to the voiced f0 track.
    /// Defaults to **3** — the narrowest that still erases isolated octave
    /// errors, chosen over pYIN's display-oriented 5 so short notes survive (see
    /// the module docs).
    pub median_window: usize,
}

impl Default for PitchPreFilter {
    fn default() -> Self {
        Self { median_window: 3 }
    }
}

impl PitchPreFilter {
    /// Pre-filter a `contour` into the note-HMM's `(time, semitone, voicing)`
    /// frames. Frame count and order are preserved 1:1 with the input.
    ///
    /// The median runs in **Hz space** over the voiced track (unvoiced frames,
    /// carried as `0.0`, are skipped so they neither gain a spurious pitch nor
    /// drag a neighbour's median). Since Hz→semitone is monotonic, the median of
    /// the frequencies equals the median of the semitones — filtering in Hz lets
    /// this reuse the tracker's tested [`median_filter_positive`], and the result
    /// is converted to semitones once, afterwards.
    ///
    /// A deferred bilateral pass (ADR-0010) would slot in here, between the
    /// median and the semitone conversion; it is not built.
    pub fn run(&self, contour: &[F0Frame]) -> Vec<PitchFrame> {
        // The f0 as a plain Hz track. A frame with no pitch carries 0.0 (the
        // F0Frame contract's unvoiced marker) and `median_filter_positive` skips
        // every non-positive sample, so unvoiced frames drop out of the median on
        // their own — no separate gating needed here. Voicing is independent of
        // this and the HMM interprets it separately.
        let hz_track: Vec<f32> = contour.iter().map(|f| f.frequency_hz).collect();
        let smoothed = median_filter_positive(&hz_track, self.median_window);

        contour
            .iter()
            .zip(smoothed)
            .map(|(f, hz)| PitchFrame {
                time_s: f.time_s,
                // Gate on the *input* frame's voiced status, not the smoothed
                // value: the median can bleed a neighbour's pitch into an
                // unvoiced boundary frame, and resurrecting a pitch there would
                // pre-empt the HMM's voiced-vs-silence decision. A voiced input
                // always yields a positive median (its own frame is in-window).
                semitone: if f.frequency_hz > 0.0 {
                    Some(hz_to_midi(hz))
                } else {
                    None
                },
                voicing: f.voicing,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP_S: f64 = 256.0 / 44_100.0; // the shared pYIN / onset hop, ~5.8 ms

    /// A voiced f0 frame at `hz` (0.0 = unvoiced) with a plausible voicing.
    fn frame(time_s: f64, hz: f32) -> F0Frame {
        F0Frame {
            time_s,
            frequency_hz: hz.max(0.0),
            voicing: if hz > 0.0 { 0.9 } else { 0.1 },
        }
    }

    /// Build a contour at the shared hop from a list of Hz values (0.0 unvoiced).
    fn contour(hz: &[f32]) -> Vec<F0Frame> {
        hz.iter()
            .enumerate()
            .map(|(i, &h)| frame(i as f64 * HOP_S, h))
            .collect()
    }

    #[test]
    fn converts_voiced_hz_to_semitone() {
        // 440 Hz -> A4 -> semitone 69; 880 Hz -> A5 -> 81.
        let out = PitchPreFilter::default().run(&contour(&[440.0, 440.0, 880.0, 880.0]));
        // Neighbours share the pitch, so the median leaves them; endpoints too.
        assert!((out[0].semitone.unwrap() - 69.0).abs() < 1e-3, "{:?}", out[0]);
        assert!((out[3].semitone.unwrap() - 81.0).abs() < 1e-3, "{:?}", out[3]);
    }

    #[test]
    fn unvoiced_frame_has_no_semitone() {
        let out = PitchPreFilter::default().run(&contour(&[0.0, 0.0, 0.0]));
        assert!(out.iter().all(|f| f.semitone.is_none()), "{out:?}");
    }

    #[test]
    fn three_tap_median_erases_isolated_octave_spike() {
        // A single doubled frame amid a steady 440 Hz contour: the 3-tap median
        // must pull it back to A4 (69), not leave it up an octave at ~81.
        let out = PitchPreFilter::default().run(&contour(&[440.0, 440.0, 880.0, 440.0, 440.0]));
        assert!(
            (out[2].semitone.unwrap() - 69.0).abs() < 0.1,
            "octave spike survived: {:?}",
            out[2].semitone
        );
    }

    #[test]
    fn narrower_window_survives_a_short_note_that_a_wide_one_eats() {
        // A two-frame note (494 Hz, B4) embedded in steady A4 (440). This is
        // SiPTH's warning made concrete: in a width-5 window the pair is a 2-of-5
        // minority, so the median absorbs it back into A4; in the default width-3
        // it is a 2-of-3 majority and survives. Full A4 context on both sides
        // keeps the comparison clear of even-window boundary effects.
        let hz = [440.0, 440.0, 440.0, 493.88, 493.88, 440.0, 440.0, 440.0];
        let wide = PitchPreFilter { median_window: 5 }.run(&contour(&hz));
        let narrow = PitchPreFilter::default().run(&contour(&hz)); // width 3

        let is_b4 = |f: &PitchFrame| (f.semitone.unwrap() - 71.0).abs() < 0.5;
        assert!(
            !wide.iter().any(is_b4),
            "width-5 was expected to eat the short note, but B4 survived: {wide:?}"
        );
        assert!(
            narrow.iter().any(is_b4),
            "width-3 must keep the short note; none near B4: {narrow:?}"
        );
    }

    #[test]
    fn carries_voicing_and_time_verbatim() {
        let mut c = contour(&[440.0, 0.0, 440.0]);
        c[1].voicing = 0.37; // an unvoiced-ish frame with a specific voicing
        let out = PitchPreFilter::default().run(&c);
        assert_eq!(out.len(), c.len());
        for (o, i) in out.iter().zip(&c) {
            assert_eq!(o.time_s, i.time_s);
            assert_eq!(o.voicing, i.voicing);
        }
        // The gated-out middle frame carries voicing but no pitch.
        assert_eq!(out[1].voicing, 0.37);
        assert!(out[1].semitone.is_none());
    }

    #[test]
    fn unvoiced_neighbour_does_not_drag_a_voiced_median() {
        // A voiced frame bordered by unvoiced zeros must keep its own pitch, not
        // be pulled toward silence — median_filter_positive skips the zeros.
        let out = PitchPreFilter::default().run(&contour(&[0.0, 440.0, 0.0]));
        assert!((out[1].semitone.unwrap() - 69.0).abs() < 1e-3, "{:?}", out[1]);
    }

    #[test]
    fn empty_contour_yields_empty_output() {
        assert!(PitchPreFilter::default().run(&[]).is_empty());
    }

    #[test]
    fn frame_count_is_preserved() {
        let out = PitchPreFilter::default().run(&contour(&[440.0, 0.0, 587.33, 587.33, 0.0]));
        assert_eq!(out.len(), 5);
    }
}
