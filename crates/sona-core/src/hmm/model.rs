//! **Note-HMM model** — states, emission, and transitions (ticket 05, [ADR-0010]).
//!
//! [ADR-0010]: ../../../docs/adr/0010-note-hmm-segmentation.md
//!
//! This is pure, side-effect-free *construction*: it builds the lattice the
//! Viterbi decoder (ticket 06) runs over. Nothing here decodes — the model only
//! knows how to score one frame against a state ([`HmmModel::emission_logs`]) and
//! how likely one state is to follow another ([`HmmModel::transition`]).
//!
//! ## States (Tony/Ryynänen)
//!
//! A vocal-range pitch grid at **3 bins per semitone** — sub-semitone bins keep
//! off-tune / non-A440 / expressive intonation from mis-snapping (ADR-0010). Each
//! pitch bin carries three left-to-right **sub-states**:
//!
//! - **Attack** — the note's onset; high pitch variance tolerated (σ≈5 st).
//! - **Stable** — the steady middle; tight variance (σ≈0.9 st).
//! - **Silent** — the note's tail / the rest that follows it, *remembering* the
//!   bin's pitch so the next note's interval is well-defined (see transitions).
//!
//! There is no separate global silence state: a `Silent(p)` sub-state is silence
//! "having last sung pitch `p`", which is what lets the interval prior on the
//! *next* attack know where it is jumping from. During a long rest the decode
//! self-loops in one `Silent(p)`.
//!
//! State index layout is dense and contiguous: `bin * 3 + sub`, so a pitch range
//! of `P` bins is `3P` states.
//!
//! ## Emission
//!
//! Per frame the model splits probability between the **voiced** sub-states
//! (Attack/Stable) and the **Silent** sub-states with a voicing prior `v≈0.5`,
//! driven by the pYIN voicing the pre-filtered frame carries (ADR-0010). A voiced
//! sub-state additionally scores a **Gaussian on the semitone deviation** between
//! the frame and the bin centre, its influence tempered by a trust exponent
//! `τ≈0.1` so the transition prior — not a single noisy frame — decides
//! boundaries. Std-dev is sub-state–dependent (Attack wide, Stable tight), so a
//! note's beginning tolerates more pitch wobble than its middle.
//!
//! ## Transitions
//!
//! Left-to-right within a note with the literature self-probabilities
//! (`Attack 0.9 / Stable 0.99 / Silent 0.9999`, ADR-0010); the Stable self-prob
//! is Phase B's one main knob (ticket 08). A note changes pitch through an
//! **interval prior** — `Stable(p)`/`Silent(p) → Attack(q)` weighted by
//! `exp(-|p−q|/λ)`, so small steps are cheap and octave jumps expensive. That
//! penalty is what rejects octave artefacts at the source (replacing the UI-side
//! slope filter + MAD trim, ADR-0010). A `Stable(p)` reaches its own `Silent(p)`
//! for a rest; a legato pitch change skips silence and goes straight
//! `Stable(p) → Attack(q)`.

/// Sub-states of a pitch bin, in left-to-right order. Discriminants are the
/// in-bin offset used by [`HmmModel::state`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sub {
    /// Note onset — wide pitch variance tolerated.
    Attack = 0,
    /// Steady middle — tight pitch variance.
    Stable = 1,
    /// The note's tail / the following rest, remembering the bin's pitch.
    Silent = 2,
}

/// Number of sub-states per pitch bin.
pub const SUBS: usize = 3;

/// Tunable parameters of the note-HMM. [`Default`] is the ADR-0010 literature
/// starting point; ticket 08's sweep sets the shipped values (chiefly
/// [`stable_self`](HmmParams::stable_self) and [`interval_lambda`](HmmParams::interval_lambda)).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HmmParams {
    /// Pitch bins per semitone (ADR-0010: 3). The decode grid resolution.
    pub bins_per_semitone: usize,
    /// Emission Gaussian std-dev (semitones) in the **Attack** sub-state — wide,
    /// so an onset's pitch may still be settling.
    pub sigma_attack: f32,
    /// Emission Gaussian std-dev (semitones) in the **Stable** sub-state — tight.
    pub sigma_stable: f32,
    /// Trust exponent `τ` scaling the pitch Gaussian's log-likelihood. Lower
    /// trusts the contour less (flatter emission), letting the transition prior
    /// dominate boundary placement. ADR-0010's nominal `τ≈0.1` is a *floor* — at
    /// that value the decode won't even follow a clean legato pitch change, so the
    /// default trusts the (already median-smoothed) contour more; the sweep can
    /// lower it if real stems over-segment.
    pub pitch_trust: f32,
    /// Voicing prior `v`: the base share of emission mass given to voiced
    /// sub-states vs. Silent, before the frame's own pYIN voicing tilts it.
    pub voicing_prior: f32,
    /// Attack self-transition probability. ADR-0010's nominal is `0.9`, but on a
    /// clean, on-pitch contour the Attack and Stable emissions coincide, so a high
    /// value leaves the decode stuck in Attack for tens of frames and blurs the
    /// Attack→Stable boundary the segmenter and energy splitter read. The default
    /// keeps Attack short (a few frames of onset-variance tolerance), which is its
    /// actual job.
    pub attack_self: f32,
    /// **Stable self-transition probability — Phase B's main knob.** Higher merges
    /// (kills vibrato jitter, risks merging fast notes); lower admits more note
    /// changes. Tuned against the harness, not by eye (ADR-0010).
    pub stable_self: f32,
    /// Silent self-transition probability (how sticky a rest is).
    pub silent_self: f32,
    /// Of the mass a `Stable` state sheds when it leaves, the fraction that goes
    /// to its own `Silent` (a rest) rather than straight to another note's Attack
    /// (a legato change).
    pub stable_to_silent_frac: f32,
    /// Interval-prior decay `λ` (semitones): note-change weight is
    /// `exp(-|Δsemitone|/λ)`. Smaller makes large jumps costlier.
    pub interval_lambda: f32,
    /// Log-domain bonus added to every **Attack** state's emission at a frame
    /// where the energy-onset detector fired (ADR-0006). This is Tony's RMS-rise
    /// splitter: a detected onset forces the decode into an Attack, so two notes
    /// re-articulated at one pitch (which pitch alone cannot separate) split.
    /// Large, because a same-pitch re-attack must outweigh a cheap Stable
    /// self-loop; it only ever applies at the sparse detected-onset frames.
    pub onset_attack_bonus: f32,
}

impl Default for HmmParams {
    fn default() -> Self {
        Self {
            bins_per_semitone: 3,
            sigma_attack: 5.0,
            sigma_stable: 0.9,
            pitch_trust: 1.0,
            voicing_prior: 0.5,
            attack_self: 0.6,
            stable_self: 0.99,
            silent_self: 0.9999,
            stable_to_silent_frac: 0.5,
            interval_lambda: 2.5,
            onset_attack_bonus: 12.0,
        }
    }
}

/// One analysis frame as the emission model reads it: a semitone (voiced) or
/// `None` (no pitch), plus the pYIN voicing probability. This mirrors the
/// pre-filter's `PitchFrame` (ticket 04) without coupling the model to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObsFrame {
    /// Continuous MIDI semitone, or `None` when the frame has no pitch.
    pub semitone: Option<f32>,
    /// Voiced/unvoiced probability in `0.0..=1.0`.
    pub voicing: f32,
}

/// Smallest probability the log terms clamp to, so `ln` never sees `0` and a
/// dead dimension stays finitely (not infinitely) penalised.
const PROB_FLOOR: f32 = 1e-4;

/// Precomputed log-transition constants for the Viterbi decoder — the structural
/// pieces of [`HmmModel::transition`], pulled out once so the decode is
/// `O(frames · bins²)` without re-deriving them per state pair.
pub(crate) struct TransConsts {
    pub log_attack_self: f32,
    pub log_attack_to_stable: f32,
    pub log_stable_self: f32,
    pub log_stable_to_silent: f32,
    pub log_stable_change_base: f32,
    pub log_silent_self: f32,
    pub log_silent_change_base: f32,
    pub bin_decay: f32,
    /// Per-source-bin `ln Z(p)` note-change normaliser.
    pub log_z: Vec<f32>,
}

/// A constructed note-HMM over a fixed integer-MIDI pitch range. Built once per
/// decode (the range is taken from the stem's observed pitch span, ticket 06);
/// holds only immutable structure, so it is cheap to query per frame.
#[derive(Debug, Clone)]
pub struct HmmModel {
    params: HmmParams,
    /// Lowest bin's centre pitch, in MIDI semitones (integer-valued `f32`).
    min_midi: f32,
    /// Number of pitch bins.
    n_bins: usize,
    // Precomputed log-transition constants (all in nats).
    log_attack_self: f32,
    log_attack_to_stable: f32,
    log_stable_self: f32,
    log_stable_to_silent: f32,
    /// `ln(stable_leave · (1−to_silent_frac) / Z)` — the pitch-independent part of
    /// a `Stable(p) → Attack(q)` transition; add the interval log-weight for `q`.
    log_stable_change_base: f32,
    log_silent_self: f32,
    /// `ln(silent_leave / Z)` — the pitch-independent part of `Silent(p) → Attack(q)`.
    log_silent_change_base: f32,
    /// Per-bin normaliser exponent: interval weight is `exp(-|Δbin|·bin_decay)`.
    bin_decay: f32,
}

impl HmmModel {
    /// Build the model over the inclusive integer-MIDI range `[min_midi, max_midi]`.
    /// Panics only on an empty range (`max < min`); callers derive a valid,
    /// padded range from the contour.
    pub fn new(params: HmmParams, min_midi: u8, max_midi: u8) -> Self {
        assert!(max_midi >= min_midi, "empty pitch range");
        let bps = params.bins_per_semitone.max(1);
        let n_semitones = (max_midi - min_midi) as usize + 1;
        let n_bins = n_semitones * bps;

        // Interval normaliser Z: a single decay length shared by all sources; the
        // per-source Z(p) is folded into the transition at query time via
        // `interval_z`. Here we bake the pitch-independent leave-mass constants.
        let bin_decay = 1.0 / (bps as f32 * params.interval_lambda.max(f32::EPSILON));

        let stable_leave = (1.0 - params.stable_self).max(0.0);
        let silent_leave = (1.0 - params.silent_self).max(0.0);
        let to_silent = stable_leave * params.stable_to_silent_frac;
        let stable_change = stable_leave * (1.0 - params.stable_to_silent_frac);

        HmmModel {
            log_attack_self: params.attack_self.max(PROB_FLOOR).ln(),
            log_attack_to_stable: (1.0 - params.attack_self).max(PROB_FLOOR).ln(),
            log_stable_self: params.stable_self.max(PROB_FLOOR).ln(),
            log_stable_to_silent: to_silent.max(PROB_FLOOR).ln(),
            log_stable_change_base: stable_change.max(PROB_FLOOR).ln(),
            log_silent_self: params.silent_self.max(PROB_FLOOR).ln(),
            log_silent_change_base: silent_leave.max(PROB_FLOOR).ln(),
            bin_decay,
            params,
            min_midi: min_midi as f32,
            n_bins,
        }
    }

    /// The model's parameters.
    pub fn params(&self) -> &HmmParams {
        &self.params
    }

    /// Number of pitch bins.
    pub fn n_bins(&self) -> usize {
        self.n_bins
    }

    /// Total number of states (`3 · n_bins`).
    pub fn n_states(&self) -> usize {
        self.n_bins * SUBS
    }

    /// Dense state index for `(bin, sub)`.
    pub fn state(&self, bin: usize, sub: Sub) -> usize {
        bin * SUBS + sub as usize
    }

    /// Inverse of [`state`](Self::state): `(bin, sub)` for a state index.
    pub fn split(&self, state: usize) -> (usize, Sub) {
        let sub = match state % SUBS {
            0 => Sub::Attack,
            1 => Sub::Stable,
            _ => Sub::Silent,
        };
        (state / SUBS, sub)
    }

    /// Centre pitch of `bin`, in MIDI semitones.
    pub fn bin_center(&self, bin: usize) -> f32 {
        self.min_midi + bin as f32 / self.params.bins_per_semitone as f32
    }

    /// Interval log-weight for a note change between bins `from` and `to`:
    /// `-|Δbin|·bin_decay` (i.e. `ln exp(-|Δsemitone|/λ)`), before the per-source
    /// normaliser. Monotonically decreasing in `|from − to|`.
    pub fn interval_log_weight(&self, from: usize, to: usize) -> f32 {
        let d = (from as f32 - to as f32).abs();
        -d * self.bin_decay
    }

    /// Per-source interval normaliser `ln Z(p) = ln Σ_q exp(-|p−q|·bin_decay)`, so
    /// a source bin's outgoing note-change mass sums correctly regardless of how
    /// many bins lie within reach (edge bins have fewer neighbours).
    fn interval_log_z(&self, from: usize) -> f32 {
        let z: f32 = (0..self.n_bins)
            .map(|q| (-((from as f32 - q as f32).abs()) * self.bin_decay).exp())
            .sum();
        z.max(PROB_FLOOR).ln()
    }

    /// The precomputed log-transition constants and per-bin note-change
    /// normalisers the Viterbi decoder (ticket 06) reads. Bundled so the decoder
    /// can exploit the transition structure directly instead of calling
    /// [`transition`](Self::transition) `states²` times.
    pub(crate) fn trans_consts(&self) -> TransConsts {
        TransConsts {
            log_attack_self: self.log_attack_self,
            log_attack_to_stable: self.log_attack_to_stable,
            log_stable_self: self.log_stable_self,
            log_stable_to_silent: self.log_stable_to_silent,
            log_stable_change_base: self.log_stable_change_base,
            log_silent_self: self.log_silent_self,
            log_silent_change_base: self.log_silent_change_base,
            bin_decay: self.bin_decay,
            log_z: (0..self.n_bins).map(|p| self.interval_log_z(p)).collect(),
        }
    }

    /// Log-probability of transitioning from state `i` to state `j`. Returns
    /// [`f32::NEG_INFINITY`] for a transition the left-to-right structure forbids.
    /// This is the readable reference used by the unit tests; the Viterbi decoder
    /// (ticket 06) exploits the same structure directly for speed.
    pub fn transition(&self, i: usize, j: usize) -> f32 {
        let (bi, si) = self.split(i);
        let (bj, sj) = self.split(j);
        match (si, sj) {
            // Within a note, left-to-right.
            (Sub::Attack, Sub::Attack) if bi == bj => self.log_attack_self,
            (Sub::Attack, Sub::Stable) if bi == bj => self.log_attack_to_stable,
            (Sub::Stable, Sub::Stable) if bi == bj => self.log_stable_self,
            (Sub::Stable, Sub::Silent) if bi == bj => self.log_stable_to_silent,
            (Sub::Silent, Sub::Silent) if bi == bj => self.log_silent_self,
            // Note change: Stable/Silent of one pitch → Attack of another (or the
            // same, a re-articulation), shaped by the interval prior.
            (Sub::Stable, Sub::Attack) => {
                self.log_stable_change_base + self.interval_log_weight(bi, bj)
                    - self.interval_log_z(bi)
            }
            (Sub::Silent, Sub::Attack) => {
                self.log_silent_change_base + self.interval_log_weight(bi, bj)
                    - self.interval_log_z(bi)
            }
            _ => f32::NEG_INFINITY,
        }
    }

    /// Log-emission of every state for one observed frame, indexed by state.
    ///
    /// Voiced sub-states score `ln v_prior + ln voicing + τ·gaussian`; Silent
    /// sub-states score `ln(1−v_prior) + ln(1−voicing)`. Terms are clamped away
    /// from `0`/`-∞` by [`PROB_FLOOR`].
    pub fn emission_logs(&self, frame: ObsFrame) -> Vec<f32> {
        let v = frame.voicing.clamp(PROB_FLOOR, 1.0 - PROB_FLOOR);
        let ln_voiced_mix = self.params.voicing_prior.max(PROB_FLOOR).ln() + v.ln();
        let ln_silent_mix =
            (1.0 - self.params.voicing_prior).max(PROB_FLOOR).ln() + (1.0 - v).ln();

        let mut out = vec![0.0f32; self.n_states()];
        for bin in 0..self.n_bins {
            let center = self.bin_center(bin);
            let attack = ln_voiced_mix + self.pitch_log(frame.semitone, center, self.params.sigma_attack);
            let stable = ln_voiced_mix + self.pitch_log(frame.semitone, center, self.params.sigma_stable);
            out[self.state(bin, Sub::Attack)] = attack;
            out[self.state(bin, Sub::Stable)] = stable;
            out[self.state(bin, Sub::Silent)] = ln_silent_mix;
        }
        out
    }

    /// The trust-weighted Gaussian pitch log-term for a voiced frame at `semitone`
    /// against a bin `center` with std-dev `sigma`. An unpitched frame (`None`)
    /// contributes `0` — its suppression comes from the voicing mix term, not a
    /// fabricated pitch penalty.
    fn pitch_log(&self, semitone: Option<f32>, center: f32, sigma: f32) -> f32 {
        match semitone {
            Some(s) => {
                let dev = (s - center) / sigma.max(f32::EPSILON);
                self.params.pitch_trust * (-0.5 * dev * dev)
            }
            None => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> HmmModel {
        // A4 (69) ± an octave, enough bins for the interval tests.
        HmmModel::new(HmmParams::default(), 57, 81)
    }

    #[test]
    fn state_indexing_round_trips() {
        let m = model();
        for bin in 0..m.n_bins() {
            for sub in [Sub::Attack, Sub::Stable, Sub::Silent] {
                assert_eq!(m.split(m.state(bin, sub)), (bin, sub));
            }
        }
        assert_eq!(m.n_states(), m.n_bins() * 3);
    }

    #[test]
    fn bins_are_three_per_semitone() {
        let m = model();
        // 57..=81 inclusive is 25 semitones × 3 bins.
        assert_eq!(m.n_bins(), 25 * 3);
        // Bin 0 centres on the low edge; three bins span one semitone.
        assert!((m.bin_center(0) - 57.0).abs() < 1e-6);
        assert!((m.bin_center(3) - 58.0).abs() < 1e-6);
    }

    #[test]
    fn transition_rows_are_normalized() {
        // Every state's outgoing probabilities must sum to 1 (mass normalization).
        let m = model();
        for i in 0..m.n_states() {
            let mass: f32 = (0..m.n_states())
                .map(|j| m.transition(i, j))
                .filter(|x| x.is_finite())
                .map(|x| x.exp())
                .sum();
            assert!(
                (mass - 1.0).abs() < 1e-3,
                "state {i} outgoing mass {mass}, want 1.0"
            );
        }
    }

    #[test]
    fn sigma_ordering_attack_tolerates_more_deviation_than_stable() {
        // At a non-zero pitch deviation the wide-σ Attack must emit at least as
        // much as the tight-σ Stable — the whole point of the two std-devs.
        let m = model();
        let bin = 36; // some interior bin; centre = 57 + 12 = 69 (A4)
        let center = m.bin_center(bin);
        // A frame 2 semitones sharp of the bin centre.
        let frame = ObsFrame { semitone: Some(center + 2.0), voicing: 0.9 };
        let e = m.emission_logs(frame);
        let attack = e[m.state(bin, Sub::Attack)];
        let stable = e[m.state(bin, Sub::Stable)];
        assert!(
            attack > stable,
            "Attack (wide σ) should out-emit Stable at dev=2: {attack} vs {stable}"
        );
        // On-centre the two coincide (both Gaussians peak at 0 deviation).
        let on = m.emission_logs(ObsFrame { semitone: Some(center), voicing: 0.9 });
        assert!((on[m.state(bin, Sub::Attack)] - on[m.state(bin, Sub::Stable)]).abs() < 1e-4);
    }

    #[test]
    fn interval_penalty_is_monotonic_in_distance() {
        // The note-change weight must strictly decrease as the pitch jump grows,
        // making octaves the most expensive move (rejects octave artefacts).
        let m = model();
        let from = m.state(36, Sub::Silent); // Silent at A4
        let semitone_step = m.params().bins_per_semitone; // one semitone in bins
        let mut prev = f32::INFINITY;
        for steps in 1..=12 {
            let to = m.state(36 + steps * semitone_step, Sub::Attack);
            let t = m.transition(from, to);
            assert!(t.is_finite(), "note-change should be reachable");
            assert!(t < prev, "transition must fall as the interval grows");
            prev = t;
        }
        // An octave jump is far cheaper than a semitone step.
        let semitone = m.transition(from, m.state(36 + semitone_step, Sub::Attack));
        let octave = m.transition(from, m.state(36 + 12 * semitone_step, Sub::Attack));
        assert!(octave < semitone - 2.0, "octave ({octave}) ≪ semitone ({semitone})");
    }

    #[test]
    fn voiced_frame_favours_pitch_states_unvoiced_favours_silent() {
        let m = model();
        let bin = 36;
        let center = m.bin_center(bin);

        let voiced = m.emission_logs(ObsFrame { semitone: Some(center), voicing: 0.95 });
        assert!(
            voiced[m.state(bin, Sub::Stable)] > voiced[m.state(bin, Sub::Silent)],
            "a clearly voiced, on-pitch frame prefers the pitch state"
        );

        let unvoiced = m.emission_logs(ObsFrame { semitone: None, voicing: 0.02 });
        assert!(
            unvoiced[m.state(bin, Sub::Silent)] > unvoiced[m.state(bin, Sub::Stable)],
            "an unvoiced frame prefers Silent"
        );
    }

    #[test]
    fn forbidden_transitions_are_neg_infinity() {
        let m = model();
        // Stable → Stable across different pitches is not a legal move (must pass
        // through an Attack); Attack of one pitch → Attack of another likewise.
        assert_eq!(
            m.transition(m.state(10, Sub::Stable), m.state(11, Sub::Stable)),
            f32::NEG_INFINITY
        );
        assert_eq!(
            m.transition(m.state(10, Sub::Attack), m.state(11, Sub::Attack)),
            f32::NEG_INFINITY
        );
        // Silent → Stable (skipping the Attack) is illegal too.
        assert_eq!(
            m.transition(m.state(10, Sub::Silent), m.state(10, Sub::Stable)),
            f32::NEG_INFINITY
        );
    }
}
