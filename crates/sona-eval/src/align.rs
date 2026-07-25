//! Aligning a producer's f0 contour to the reference time grid.
//!
//! The pitch metrics ([`crate::metrics::pitch`]) score two contours **frame for
//! frame**, so the estimate must first be resampled onto the reference's frame
//! times. `mir_eval` does this inside `melody.to_cent_voicing`; we factor it out
//! here so the metric itself stays a pure, aligned comparison.
//!
//! [`resample_f0_onto`] follows `mir_eval.melody.resample_melody_series`:
//! linear interpolation **in cent space**, unvoiced (0 Hz) frames retained as
//! gaps rather than interpolated through, and voicing carried zero-order (the
//! left sample). It differs deliberately in one place: reference times **outside
//! the estimate's covered span** are returned unvoiced, where `mir_eval`
//! extrapolates the last pitch across the tail. Holding a pitch over frames the
//! producer never analysed would invent voiced content; marking them unvoiced is
//! both cleaner and stricter (it can only *lower* pitch scores, never inflate
//! them). For a producer whose grid is a prefix of the reference's — pYIN on the
//! golden fixtures, which share the 256-sample hop — every in-span frame lands on
//! an exact reference time, so interpolation is a pass-through and only this tail
//! handling is in play.

use crate::metrics::pitch::{cents_to_hz, hz_to_cents};
use crate::reference::RefF0;

/// Resample `source` f0 frames onto `target_times`, returning one [`RefF0`] per
/// target time (so the result aligns with a reference contour sampled at those
/// times). See the module docs for the interpolation rules.
pub fn resample_f0_onto(target_times: &[f64], source: &[RefF0]) -> Vec<RefF0> {
    if source.is_empty() {
        return target_times
            .iter()
            .map(|&t| RefF0 { time_s: t, frequency_hz: 0.0, voiced: false })
            .collect();
    }

    // Source arrays in cent space. `cent` is 0 on unvoiced frames; `held` fills
    // those gaps forward with the last real pitch so linear interpolation across
    // a short unvoiced gap doesn't dip toward zero (mir_eval's `frequencies_held`).
    let times: Vec<f64> = source.iter().map(|f| f.time_s).collect();
    let voiced: Vec<bool> = source.iter().map(|f| f.voiced && f.frequency_hz > 0.0).collect();
    let cent: Vec<f64> = source
        .iter()
        .zip(&voiced)
        .map(|(f, &v)| if v { hz_to_cents(f.frequency_hz) } else { 0.0 })
        .collect();
    let mut held = cent.clone();
    for i in 1..held.len() {
        if cent[i] == 0.0 {
            held[i] = held[i - 1];
        }
    }

    let first_t = times[0];
    let last_t = times[times.len() - 1];

    target_times
        .iter()
        .map(|&tt| {
            // Outside the estimate's span -> unvoiced (see module docs).
            if tt < first_t || tt > last_t {
                return RefF0 { time_s: tt, frequency_hz: 0.0, voiced: false };
            }
            // `lo` is the last source index at or before `tt` (its "left sample").
            // `total_cmp` avoids a panic on a NaN frame time (unreachable for our
            // sample-index-derived times, but the typed comparison is free).
            let lo = match times.binary_search_by(|t| t.total_cmp(&tt)) {
                Ok(i) => i,
                Err(i) => i - 1, // i>=1 here because tt >= first_t
            };

            // Zero-order (left-sample) voicing and frequency mask, exactly as
            // mir_eval takes them; both derive from the same left sample, so a
            // voiced result always carries a real (masked-in) pitch.
            let out_voiced = voiced[lo];
            if !out_voiced {
                return RefF0 { time_s: tt, frequency_hz: 0.0, voiced: false };
            }

            // Linear interpolation of the held cents between `lo` and `lo+1`.
            let interp_cent = if times[lo] == tt || lo + 1 >= times.len() {
                held[lo]
            } else {
                let (t0, t1) = (times[lo], times[lo + 1]);
                let frac = (tt - t0) / (t1 - t0);
                held[lo] + frac * (held[lo + 1] - held[lo])
            };

            RefF0 { time_s: tt, frequency_hz: cents_to_hz(interp_cent), voiced: true }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(t: f64, hz: f32) -> RefF0 {
        RefF0 { time_s: t, frequency_hz: hz, voiced: true }
    }
    fn u(t: f64) -> RefF0 {
        RefF0 { time_s: t, frequency_hz: 0.0, voiced: false }
    }

    #[test]
    fn coincident_grid_is_a_passthrough() {
        // Same times -> each frame comes back essentially unchanged.
        let source = vec![v(0.0, 440.0), u(0.01), v(0.02, 220.0)];
        let out = resample_f0_onto(&[0.0, 0.01, 0.02], &source);
        assert_eq!(out.len(), 3);
        assert!(out[0].voiced && (out[0].frequency_hz - 440.0).abs() < 0.05);
        assert!(!out[1].voiced && out[1].frequency_hz == 0.0);
        assert!(out[2].voiced && (out[2].frequency_hz - 220.0).abs() < 0.05);
    }

    #[test]
    fn prefix_estimate_pads_tail_unvoiced() {
        // Estimate covers [0, 0.01]; reference asks up to 0.03 -> tail unvoiced.
        let source = vec![v(0.0, 440.0), v(0.01, 440.0)];
        let out = resample_f0_onto(&[0.0, 0.01, 0.02, 0.03], &source);
        assert!(out[0].voiced && out[1].voiced);
        assert!(!out[2].voiced && !out[3].voiced);
    }

    #[test]
    fn interpolates_between_frames_in_cent_space() {
        // Midpoint in time between 220 and 440 Hz -> geometric mean (one octave
        // is linear in cents), i.e. ~311 Hz, not the arithmetic mean 330.
        let source = vec![v(0.0, 220.0), v(0.02, 440.0)];
        let out = resample_f0_onto(&[0.01], &source);
        assert!(out[0].voiced);
        let expected = 220.0 * 2f32.powf(0.5); // +600 cents
        assert!((out[0].frequency_hz - expected).abs() < 0.5);
    }

    #[test]
    fn empty_source_is_all_unvoiced() {
        let out = resample_f0_onto(&[0.0, 0.01], &[]);
        assert!(out.iter().all(|f| !f.voiced && f.frequency_hz == 0.0));
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn before_first_frame_is_unvoiced() {
        let source = vec![v(0.01, 440.0)];
        let out = resample_f0_onto(&[0.0, 0.01], &source);
        assert!(!out[0].voiced); // 0.0 is before the estimate starts
        assert!(out[1].voiced);
    }
}
