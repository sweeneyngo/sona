//! **GAME note extraction** (optional `game` feature): openvpi's GAME singing
//! transcription model, run through ONNX Runtime, then snapped to the song's grid.
//!
//! On the UST corpus's dev songs GAME's notes beat sona's own grid decoder
//! (cell accuracy 86% against 79%, `crates/sona-eval/reports/grid-cell-decoder.md`):
//! it is trained on Japanese singing with accompaniment in the mix, so it decides
//! sung-vs-rest and note boundaries better under harmony.
//!
//! The model is **not bundled**. Its weights are CC BY-NC-SA 4.0 (non-commercial),
//! so the user downloads a release (`GAME-1.0.3-medium-onnx`) and points
//! [`GameModel::load`] at the folder. ONNX Runtime is loaded dynamically: set
//! `ORT_DYLIB_PATH` to a `libonnxruntime` shared library.
//!
//! Pipeline, per ~20 s chunk cut at the quietest point: `encoder` → `segmenter`
//! (a D3PM refinement loop, `steps` passes) → `bd2dur` → `estimator` (pitch and
//! presence per note). [`snap_to_grid`] then removes the stem's tuning offset,
//! estimates the grid phase from the voiced onsets, snaps every boundary to the
//! nearest line and keeps the line monophonic.

use std::path::Path;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

use crate::segment::{NoteSegment, OffsetInfo, OffsetKind};

/// GAME's own audio rate and frame step.
pub const GAME_SAMPLE_RATE: u32 = 44_100;

/// Inference settings. Defaults are the dev-split choices (presence threshold
/// tuned on dev: 0.8 against GAME's recommended 0.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameParams {
    /// Segmenter D3PM refinement passes.
    pub steps: usize,
    /// Boundary decoding threshold.
    pub boundary_threshold: f32,
    /// Boundary decoding radius, frames.
    pub boundary_radius: i64,
    /// A note whose presence confidence is above this is voiced.
    pub presence_threshold: f32,
    /// Language id from the model's `config.json` (`0` = language-neutral).
    pub language: i64,
    /// Target chunk length, seconds; each cut moves to the quietest 50 ms within ±4 s.
    pub chunk_s: f64,
    /// ONNX Runtime intra-op threads.
    pub threads: usize,
}

impl Default for GameParams {
    fn default() -> Self {
        Self { steps: 8, boundary_threshold: 0.2, boundary_radius: 2, presence_threshold: 0.8, language: 2, chunk_s: 20.0, threads: 8 }
    }
}

/// One GAME note in stem seconds, before grid snapping.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GameNote {
    /// Start, seconds.
    pub onset_s: f64,
    /// End, seconds.
    pub offset_s: f64,
    /// Pitch in MIDI semitones (continuous).
    pub midi: f32,
    /// Presence above [`GameParams::presence_threshold`].
    pub voiced: bool,
}

/// Why GAME couldn't run.
#[derive(Debug)]
pub enum GameError {
    /// ONNX Runtime failed (missing `ORT_DYLIB_PATH`, a bad model file, …).
    Ort(String),
    /// The input isn't 44.1 kHz.
    SampleRate(u32),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::Ort(e) => write!(f, "GAME / ONNX Runtime: {e}"),
            GameError::SampleRate(sr) => write!(f, "GAME needs {GAME_SAMPLE_RATE} Hz audio, got {sr} Hz"),
        }
    }
}

impl std::error::Error for GameError {}

fn ort_err(e: impl std::fmt::Display) -> GameError {
    GameError::Ort(e.to_string())
}

/// The five GAME graphs, loaded once.
pub struct GameModel {
    encoder: Mutex<Session>,
    segmenter: Mutex<Session>,
    estimator: Mutex<Session>,
    dur2bd: Mutex<Session>,
    bd2dur: Mutex<Session>,
    /// Inference settings.
    pub params: GameParams,
}

impl std::fmt::Debug for GameModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameModel").field("params", &self.params).finish_non_exhaustive()
    }
}

impl GameModel {
    /// Load `encoder.onnx`, `segmenter.onnx`, `estimator.onnx`, `dur2bd.onnx` and
    /// `bd2dur.onnx` from a GAME ONNX release folder.
    pub fn load(dir: &Path, params: GameParams) -> Result<Self, GameError> {
        const GRAPHS: [&str; 5] = ["encoder", "segmenter", "estimator", "dur2bd", "bd2dur"];
        if let Some(missing) = GRAPHS.iter().map(|g| dir.join(format!("{g}.onnx"))).find(|p| !p.is_file()) {
            return Err(GameError::Ort(format!("{} not found: point at a GAME ONNX release folder", missing.display())));
        }
        // ort's dynamic loader panics when the library can't be opened, so check first.
        match std::env::var_os("ORT_DYLIB_PATH") {
            Some(p) if Path::new(&p).is_file() => {}
            Some(p) => return Err(GameError::Ort(format!("ORT_DYLIB_PATH={} is not a file", Path::new(&p).display()))),
            None => return Err(GameError::Ort("set ORT_DYLIB_PATH to an ONNX Runtime shared library (libonnxruntime.so / .dylib / onnxruntime.dll)".into())),
        }
        let open = |name: &str| -> Result<Mutex<Session>, GameError> {
            let s = Session::builder()
                .map_err(ort_err)?
                .with_intra_threads(params.threads)
                .map_err(ort_err)?
                .commit_from_file(dir.join(format!("{name}.onnx")))
                .map_err(ort_err)?;
            Ok(Mutex::new(s))
        };
        Ok(Self {
            encoder: open("encoder")?,
            segmenter: open("segmenter")?,
            estimator: open("estimator")?,
            dur2bd: open("dur2bd")?,
            bd2dur: open("bd2dur")?,
            params,
        })
    }

    /// Notes for a whole stem (mono, 44.1 kHz), voiced and unvoiced, in order.
    pub fn transcribe(&self, samples: &[f32], sample_rate: u32) -> Result<Vec<GameNote>, GameError> {
        self.transcribe_with(samples, sample_rate, self.params)
    }

    /// [`transcribe`](Self::transcribe) with other inference settings (the
    /// thread count is fixed when the model loads).
    pub fn transcribe_with(&self, samples: &[f32], sample_rate: u32, params: GameParams) -> Result<Vec<GameNote>, GameError> {
        if sample_rate != GAME_SAMPLE_RATE {
            return Err(GameError::SampleRate(sample_rate));
        }
        let mut notes = Vec::new();
        let cuts = quiet_cuts(samples, sample_rate, params.chunk_s);
        for w in cuts.windows(2) {
            let t0 = w[0] as f64 / sample_rate as f64;
            let mut t = t0;
            for (dur, midi, voiced) in self.run_chunk(&samples[w[0]..w[1]], sample_rate, params)? {
                notes.push(GameNote { onset_s: t, offset_s: t + dur as f64, midi, voiced });
                t += dur as f64;
            }
        }
        Ok(notes)
    }

    fn run_chunk(&self, y: &[f32], sr: u32, p: GameParams) -> Result<Vec<(f32, f32, bool)>, GameError> {
        let dur = y.len() as f32 / sr as f32;
        let scalar_f = |v: f32| Tensor::from_array((Vec::<i64>::new(), vec![v])).map_err(ort_err);
        let scalar_i = |v: i64| Tensor::from_array((Vec::<i64>::new(), vec![v])).map_err(ort_err);

        let (x_seg, x_est, mask_t) = {
            let mut enc = self.encoder.lock().expect("encoder lock");
            let out = enc
                .run(ort::inputs! {
                    "waveform" => Tensor::from_array(([1usize, y.len()], y.to_vec())).map_err(ort_err)?,
                    "duration" => Tensor::from_array(([1usize], vec![dur])).map_err(ort_err)?,
                })
                .map_err(ort_err)?;
            let own_f32 = |name: &str| -> Result<(Vec<i64>, Vec<f32>), GameError> {
                let (s, d) = out[name].try_extract_tensor::<f32>().map_err(ort_err)?;
                Ok((s.to_vec(), d.to_vec()))
            };
            let (ms, md) = out["maskT"].try_extract_tensor::<bool>().map_err(ort_err)?;
            (own_f32("x_seg")?, own_f32("x_est")?, (ms.to_vec(), md.to_vec()))
        };
        let mask = || Tensor::from_array((mask_t.0.clone(), mask_t.1.clone())).map_err(ort_err);

        let known = {
            let mut s = self.dur2bd.lock().expect("dur2bd lock");
            let out = s
                .run(ort::inputs! {
                    "durations" => Tensor::from_array(([1usize, 1], vec![dur])).map_err(ort_err)?,
                    "maskT" => mask()?,
                })
                .map_err(ort_err)?;
            let (sh, d) = out["boundaries"].try_extract_tensor::<bool>().map_err(ort_err)?;
            (sh.to_vec(), d.to_vec())
        };
        let mut prev = known.clone();
        for i in 0..p.steps {
            let mut s = self.segmenter.lock().expect("segmenter lock");
            let out = s
                .run(ort::inputs! {
                    "x_seg" => Tensor::from_array((x_seg.0.clone(), x_seg.1.clone())).map_err(ort_err)?,
                    "language" => Tensor::from_array(([1usize], vec![p.language])).map_err(ort_err)?,
                    "known_boundaries" => Tensor::from_array((known.0.clone(), known.1.clone())).map_err(ort_err)?,
                    "prev_boundaries" => Tensor::from_array((prev.0.clone(), prev.1.clone())).map_err(ort_err)?,
                    "t" => Tensor::from_array(([1usize], vec![i as f32 / p.steps as f32])).map_err(ort_err)?,
                    "maskT" => mask()?,
                    "threshold" => scalar_f(p.boundary_threshold)?,
                    "radius" => scalar_i(p.boundary_radius)?,
                })
                .map_err(ort_err)?;
            let (sh, d) = out["boundaries"].try_extract_tensor::<bool>().map_err(ort_err)?;
            prev = (sh.to_vec(), d.to_vec());
        }
        let (durs, mask_n) = {
            let mut s = self.bd2dur.lock().expect("bd2dur lock");
            let out = s
                .run(ort::inputs! {
                    "boundaries" => Tensor::from_array((prev.0.clone(), prev.1.clone())).map_err(ort_err)?,
                    "maskT" => mask()?,
                })
                .map_err(ort_err)?;
            let (_, d) = out["durations"].try_extract_tensor::<f32>().map_err(ort_err)?;
            let (ns, nd) = out["maskN"].try_extract_tensor::<bool>().map_err(ort_err)?;
            (d.to_vec(), (ns.to_vec(), nd.to_vec()))
        };
        let (presence, scores) = {
            let mut s = self.estimator.lock().expect("estimator lock");
            let out = s
                .run(ort::inputs! {
                    "x_est" => Tensor::from_array((x_est.0.clone(), x_est.1.clone())).map_err(ort_err)?,
                    "boundaries" => Tensor::from_array((prev.0.clone(), prev.1.clone())).map_err(ort_err)?,
                    "maskT" => mask()?,
                    "maskN" => Tensor::from_array((mask_n.0.clone(), mask_n.1.clone())).map_err(ort_err)?,
                    "threshold" => scalar_f(p.presence_threshold)?,
                })
                .map_err(ort_err)?;
            let (_, pr) = out["presence"].try_extract_tensor::<bool>().map_err(ort_err)?;
            let (_, sc) = out["scores"].try_extract_tensor::<f32>().map_err(ort_err)?;
            (pr.to_vec(), sc.to_vec())
        };
        let n = mask_n.1.iter().filter(|&&m| m).count();
        Ok((0..n).map(|i| (durs[i], scores[i], presence[i])).collect())
    }
}

/// Sample indices cutting `samples` into ~`chunk_s` pieces, each cut moved to the
/// quietest 50 ms within ±4 s, so no note is split mid-word. Always starts at 0
/// and ends at `samples.len()`.
pub fn quiet_cuts(samples: &[f32], sample_rate: u32, chunk_s: f64) -> Vec<usize> {
    let sr = sample_rate as usize;
    let hop = (sr / 20).max(1); // 50 ms
    let energy: Vec<f32> = samples.chunks(hop).map(|c| c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).collect();
    let (chunk, slack) = ((chunk_s * sr as f64) as usize, 4 * sr);
    let mut cuts = vec![0usize];
    let mut t = chunk;
    while t + 5 * sr < samples.len() {
        let (a, b) = ((t.saturating_sub(slack)) / hop, ((t + slack) / hop).min(energy.len()));
        let k = (a..b).min_by(|&i, &j| energy[i].total_cmp(&energy[j])).unwrap_or(t / hop);
        let cut = (k * hop).max(cuts.last().copied().unwrap_or(0) + sr);
        cuts.push(cut);
        t = cut + chunk;
    }
    cuts.push(samples.len());
    cuts.dedup();
    cuts
}

/// The subharmonic octave check. GAME sometimes tracks high voices an octave
/// low: in three of 28 corpus songs about 60% of its notes sat exactly 12
/// semitones under the UST. A note placed an octave low has almost no energy at
/// its odd harmonics (1, 3, 5 × f), because the true fundamental is 2f.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OctaveRule {
    /// A song whose median odd/even log ratio is below this is octave-low throughout…
    pub song_threshold: f32,
    /// …and there a note moves up when its own ratio is below this.
    pub note_threshold_low_song: f32,
    /// Elsewhere only a note this clearly subharmonic moves up.
    pub note_threshold: f32,
}

impl Default for OctaveRule {
    fn default() -> Self {
        Self { song_threshold: -0.25, note_threshold_low_song: 0.0, note_threshold: -1.5 }
    }
}

const OCT_NFFT: usize = 4096;
const OCT_HOP: usize = 441;

/// Per voiced note, the log ratio of odd-harmonic to even-harmonic energy
/// (`0.0` for unvoiced notes and notes shorter than one analysis frame).
pub fn harmonic_ratios(notes: &[GameNote], samples: &[f32], sample_rate: u32) -> Vec<f32> {
    use realfft::RealFftPlanner;
    let sr = sample_rate as f64;
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(OCT_NFFT);
    let window: Vec<f32> = (0..OCT_NFFT).map(|i| (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (OCT_NFFT - 1) as f64).cos()) as f32).collect();
    let frames = if samples.len() >= OCT_NFFT { 1 + (samples.len() - OCT_NFFT) / OCT_HOP } else { 0 };
    let (mut input, mut spectrum) = (fft.make_input_vec(), fft.make_output_vec());
    let mut mags = vec![0f32; OCT_NFFT / 2 + 1];
    notes
        .iter()
        .map(|n| {
            if !n.voiced {
                return 0.0;
            }
            let a = (n.onset_s * sr / OCT_HOP as f64) as usize;
            let b = ((n.offset_s * sr / OCT_HOP as f64) as usize).max(a + 1).min(frames);
            if a >= b {
                return 0.0;
            }
            let f0 = 440.0 * 2f64.powf((n.midi as f64 - 69.0) / 12.0);
            let mut sums = [0f64; 7];
            for fr in a..b {
                let s = &samples[fr * OCT_HOP..fr * OCT_HOP + OCT_NFFT];
                for ((x, &v), &w) in input.iter_mut().zip(s).zip(&window) {
                    *x = v * w;
                }
                if fft.process(&mut input, &mut spectrum).is_err() {
                    return 0.0;
                }
                for (m, c) in mags.iter_mut().zip(&spectrum) {
                    *m = c.norm();
                }
                for (k, sum) in sums.iter_mut().enumerate().skip(1) {
                    let f = k as f64 * f0;
                    if f * 1.03 >= sr / 2.0 {
                        continue;
                    }
                    let lo = (f * 0.97 * OCT_NFFT as f64 / sr) as usize;
                    let hi = ((f * 1.03 * OCT_NFFT as f64 / sr).ceil() as usize + 1).min(mags.len());
                    *sum += mags[lo..hi].iter().copied().fold(0f32, f32::max) as f64;
                }
            }
            let count = (b - a) as f64;
            let odd = (sums[1] + sums[3] + sums[5]) / count;
            let even = (sums[2] + sums[4] + sums[6]) / count;
            ((odd + 1e-6) / (even + 1e-6)).ln() as f32
        })
        .collect()
}

/// Apply [`OctaveRule`] in place: raise subharmonic notes by an octave. Returns
/// how many notes moved.
pub fn correct_octaves(notes: &mut [GameNote], samples: &[f32], sample_rate: u32, rule: OctaveRule) -> usize {
    let ratios = harmonic_ratios(notes, samples, sample_rate);
    let mut voiced: Vec<f32> = notes.iter().zip(&ratios).filter(|(n, _)| n.voiced).map(|(_, &r)| r).collect();
    if voiced.is_empty() {
        return 0;
    }
    voiced.sort_by(f32::total_cmp);
    let mid = voiced.len() / 2;
    let median = if voiced.len() % 2 == 0 { (voiced[mid - 1] + voiced[mid]) / 2.0 } else { voiced[mid] };
    let threshold = if median < rule.song_threshold { rule.note_threshold_low_song } else { rule.note_threshold };
    let mut moved = 0;
    for (n, &r) in notes.iter_mut().zip(&ratios) {
        if n.voiced && r < threshold {
            n.midi += 12.0;
            moved += 1;
        }
    }
    moved
}

/// One grid cell in seconds at `bpm` for a grid of `division` cells per whole
/// note (16 = sixteenths, 24 = sixteenth triplets).
pub fn grid_spacing_s(bpm: f64, division: u32) -> f64 {
    240.0 / bpm / f64::from(division.max(1))
}

/// Snap GAME's notes to a grid of `spacing_s` seconds: remove the stem's tuning
/// offset, estimate the phase from the voiced onsets (circular mean), snap each
/// voiced note's onset and offset to the nearest line (at least one cell), and
/// keep the line monophonic — a later note that starts inside an earlier one
/// trims it; one on the same line moves to its end.
pub fn snap_to_grid(notes: &[GameNote], spacing_s: f64) -> Vec<NoteSegment> {
    let voiced: Vec<&GameNote> = notes.iter().filter(|n| n.voiced && n.offset_s > n.onset_s).collect();
    if voiced.is_empty() || spacing_s.is_nan() || spacing_s <= 0.0 {
        return Vec::new();
    }
    let tune = {
        let (x, y) = voiced.iter().fold((0.0f64, 0.0f64), |(x, y), n| {
            let a = std::f64::consts::TAU * (n.midi - n.midi.round()) as f64;
            (x + a.cos(), y + a.sin())
        });
        (y.atan2(x) / std::f64::consts::TAU) as f32
    };
    let phase = {
        let (x, y) = voiced.iter().fold((0.0f64, 0.0f64), |(x, y), n| {
            let a = std::f64::consts::TAU * n.onset_s / spacing_s;
            (x + a.cos(), y + a.sin())
        });
        y.atan2(x) / std::f64::consts::TAU * spacing_s
    };
    let snap = |t: f64| phase + ((t - phase) / spacing_s).round() * spacing_s;
    let mut out: Vec<(f64, f64, u8)> = voiced
        .iter()
        .map(|n| {
            let on = snap(n.onset_s);
            let mut off = snap(n.offset_s);
            if off <= on + 1e-9 {
                off = on + spacing_s;
            }
            (on, off, (n.midi - tune).round().clamp(0.0, 127.0) as u8)
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    for i in 1..out.len() {
        if out[i].0 < out[i - 1].1 - 1e-9 {
            if out[i].0 > out[i - 1].0 + 1e-9 {
                out[i - 1].1 = out[i].0;
            } else {
                out[i].0 = out[i - 1].1;
                if out[i].1 <= out[i].0 + 1e-9 {
                    out[i].1 = out[i].0 + spacing_s;
                }
            }
        }
    }
    let n = out.len();
    out.iter()
        .enumerate()
        .map(|(i, &(on, off, midi))| {
            let seamless_next = i + 1 < n && (out[i + 1].0 - off).abs() < 1e-6;
            NoteSegment {
                onset_s: on,
                offset_s: off,
                midi_pitch: midi,
                onset_strength: 1.0,
                split_from_plateau: false,
                offset: OffsetInfo {
                    kind: if i + 1 == n { OffsetKind::StemEnd } else if seamless_next { OffsetKind::NextOnset } else { OffsetKind::EnergyDecay },
                    drop_steepness: 0.5,
                },
                pitch_dispersion_semitones: 0.0,
                voiced_confidence: 1.0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(on: f64, off: f64, midi: f32, voiced: bool) -> GameNote {
        GameNote { onset_s: on, offset_s: off, midi, voiced }
    }

    #[test]
    fn snapping_lands_on_one_grid_removes_tuning_and_drops_rests() {
        // 0.1 s cells, phase 0.03, everything a quarter-tone flat, one rest.
        let notes = [
            note(0.031, 0.228, 63.75, true),
            note(0.228, 0.335, 0.0, false),
            note(0.335, 0.531, 66.74, true),
            note(0.531, 0.73, 64.76, true),
        ];
        let segs = snap_to_grid(&notes, 0.1);
        // The phase is estimated (≈0.03 here), so read cells relative to the first onset.
        let g0 = segs[0].onset_s;
        assert!((g0 - 0.03).abs() < 0.01, "phase near 0.03: {g0}");
        let cell = |t: f64| (t - g0) / 0.1;
        let got: Vec<(i64, i64, u8)> = segs.iter().map(|s| (cell(s.onset_s).round() as i64, cell(s.offset_s).round() as i64, s.midi_pitch)).collect();
        assert_eq!(got, vec![(0, 2, 64), (3, 5, 67), (5, 7, 65)]);
        for s in &segs {
            for t in [s.onset_s, s.offset_s] {
                assert!((cell(t) - cell(t).round()).abs() < 1e-6, "off the grid: {s:?}");
            }
        }
    }

    #[test]
    fn snapping_keeps_one_voice_when_two_notes_round_onto_one_line() {
        let segs = snap_to_grid(&[note(0.00, 0.04, 60.0, true), note(0.04, 0.24, 62.0, true)], 0.1);
        for w in segs.windows(2) {
            assert!(w[0].offset_s <= w[1].onset_s + 1e-9, "{segs:?}");
        }
        assert_eq!(segs.len(), 2, "no note is dropped: {segs:?}");
    }

    fn tone(f: f64, harmonics: &[usize], secs: f64) -> Vec<f32> {
        let sr = 44_100.0;
        (0..(secs * sr) as usize)
            .map(|i| harmonics.iter().map(|&k| (std::f64::consts::TAU * k as f64 * f * i as f64 / sr).sin() / k as f64).sum::<f64>() as f32 * 0.2)
            .collect()
    }

    #[test]
    fn a_note_placed_an_octave_low_is_raised_and_a_true_one_is_not() {
        // Sung at 440 Hz with all its harmonics. GAME says 220 Hz (A3) or 440 Hz (A4).
        let y = tone(440.0, &[1, 2, 3, 4, 5, 6, 7], 1.0);
        let low = [note(0.1, 0.9, 57.0, true)];
        let right = [note(0.1, 0.9, 69.0, true)];
        assert!(harmonic_ratios(&low, &y, 44_100)[0] < -1.5, "odd harmonics of 220 Hz are empty");
        assert!(harmonic_ratios(&right, &y, 44_100)[0] > 0.0, "a true fundamental has odd harmonics");
        let mut n = low;
        assert_eq!(correct_octaves(&mut n, &y, 44_100, OctaveRule::default()), 1);
        assert_eq!(n[0].midi, 69.0);
        let mut n = right;
        assert_eq!(correct_octaves(&mut n, &y, 44_100, OctaveRule::default()), 0);
        assert_eq!(n[0].midi, 69.0);
    }

    #[test]
    fn quiet_cuts_cover_the_stem_and_cut_at_silence() {
        let sr = 1000u32;
        // 50 s of tone with a silent 0.5 s gap at 21 s.
        let samples: Vec<f32> = (0..50 * sr as usize).map(|i| if (21_000..21_500).contains(&i) { 0.0 } else { 0.5 }).collect();
        let cuts = quiet_cuts(&samples, sr, 20.0);
        assert_eq!(cuts.first(), Some(&0));
        assert_eq!(cuts.last(), Some(&samples.len()));
        assert!(cuts.iter().any(|&c| (21_000..21_500).contains(&c)), "a cut lands in the silence: {cuts:?}");
    }
}
