//! **Viterbi decode + energy same-pitch split** (ticket 06, [ADR-0010]).
//!
//! [ADR-0010]: ../../../docs/adr/0010-note-hmm-segmentation.md
//!
//! Standard log-domain Viterbi over the note-HMM lattice ([`super::model`]):
//! `score[t][j] = emit[t][j] + maxᵢ(score[t−1][i] + trans[i][j])`, with
//! backpointers and a backtrack. The naïve form is `O(frames · states²)`; here
//! the transition structure is exploited so it is `O(frames · bins²)` — only the
//! note-change (`Stable`/`Silent → Attack`) edges are dense, and every other edge
//! is `O(1)` incoming.
//!
//! ## Energy same-pitch splitter
//!
//! Pitch alone cannot separate two notes re-articulated at one pitch (the
//! documented over-merge failure of pitch-only methods). The energy-onset
//! detector (ADR-0006) supplies the missing signal: at a frame where an onset
//! fired, every `Attack` state's emission gets a large bonus
//! ([`HmmParams::onset_attack_bonus`](super::model::HmmParams::onset_attack_bonus)),
//! so the decode is driven into an Attack there and the syllable splits. This is
//! the model's only coupling to energy; it enters purely as a per-frame prior.

use super::model::{HmmModel, ObsFrame, Sub};

/// Everything the decoder needs for one stem: the per-frame observations and,
/// aligned to them, whether the energy-onset detector fired at each frame.
#[derive(Debug, Clone)]
pub struct DecodeInput {
    /// Per-frame `(semitone, voicing)` observations, in time order.
    pub obs: Vec<ObsFrame>,
    /// `onset[t] == true` marks a frame where an energy onset was detected. Same
    /// length as [`obs`](Self::obs).
    pub onset: Vec<bool>,
}

/// Penalty (nats) applied to an `Attack`/`Stable` state at `t = 0`: a stem may
/// begin part-way through a note, but starting already voiced is far less likely
/// than starting in silence, and starting mid-note in `Stable` (no attack seen)
/// is disallowed outright.
const START_VOICED_PENALTY: f32 = 6.0;

/// Viterbi-decode the most likely state path over `input`, one state index per
/// frame. Returns an empty path for an empty observation sequence.
///
/// The `onset` flags drive the energy same-pitch splitter; pass an all-`false`
/// slice (or via [`DecodeInput`]) to decode on pitch alone.
pub fn viterbi_path(model: &HmmModel, input: &DecodeInput) -> Vec<usize> {
    let t_len = input.obs.len();
    if t_len == 0 {
        return Vec::new();
    }
    let n_bins = model.n_bins();
    let n_states = model.n_states();
    let c = model.trans_consts();
    let bonus = model.params().onset_attack_bonus;

    // score[j] for the current / previous frame; back[t][j] = best predecessor.
    let mut prev = emission_with_onset(model, input.obs[0], input.onset[0], bonus);
    apply_start_prior(model, &mut prev);
    let mut back: Vec<Vec<usize>> = vec![vec![usize::MAX; n_states]; t_len];

    // `t` is the frame index into obs / onset / back; a range loop is the clearest
    // form for a Viterbi time sweep that reads several arrays at `t` and `t-1`.
    #[allow(clippy::needless_range_loop)]
    for t in 1..t_len {
        let emit = emission_with_onset(model, input.obs[t], input.onset[t], bonus);
        let mut cur = vec![f32::NEG_INFINITY; n_states];
        let bt = &mut back[t];

        // --- Note-change contribution into every Attack(q) --------------------
        // src(p): the better of leaving Stable(p) or Silent(p) for a new note,
        // net of the per-source normaliser. note_change(q) = maxₚ src(p) − |p−q|·decay.
        let mut src_val = vec![f32::NEG_INFINITY; n_bins];
        let mut src_state = vec![usize::MAX; n_bins];
        for p in 0..n_bins {
            let from_stable = prev[model.state(p, Sub::Stable)] + c.log_stable_change_base;
            let from_silent = prev[model.state(p, Sub::Silent)] + c.log_silent_change_base;
            let (v, s) = if from_stable >= from_silent {
                (from_stable, model.state(p, Sub::Stable))
            } else {
                (from_silent, model.state(p, Sub::Silent))
            };
            src_val[p] = v - c.log_z[p];
            src_state[p] = s;
        }

        for q in 0..n_bins {
            // Best source bin p for a note change into q (dense: O(bins)).
            let mut best_change = f32::NEG_INFINITY;
            let mut best_change_state = usize::MAX;
            for p in 0..n_bins {
                let cand = src_val[p] - (p as f32 - q as f32).abs() * c.bin_decay;
                if cand > best_change {
                    best_change = cand;
                    best_change_state = src_state[p];
                }
            }

            // Attack(q): note-change vs. staying in Attack(q).
            let a = model.state(q, Sub::Attack);
            let self_a = prev[a] + c.log_attack_self;
            let (av, as_bp) = if self_a >= best_change {
                (self_a, a)
            } else {
                (best_change, best_change_state)
            };
            cur[a] = av + emit[a];
            bt[a] = as_bp;

            // Stable(q): from Attack(q) or its own self-loop. O(1).
            let st = model.state(q, Sub::Stable);
            let from_attack = prev[a] + c.log_attack_to_stable;
            let self_st = prev[st] + c.log_stable_self;
            let (sv, st_bp) = if self_st >= from_attack { (self_st, st) } else { (from_attack, a) };
            cur[st] = sv + emit[st];
            bt[st] = st_bp;

            // Silent(q): from Stable(q) (a rest opening) or its own self-loop. O(1).
            let si = model.state(q, Sub::Silent);
            let from_stable = prev[st] + c.log_stable_to_silent;
            let self_si = prev[si] + c.log_silent_self;
            let (siv, si_bp) = if self_si >= from_stable { (self_si, si) } else { (from_stable, st) };
            cur[si] = siv + emit[si];
            bt[si] = si_bp;
        }

        prev = cur;
    }

    backtrack(&prev, &back)
}

/// Emission logs for one frame, with the onset Attack bonus folded in when the
/// energy detector fired at this frame.
fn emission_with_onset(model: &HmmModel, frame: ObsFrame, onset: bool, bonus: f32) -> Vec<f32> {
    let mut emit = model.emission_logs(frame);
    if onset {
        for bin in 0..model.n_bins() {
            emit[model.state(bin, Sub::Attack)] += bonus;
        }
    }
    emit
}

/// Bias the initial frame toward silence: a stem usually starts in a rest.
/// `Silent` states keep their emission; `Attack` states are penalised (a stem
/// *can* open mid-note); `Stable` starts are ruled out (a note must open on an
/// attack, never already in its steady middle).
fn apply_start_prior(model: &HmmModel, score: &mut [f32]) {
    for bin in 0..model.n_bins() {
        score[model.state(bin, Sub::Attack)] -= START_VOICED_PENALTY;
        score[model.state(bin, Sub::Stable)] = f32::NEG_INFINITY;
    }
}

/// Follow the backpointers from the best final state to recover the path.
fn backtrack(final_score: &[f32], back: &[Vec<usize>]) -> Vec<usize> {
    let t_len = back.len();
    let mut state = (0..final_score.len())
        .max_by(|&a, &b| final_score[a].partial_cmp(&final_score[b]).unwrap())
        .expect("non-empty state space");

    let mut path = vec![0usize; t_len];
    for t in (0..t_len).rev() {
        path[t] = state;
        // back[0] is unused (t=0 has no predecessor); stop stepping there.
        if t > 0 {
            state = back[t][state];
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hmm::model::HmmParams;

    /// Build observations from a list of `(semitone_option, voicing)` frames.
    fn obs(frames: &[(Option<f32>, f32)]) -> Vec<ObsFrame> {
        frames
            .iter()
            .map(|&(semitone, voicing)| ObsFrame { semitone, voicing })
            .collect()
    }

    /// Decode with no energy onsets (pitch-only).
    fn decode_pitch_only(model: &HmmModel, o: &[ObsFrame]) -> Vec<(usize, Sub)> {
        let input = DecodeInput {
            obs: o.to_vec(),
            onset: vec![false; o.len()],
        };
        viterbi_path(model, &input)
            .into_iter()
            .map(|s| model.split(s))
            .collect()
    }

    /// The integer MIDI a state's bin rounds to.
    fn state_midi(model: &HmmModel, bin: usize) -> i32 {
        model.bin_center(bin).round() as i32
    }

    #[test]
    fn recovers_a_single_steady_note_between_rests() {
        let model = HmmModel::new(HmmParams::default(), 60, 78);
        // rest, a held A4 (69), rest.
        let mut frames = vec![(None, 0.02); 4];
        frames.extend(vec![(Some(69.0), 0.95); 20]);
        frames.extend(vec![(None, 0.02); 4]);
        let path = decode_pitch_only(&model, &obs(&frames));

        // The middle is a voiced note at A4; the ends are Silent.
        assert!(matches!(path[0].1, Sub::Silent));
        assert!(matches!(path[path.len() - 1].1, Sub::Silent));
        let voiced: Vec<_> = path[4..24]
            .iter()
            .filter(|(_, s)| !matches!(s, Sub::Silent))
            .collect();
        assert!(!voiced.is_empty(), "the note body is voiced");
        for (bin, _) in &voiced {
            assert_eq!(state_midi(&model, *bin), 69, "note sits at A4");
        }
        // A note begins with an Attack.
        assert!(
            path[4..24].iter().any(|(_, s)| matches!(s, Sub::Attack)),
            "the note opens on an attack"
        );
    }

    #[test]
    fn splits_a_legato_pitch_change_into_two_notes() {
        // A4 then C5 under one breath (no rest, no onset): the decode must still
        // pass through an Attack at the change — pitch-only note segmentation.
        let model = HmmModel::new(HmmParams::default(), 60, 78);
        let mut frames = vec![(None, 0.02); 3];
        frames.extend(vec![(Some(69.0), 0.95); 16]); // A4
        frames.extend(vec![(Some(72.0), 0.95); 16]); // C5
        frames.extend(vec![(None, 0.02); 3]);
        let path = decode_pitch_only(&model, &obs(&frames));

        // Two distinct voiced pitches appear, each opened by an Attack.
        let a4_attack = path
            .iter()
            .any(|(b, s)| matches!(s, Sub::Attack) && state_midi(&model, *b) == 69);
        let c5_attack = path
            .iter()
            .any(|(b, s)| matches!(s, Sub::Attack) && state_midi(&model, *b) == 72);
        assert!(a4_attack, "A4 opens with an attack");
        assert!(c5_attack, "C5 opens with an attack (the legato split)");
    }

    #[test]
    fn energy_onset_splits_a_re_articulated_same_pitch_note() {
        // One sustained A4 with no pitch change, but an energy onset partway
        // through (a re-attack). Pitch alone sees one note; the onset must force a
        // second Attack at that frame — Tony's RMS-rise splitter.
        let model = HmmModel::new(HmmParams::default(), 60, 78);
        let frames = obs(&vec![(Some(69.0), 0.95); 40]);
        let mut onset = vec![false; 40];
        onset[20] = true; // re-articulation midway

        let input = DecodeInput { obs: frames, onset };
        let path: Vec<_> = viterbi_path(&model, &input)
            .into_iter()
            .map(|s| model.split(s))
            .collect();

        // An Attack lands at (or right at) the onset frame, opening a second note.
        assert!(
            matches!(path[20].1, Sub::Attack),
            "the energy onset forces an Attack at frame 20: {:?}",
            path[20]
        );
        // Two attack runs total (start of note 1 and the re-articulation).
        let attack_runs = path
            .windows(2)
            .filter(|w| matches!(w[1].1, Sub::Attack) && !matches!(w[0].1, Sub::Attack))
            .count()
            + usize::from(matches!(path[0].1, Sub::Attack));
        assert_eq!(attack_runs, 2, "two note-starts: {path:?}");
    }

    #[test]
    fn interval_prior_rejects_an_isolated_octave_jump() {
        // A steady A4 with a single frame that reads an octave up (A5, 81) — a
        // classic YIN octave artefact. The interval prior must make jumping up and
        // back an octave for one frame far too expensive, so the decode holds A4.
        let model = HmmModel::new(HmmParams::default(), 57, 84);
        let mut frames = vec![(Some(69.0), 0.95); 30];
        frames[15] = (Some(81.0), 0.95); // lone octave spike
        let path = decode_pitch_only(&model, &obs(&frames));

        assert_ne!(
            state_midi(&model, path[15].0),
            81,
            "the lone octave spike must not become an A5 note: {:?}",
            path[15]
        );
        assert_eq!(state_midi(&model, path[15].0), 69, "it stays A4");
    }
}
