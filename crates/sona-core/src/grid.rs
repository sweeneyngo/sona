//! **Grid-conditioned note decoding** ([ADR-0015]): an opt-in segmenter for
//! grid-locked material (the declared target: Vocaloid songs, whose notes start
//! on a 16th or 16th-triplet grid). Instead of placing boundaries frame by frame,
//! it decodes **one state per grid cell** — rest, a new note at pitch *p*, or the
//! continuation of the note at *p* — by Viterbi over cell-level evidence.
//!
//! [ADR-0015]: ../../../docs/adr/0015-grid-conditioned-cell-decoding.md
//!
//! Output is the same [`NoteSegment`] contract as the note-HMM, so confidence,
//! the annotation JSON, and the CLI are unchanged.

use crate::annotation::{EnergyFrame, F0Frame};
use crate::onset::Onset;
use crate::pitch::PitchCandidate;
use crate::segment::{drop_steepness, hz_to_midi, NoteSegment, OffsetInfo, OffsetKind};

/// The grid a stem is decoded on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    /// Tempo, beats per minute (a beat is a quarter note).
    pub bpm: f64,
    /// Cells per whole note: `16` for a 16th grid, `24` for 16th-triplets.
    pub division: u32,
    /// Grid phase: lines at `anchor_s + k·spacing`. `None` estimates it.
    pub anchor_s: Option<f64>,
}

impl GridSpec {
    /// Seconds per cell (matches the review surface's `gridSpacingSeconds`).
    pub fn spacing_s(&self) -> f64 {
        (60.0 / self.bpm) * 4.0 / self.division as f64
    }
}

/// Grid mode for the [`Transcriber`](crate::Transcriber): decode on this grid
/// (at the annotation's BPM) instead of with the frame-level note-HMM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridMode {
    /// Cells per whole note: `16` (16ths) or `24` (16th-triplets).
    pub division: u32,
    /// Known grid phase in seconds, or `None` to estimate it from the stem.
    pub anchor_s: Option<f64>,
    /// The cell decoder.
    pub decoder: GridDecoder,
}

impl GridMode {
    /// A 16th grid with an estimated phase and the default decoder.
    pub fn sixteenths() -> Self {
        GridMode { division: 16, anchor_s: None, decoder: GridDecoder::default() }
    }
}

/// The cell decoder's parameters. Defaults are the dev-split optimum (ADR-0015).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridDecoder {
    /// Std-dev (semitones) of the per-frame pitch Gaussian.
    pub sigma: f32,
    /// Cap on one frame's pitch cost (in units of the Gaussian's exponent), so a
    /// glide or an octave blip can't outvote the rest of the cell.
    pub cost_cap: f32,
    /// Cost of an unvoiced frame inside a note (a consonant or a pYIN dropout).
    pub unvoiced_in_note: f32,
    /// Cost of a voiced frame inside a rest, scaled by voicing and loudness.
    pub voiced_in_rest: f32,
    /// Exponent on the frame's voicing probability in the rest cost.
    pub voicing_pow: f32,
    /// Loudness (fraction of the stem's peak RMS) at which a voiced frame pays
    /// the full rest cost; quieter frames pay proportionally less.
    pub loud_frac: f32,
    /// Rest → note.
    pub onset_cost: f32,
    /// Note → rest.
    pub offset_cost: f32,
    /// Same-pitch re-articulation, before the cue discount.
    pub rearticulate_cost: f32,
    /// How much a full-strength re-articulation cue discounts that cost.
    pub cue_weight: f32,
    /// Base cost of a pitch change.
    pub change_cost: f32,
    /// Extra pitch-change cost per semitone of interval.
    pub interval_cost: f32,
    /// Remove the stem's global tuning offset before decoding.
    pub estimate_tuning: bool,
    /// Rests of at most this many cells between two notes are filled by
    /// extending the earlier note (`0` disables).
    pub fill_gap_cells: u32,
    /// Candidate phases tried across one cell when the anchor is unknown.
    pub phase_candidates: usize,
    /// Half-window (seconds) for the offset energy-drop steepness (confidence).
    pub drop_window_s: f64,
    /// Weigh per-frame **pitch candidates** (pitches with saliences, from
    /// [`HarmonicSalience`](crate::pitch::HarmonicSalience)) alongside the
    /// contour. Where two voices sing, pYIN often unvoices the frame or picks
    /// the other voice; harmonic-sum salience still finds the lead's f0, and its
    /// strength argues against a rest. `None` (the default) uses the contour
    /// alone. [`Transcriber`](crate::Transcriber) computes the candidates when
    /// this is set; [`segment_with_candidates`](Self::segment_with_candidates)
    /// takes them directly.
    pub candidates: Option<CandidateEvidence>,
}

/// How the cell emission weighs pitch candidates (see [`GridDecoder::candidates`]).
///
/// Each frame's candidates are normalised two ways: by the frame's strongest
/// (`r`, relative salience in `0..=1`) and by the stem's
/// `strength_percentile`-th strongest frame (`v`, frame strength in `0..=1`).
/// A note at pitch *p* then pays, per frame, `weight · v · min(cap, min_k
/// (gauss(m_k − p) + rank_cost · (1 − r_k)))`, and a rest pays
/// `rest_cost · v^rest_pow`; the contour's own terms are scaled by
/// `contour_weight`.
///
/// Defaults are the dev-split optimum (vocal and lead-only stems). A high
/// `rank_cost` means only near-ties to the strongest candidate are followed:
/// on dev, letting the continuity prior pick weaker candidates scored lower.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidateEvidence {
    /// Scale of the candidate pitch term, relative to the contour's.
    pub weight: f32,
    /// Extra cost of following a weaker candidate, at relative salience `0`.
    pub rank_cost: f32,
    /// Cost per frame of strength `1` decoded as a rest.
    pub rest_cost: f32,
    /// Exponent on frame strength in the rest cost: above `1`, weak frames
    /// (reverb tails, bleed) barely argue against a rest.
    pub rest_pow: f32,
    /// The stem's frame-strength reference, as a percentile of frames with a
    /// candidate (`0.9` = the 90th-percentile frame has strength `1`).
    pub strength_percentile: f32,
    /// Scale on the contour's own terms (`1` keeps them; `0` decodes from the
    /// candidates alone, the contour still setting tuning and range).
    pub contour_weight: f32,
    /// Delay (seconds) applied to the candidates before weighing them.
    /// Salience rises as soon as a note enters the analysis window, pYIN's
    /// periodicity only once the note fills much of it, so on the same frame
    /// clock the candidates lead the contour.
    pub lag_s: f64,
}

impl Default for CandidateEvidence {
    fn default() -> Self {
        Self { weight: 1.0, rank_cost: 8.0, rest_cost: 2.0, rest_pow: 2.0, strength_percentile: 0.9, contour_weight: 0.5, lag_s: 0.012 }
    }
}

impl Default for GridDecoder {
    fn default() -> Self {
        Self {
            sigma: 0.35,
            cost_cap: 2.0,
            unvoiced_in_note: 0.3,
            voiced_in_rest: 4.0,
            voicing_pow: 0.5,
            loud_frac: 0.4,
            onset_cost: 1.0,
            offset_cost: 1.0,
            rearticulate_cost: 8.0,
            cue_weight: 10.0,
            change_cost: 8.0,
            interval_cost: 1.0,
            estimate_tuning: true,
            fill_gap_cells: 1,
            phase_candidates: 24,
            drop_window_s: 0.04,
            candidates: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct CellNote {
    onset_s: f64,
    offset_s: f64,
    midi: i32,
}

struct Frames {
    time: Vec<f64>,
    semi: Vec<Option<f32>>,
    voicing: Vec<f32>,
    loud: Vec<f32>,
    /// Per frame × pitch, the candidate term of a note's cost (empty if none).
    cand_note: Vec<f32>,
    /// Per frame, the candidate term of a rest's cost (empty if none).
    cand_rest: Vec<f32>,
    env_time: Vec<f64>,
    env_rms: Vec<f32>,
    lo: i32,
    hi: i32,
}

impl GridDecoder {
    /// Decode a stem into grid-aligned note segments. Empty when the contour has
    /// no voiced frame or the grid is degenerate.
    pub fn segment(&self, grid: &GridSpec, onsets: &[Onset], contour: &[F0Frame], envelope: &[EnergyFrame]) -> Vec<NoteSegment> {
        self.segment_with_candidates(grid, onsets, contour, &[], envelope)
    }

    /// [`segment`](Self::segment) with per-frame pitch candidates aligned to
    /// `contour` (frame `i` of each). They are weighed only when
    /// [`candidates`](Self::candidates) is set; missing frames count as none.
    pub fn segment_with_candidates(
        &self,
        grid: &GridSpec,
        onsets: &[Onset],
        contour: &[F0Frame],
        candidates: &[Vec<PitchCandidate>],
        envelope: &[EnergyFrame],
    ) -> Vec<NoteSegment> {
        let sp = grid.spacing_s();
        if !(sp.is_finite() && sp > 0.0) {
            return Vec::new();
        }
        let Some(fr) = self.prepare(contour, candidates, envelope) else {
            return Vec::new();
        };
        let notes = match grid.anchor_s {
            Some(a) => self.decode(&fr, sp, a).0,
            // The phase comes from the contour alone even when candidates are
            // weighed: salience onsets lead pYIN's by part of a window, and on
            // dev the candidate-weighted cost picked a wrong phase on one song.
            None if !fr.cand_rest.is_empty() => match self.estimate_phase(grid, contour, envelope) {
                Some(a) => self.decode(&fr, sp, a).0,
                None => Vec::new(),
            },
            None => self.best_phase(&fr, sp),
        };
        let notes = self.fill_gaps(notes, sp);
        self.segments_of(&notes, &fr, onsets, envelope)
    }

    /// Estimated grid phase (seconds within one cell) for a stem.
    pub fn estimate_phase(&self, grid: &GridSpec, contour: &[F0Frame], envelope: &[EnergyFrame]) -> Option<f64> {
        let sp = grid.spacing_s();
        let fr = self.prepare(contour, &[], envelope)?;
        let n = self.phase_candidates.max(1);
        (0..n)
            .map(|i| {
                let a = sp * i as f64 / n as f64;
                (self.decode(&fr, sp, a).1, a)
            })
            .min_by(|x, y| x.0.total_cmp(&y.0))
            .map(|(_, a)| a)
    }

    fn best_phase(&self, fr: &Frames, sp: f64) -> Vec<CellNote> {
        let n = self.phase_candidates.max(1);
        let mut best: Option<(f64, Vec<CellNote>)> = None;
        for i in 0..n {
            let (notes, cost) = self.decode(fr, sp, sp * i as f64 / n as f64);
            if best.as_ref().is_none_or(|b| cost < b.0) {
                best = Some((cost, notes));
            }
        }
        best.map(|b| b.1).unwrap_or_default()
    }

    fn prepare(&self, contour: &[F0Frame], candidates: &[Vec<PitchCandidate>], envelope: &[EnergyFrame]) -> Option<Frames> {
        let raw: Vec<Option<f32>> =
            contour.iter().map(|f| (f.frequency_hz > 0.0).then(|| hz_to_midi(f.frequency_hz))).collect();
        let voiced: Vec<f32> = raw.iter().flatten().copied().collect();
        if voiced.is_empty() {
            return None;
        }
        let tune = if self.estimate_tuning { tuning_offset(&voiced) } else { 0.0 };
        let semi: Vec<Option<f32>> = raw.iter().map(|s| s.map(|v| v - tune)).collect();
        let mut sorted: Vec<f32> = voiced.iter().map(|v| v - tune).collect();
        let candidates: Vec<&[PitchCandidate]> = match self.candidates {
            Some(ev) if !candidates.is_empty() => {
                let hop = match (contour.first(), contour.last()) {
                    (Some(a), Some(b)) if contour.len() > 1 => (b.time_s - a.time_s) / (contour.len() - 1) as f64,
                    _ => 1.0,
                };
                let shift = if hop > 0.0 { (ev.lag_s / hop).round().max(0.0) as usize } else { 0 };
                (0..contour.len()).map(|i| i.checked_sub(shift).and_then(|j| candidates.get(j)).map_or(&[][..], |c| &c[..])).collect()
            }
            _ => Vec::new(),
        };
        if self.candidates.is_some() {
            // The pitch range must also reach the pitches only the candidates
            // see: under a harmony, pYIN may never voice the lead's register.
            let tops = candidates.iter().filter_map(|c| c.iter().max_by(|a, b| a.salience.total_cmp(&b.salience)));
            sorted.extend(tops.filter(|k| k.frequency_hz > 0.0 && k.salience > 0.0).map(|k| hz_to_midi(k.frequency_hz) - tune));
        }
        sorted.sort_by(f32::total_cmp);
        let pct = |p: f64| sorted[((sorted.len() - 1) as f64 * p).round() as usize];
        let lo = pct(0.005).floor() as i32 - 1;
        let hi = pct(0.995).ceil() as i32 + 1;

        let env_time: Vec<f64> = envelope.iter().map(|e| e.time_s).collect();
        let env_rms: Vec<f32> = envelope.iter().map(|e| e.rms).collect();
        let peak = env_rms.iter().copied().fold(0.0f32, f32::max).max(f32::MIN_POSITIVE);
        let loud = contour
            .iter()
            .map(|f| {
                let k = env_time.partition_point(|&t| t < f.time_s).min(env_rms.len().saturating_sub(1));
                env_rms.get(k).copied().unwrap_or(0.0) / peak
            })
            .collect();
        let (cand_note, cand_rest) = match self.candidates {
            Some(ev) => self.candidate_terms(&ev, &candidates, tune, lo, hi),
            None => (Vec::new(), Vec::new()),
        };
        Some(Frames {
            time: contour.iter().map(|f| f.time_s).collect(),
            semi,
            voicing: contour.iter().map(|f| f.voicing).collect(),
            loud,
            cand_note,
            cand_rest,
            env_time,
            env_rms,
            lo,
            hi,
        })
    }

    /// The candidate terms of the emission, per frame (see [`CandidateEvidence`]).
    fn candidate_terms(&self, ev: &CandidateEvidence, cands: &[&[PitchCandidate]], tune: f32, lo: i32, hi: i32) -> (Vec<f32>, Vec<f32>) {
        let n = cands.len();
        let top_of = |c: &[PitchCandidate]| c.iter().map(|k| k.salience).reduce(f32::max).filter(|&s| s > 0.0);
        let mut tops: Vec<f32> = cands.iter().filter_map(|c| top_of(c)).collect();
        if tops.is_empty() {
            return (Vec::new(), Vec::new());
        }
        tops.sort_by(f32::total_cmp);
        let pct = ev.strength_percentile.clamp(0.0, 1.0) as f64;
        let reference = tops[((tops.len() - 1) as f64 * pct).round() as usize];
        let np = (hi - lo + 1) as usize;
        let inv2s2 = 1.0 / (2.0 * self.sigma * self.sigma);
        let mut note = vec![0.0f32; n * np];
        let mut rest = vec![0.0f32; n];
        for (i, c) in cands.iter().enumerate() {
            let Some(top) = top_of(c) else { continue };
            let v = (top / reference).min(1.0);
            rest[i] = ev.rest_cost * v.powf(ev.rest_pow);
            // (tuned semitone, rank penalty) per candidate
            let ks: Vec<(f32, f32)> = c
                .iter()
                .filter(|k| k.frequency_hz > 0.0)
                .map(|k| (hz_to_midi(k.frequency_hz) - tune, ev.rank_cost * (1.0 - k.salience / top)))
                .collect();
            for (p, slot) in note[i * np..(i + 1) * np].iter_mut().enumerate() {
                let pitch = (lo + p as i32) as f32;
                let best = ks.iter().map(|&(m, r)| ((m - pitch) * (m - pitch) * inv2s2).min(self.cost_cap) + r).fold(self.cost_cap, f32::min);
                *slot = ev.weight * v * best;
            }
        }
        (note, rest)
    }

    fn decode(&self, fr: &Frames, sp: f64, anchor: f64) -> (Vec<CellNote>, f64) {
        let last = *fr.time.last().expect("prepared frames are non-empty");
        let k0 = ((0.0 - anchor) / sp).ceil() as i64;
        let k1 = ((last - anchor) / sp).floor() as i64;
        if k1 <= k0 {
            return (Vec::new(), f64::INFINITY);
        }
        let starts: Vec<f64> = (k0..k1).map(|k| anchor + k as f64 * sp).collect();
        let nc = starts.len();
        let np = (fr.hi - fr.lo + 1) as usize;
        let pitch = |i: usize| (fr.lo + i as i32) as f32;
        let at = |t: f64| fr.time.partition_point(|&x| x < t);
        let env_at = |t: f64| fr.env_time.partition_point(|&x| x < t);

        let mut em_note = vec![0.0f32; nc * np];
        let mut em_rest = vec![0.0f32; nc];
        let mut cue = vec![0.0f32; nc];
        let inv2s2 = 1.0 / (2.0 * self.sigma * self.sigma);
        let has_cands = !fr.cand_rest.is_empty();
        let cw = self.candidates.map_or(1.0, |e| e.contour_weight);
        for (c, &a) in starts.iter().enumerate() {
            let (i0, i1) = (at(a), at(a + sp));
            let mut unvoiced = 0usize;
            for i in i0..i1 {
                match fr.semi[i] {
                    Some(s) => {
                        for p in 0..np {
                            let d = s - pitch(p);
                            em_note[c * np + p] += (d * d * inv2s2).min(self.cost_cap);
                        }
                        em_rest[c] += self.voiced_in_rest
                            * fr.voicing[i].max(0.0).powf(self.voicing_pow)
                            * (fr.loud[i] / self.loud_frac).min(1.0);
                    }
                    None => unvoiced += 1,
                }
            }
            for p in 0..np {
                em_note[c * np + p] += unvoiced as f32 * self.unvoiced_in_note;
            }
            if has_cands {
                for p in 0..np {
                    em_note[c * np + p] *= cw;
                }
                em_rest[c] *= cw;
                for i in i0..i1 {
                    em_rest[c] += fr.cand_rest[i];
                    for p in 0..np {
                        em_note[c * np + p] += fr.cand_note[i * np + p];
                    }
                }
            }
            cue[c] = self.cue(fr, a, sp, &at, &env_at);
        }

        let ns = 2 * np + 1;
        let inf = f32::INFINITY;
        let mut cost = vec![inf; ns];
        cost[0] = em_rest[0];
        for p in 0..np {
            cost[1 + p] = self.onset_cost + em_note[p];
        }
        let mut back = vec![0u32; nc * ns];
        let mut next = vec![inf; ns];
        let mut note_prev = vec![0.0f32; np];
        let mut note_arg = vec![0u32; np];
        for c in 1..nc {
            for p in 0..np {
                let (on, co) = (cost[1 + p], cost[1 + np + p]);
                if on <= co {
                    note_prev[p] = on;
                    note_arg[p] = (1 + p) as u32;
                } else {
                    note_prev[p] = co;
                    note_arg[p] = (1 + np + p) as u32;
                }
            }
            let row = &mut back[c * ns..(c + 1) * ns];
            let (mut best, mut arg) = (cost[0], 0u32);
            for p in 0..np {
                let v = note_prev[p] + self.offset_cost;
                if v < best {
                    best = v;
                    arg = note_arg[p];
                }
            }
            next[0] = best + em_rest[c];
            row[0] = arg;
            let reart = (self.rearticulate_cost - self.cue_weight * cue[c]).max(0.0);
            for q in 0..np {
                let e = em_note[c * np + q];
                next[1 + np + q] = note_prev[q] + e;
                row[1 + np + q] = note_arg[q];
                let mut b = cost[0] + self.onset_cost;
                let mut a = 0u32;
                let v = note_prev[q] + reart;
                if v < b {
                    b = v;
                    a = note_arg[q];
                }
                let mut bo = inf;
                let mut ao = 0u32;
                for p in 0..np {
                    if p == q {
                        continue;
                    }
                    let v = note_prev[p] + self.change_cost + self.interval_cost * (p as f32 - q as f32).abs();
                    if v < bo {
                        bo = v;
                        ao = note_arg[p];
                    }
                }
                if bo < b {
                    b = bo;
                    a = ao;
                }
                next[1 + q] = b + e;
                row[1 + q] = a;
            }
            std::mem::swap(&mut cost, &mut next);
        }

        let (mut s, total) = cost.iter().enumerate().fold((0usize, inf), |acc, (i, &v)| if v < acc.1 { (i, v) } else { acc });
        let mut path = vec![0usize; nc];
        for c in (0..nc).rev() {
            path[c] = s;
            s = back[c * ns + s] as usize;
        }
        (notes_from_path(&path, &starts, sp, np, fr.lo), total as f64 / nc as f64)
    }

    fn cue(&self, fr: &Frames, a: f64, sp: f64, at: &dyn Fn(f64) -> usize, env_at: &dyn Fn(f64) -> usize) -> f32 {
        let (w0, w1) = (at(a - 0.04), at(a + 0.04));
        let min_voicing = if w1 > w0 { fr.voicing[w0..w1].iter().copied().fold(f32::INFINITY, f32::min) } else { 1.0 };
        let (r0, r1) = (env_at(a - 0.04), env_at(a + 0.04));
        let (c0, c1) = (env_at(a - sp), env_at(a + sp));
        let dip = if r1 > r0 && c1 > c0 {
            let min = fr.env_rms[r0..r1].iter().copied().fold(f32::INFINITY, f32::min);
            let mean = fr.env_rms[c0..c1].iter().sum::<f32>() / (c1 - c0) as f32;
            min / mean.max(1e-9)
        } else {
            1.0
        };
        (0.75 - min_voicing).max(0.0) + (0.9 - dip).max(0.0)
    }

    fn fill_gaps(&self, notes: Vec<CellNote>, sp: f64) -> Vec<CellNote> {
        fill_gaps(notes, sp, self.fill_gap_cells)
    }

    fn segments_of(&self, notes: &[CellNote], fr: &Frames, onsets: &[Onset], envelope: &[EnergyFrame]) -> Vec<NoteSegment> {
        let end = *fr.time.last().expect("non-empty");
        let born: Vec<Option<f32>> = notes
            .iter()
            .map(|n| {
                onsets
                    .iter()
                    .filter(|o| o.time_s >= n.onset_s - 0.03 && o.time_s <= n.onset_s + 0.05)
                    .map(|o| o.strength)
                    .fold(None, |m: Option<f32>, s| Some(m.map_or(s, |m| m.max(s))))
            })
            .collect();
        let seamless = |i: usize| i > 0 && (notes[i].onset_s - notes[i - 1].offset_s).abs() < 1e-6;
        let pitch_born = |i: usize| born[i].is_none() && seamless(i);
        notes
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let (i0, i1) = (fr.time.partition_point(|&t| t < n.onset_s), fr.time.partition_point(|&t| t < n.offset_s));
                let semis: Vec<f32> = fr.semi[i0..i1].iter().flatten().copied().collect();
                let dispersion = if semis.is_empty() {
                    12.0
                } else {
                    let m = semis.iter().sum::<f32>() / semis.len() as f32;
                    (semis.iter().map(|s| (s - m) * (s - m)).sum::<f32>() / semis.len() as f32).sqrt()
                };
                let voiced_confidence = if i1 > i0 { fr.voicing[i0..i1].iter().sum::<f32>() / (i1 - i0) as f32 } else { 0.0 };
                let kind = if n.offset_s >= end - 1e-6 || i + 1 == notes.len() {
                    OffsetKind::StemEnd
                } else if !seamless(i + 1) {
                    OffsetKind::EnergyDecay
                } else if pitch_born(i + 1) {
                    OffsetKind::PlateauSplit
                } else {
                    OffsetKind::NextOnset
                };
                NoteSegment {
                    onset_s: n.onset_s,
                    offset_s: n.offset_s,
                    midi_pitch: n.midi.clamp(0, 127) as u8,
                    onset_strength: born[i].unwrap_or(0.0),
                    split_from_plateau: pitch_born(i),
                    offset: OffsetInfo { kind, drop_steepness: self.offset_drop(envelope, n.offset_s) },
                    pitch_dispersion_semitones: dispersion,
                    voiced_confidence: voiced_confidence.clamp(0.0, 1.0),
                }
            })
            .collect()
    }

    fn offset_drop(&self, envelope: &[EnergyFrame], offset_s: f64) -> f32 {
        let step = (self.drop_window_s / 4.0).max(1e-4);
        let mut best = 0.0f32;
        let mut t = offset_s;
        while t <= offset_s + 2.0 * self.drop_window_s {
            best = best.max(drop_steepness(envelope, t, self.drop_window_s));
            t += step;
        }
        best
    }
}

/// Model-checked in `verification/grid-notes` (ReadCell).
fn notes_from_path(path: &[usize], starts: &[f64], sp: f64, np: usize, lo: i32) -> Vec<CellNote> {
    let mut notes: Vec<CellNote> = Vec::new();
    for (c, &st) in path.iter().enumerate() {
        if st == 0 {
            continue;
        }
        let midi = lo + ((st - 1) % np) as i32;
        let (a, b) = (starts[c], starts[c] + sp);
        let continues = st > np && notes.last().is_some_and(|n| (n.offset_s - a).abs() < 1e-6 && n.midi == midi);
        if continues {
            notes.last_mut().expect("checked").offset_s = b;
        } else {
            notes.push(CellNote { onset_s: a, offset_s: b, midi });
        }
    }
    notes
}

/// Model-checked in `verification/grid-notes` (FillGap).
fn fill_gaps(mut notes: Vec<CellNote>, sp: f64, max_cells: u32) -> Vec<CellNote> {
    if max_cells == 0 {
        return notes;
    }
    let max_gap = max_cells as f64 * sp + 1e-6;
    for i in 1..notes.len() {
        let gap = notes[i].onset_s - notes[i - 1].offset_s;
        if gap > 1e-6 && gap <= max_gap {
            notes[i - 1].offset_s = notes[i].onset_s;
        }
    }
    notes
}

fn tuning_offset(semis: &[f32]) -> f32 {
    let (mut x, mut y) = (0.0f64, 0.0f64);
    for &s in semis {
        let ang = std::f64::consts::TAU * (s - s.round()) as f64;
        x += ang.cos();
        y += ang.sin();
    }
    (y.atan2(x) / std::f64::consts::TAU) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP_S: f64 = 256.0 / 44_100.0;

    fn stem(segments: &[(f64, f32, f32)]) -> (Vec<F0Frame>, Vec<EnergyFrame>) {
        let end = segments.last().map(|s| s.0).unwrap_or(0.0);
        let n = (end / HOP_S) as usize;
        let mut c = Vec::new();
        let mut e = Vec::new();
        for i in 0..n {
            let t = i as f64 * HOP_S;
            let (_, m, r) = *segments.iter().find(|s| t < s.0).expect("covered");
            let hz = if m > 0.0 { 440.0 * 2f32.powf((m - 69.0) / 12.0) } else { 0.0 };
            c.push(F0Frame { time_s: t, frequency_hz: hz, voicing: if m > 0.0 { 0.95 } else { 0.02 } });
            e.push(EnergyFrame { time_s: t, rms: r });
        }
        (c, e)
    }

    fn grid(anchor: Option<f64>) -> GridSpec {
        GridSpec { bpm: 150.0, division: 16, anchor_s: anchor }
    }

    #[test]
    fn notes_land_on_grid_lines_and_one_cell_notes_survive() {
        let (c, e) = stem(&[(0.2, 0.0, 0.01), (0.3, 64.0, 0.5), (0.6, 67.0, 0.5), (0.8, 0.0, 0.01)]);
        let notes = GridDecoder::default().segment(&grid(Some(0.0)), &[], &c, &e);
        let got: Vec<(f64, f64, u8)> = notes.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch)).collect();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!((got[0].0 - 0.2).abs() < 1e-9 && got[0].2 == 64, "{got:?}");
        assert!((got[1].0 - 0.3).abs() < 1e-9 && (got[1].1 - 0.6).abs() < 1e-9 && got[1].2 == 67, "{got:?}");
    }

    #[test]
    fn a_one_cell_unvoiced_consonant_is_absorbed_by_the_previous_note() {
        let (c, e) = stem(&[(0.1, 0.0, 0.01), (0.4, 64.0, 0.5), (0.5, 0.0, 0.4), (0.8, 66.0, 0.5), (1.0, 0.0, 0.01)]);
        let notes = GridDecoder::default().segment(&grid(Some(0.0)), &[], &c, &e);
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!((notes[0].offset_s - notes[1].onset_s).abs() < 1e-9, "legato across the consonant: {notes:?}");
    }

    #[test]
    fn a_flat_tuned_stem_still_decodes_the_intended_semitone() {
        let (c, e) = stem(&[(0.1, 0.0, 0.01), (0.5, 63.7, 0.5), (0.9, 66.7, 0.5), (1.0, 0.0, 0.01)]);
        let notes = GridDecoder::default().segment(&grid(Some(0.0)), &[], &c, &e);
        let pitches: Vec<u8> = notes.iter().map(|n| n.midi_pitch).collect();
        assert_eq!(pitches, vec![64, 67]);
    }

    #[test]
    fn the_estimated_phase_recovers_a_shifted_grid() {
        let off = 0.037;
        let (c, e) = stem(&[
            (0.1 + off, 0.0, 0.01),
            (0.3 + off, 60.0, 0.5),
            (0.4 + off, 64.0, 0.5),
            (0.7 + off, 62.0, 0.5),
            (0.8 + off, 67.0, 0.5),
            (1.2 + off, 65.0, 0.5),
            (1.4, 0.0, 0.01),
        ]);
        let phase = GridDecoder::default().estimate_phase(&grid(None), &c, &e).expect("voiced");
        assert!((phase - off).abs() < 0.01, "phase {phase}");
    }

    /// Mirrors the invariants of `verification/grid-notes/GridNotes.tla`.
    #[test]
    fn every_short_path_reads_into_a_faithful_monophonic_line() {
        let (np, sp, max_len, fill) = (2usize, 0.1, 6usize, 1u32);
        let n_states = 2 * np + 1;
        for len in 1..=max_len {
            let starts: Vec<f64> = (0..len).map(|c| c as f64 * sp).collect();
            for code in 0..n_states.pow(len as u32) {
                let path: Vec<usize> = (0..len).map(|c| (code / n_states.pow(c as u32)) % n_states).collect();
                let notes = fill_gaps(notes_from_path(&path, &starts, sp, np, 60), sp, fill);
                let cell_of = |t: f64| (t / sp).round() as usize;
                for w in notes.windows(2) {
                    assert!(w[0].offset_s <= w[1].onset_s + 1e-9, "DecodedMonophonic: {path:?} -> {notes:?}");
                }
                for n in &notes {
                    let (a, b) = (n.onset_s / sp, n.offset_s / sp);
                    assert!((a - a.round()).abs() < 1e-9 && (b - b.round()).abs() < 1e-9 && b - a >= 1.0 - 1e-9, "DecodedOnGrid: {path:?} -> {notes:?}");
                }
                let covering = |c: usize| notes.iter().filter(move |n| cell_of(n.onset_s) <= c && c < cell_of(n.offset_s));
                for (c, &st) in path.iter().enumerate() {
                    if st != 0 {
                        let midi = 60 + ((st - 1) % np) as i32;
                        assert!(covering(c).count() == 1 && covering(c).all(|n| n.midi == midi), "SungCellsKept: cell {c} of {path:?} -> {notes:?}");
                    } else if covering(c).count() > 0 {
                        assert!(
                            notes.iter().any(|n| cell_of(n.onset_s) > c && cell_of(n.onset_s) <= c + fill as usize),
                            "RestsStayRests: cell {c} of {path:?} -> {notes:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn silence_yields_no_notes() {
        let (c, e) = stem(&[(1.0, 0.0, 0.0)]);
        assert!(GridDecoder::default().segment(&grid(None), &[], &c, &e).is_empty());
    }
}
