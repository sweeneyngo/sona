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

use crate::annotation::{EnergyFrame, Note};
use crate::confidence::ConfidenceScorer;
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
    /// Seed for the segmenter's boundary sampling. Each transcription starts from it, so a song always
    /// gives the same notes (ADR-0023).
    pub segmenter_seed: f32,
}

impl Default for GameParams {
    fn default() -> Self {
        Self { steps: 8, boundary_threshold: 0.2, boundary_radius: 2, presence_threshold: 0.8, language: 2, chunk_s: 20.0, threads: 8, segmenter_seed: 1.0 }
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

enum Graph<'a> {
    File(&'a Path),
    Bytes(&'a [u8]),
}

fn session(graph: Graph<'_>, threads: usize) -> Result<Session, GameError> {
    let b = Session::builder().map_err(ort_err)?.with_intra_threads(threads).map_err(ort_err)?.with_deterministic_compute(true).map_err(ort_err)?;
    match graph {
        Graph::File(p) => b.commit_from_file(p),
        Graph::Bytes(m) => b.commit_from_memory(m),
    }
    .map_err(ort_err)
}

/// The segmenter with its random node seeded. Refuses a graph with random ops the patch can't reach
/// (inside a subgraph or function): that model would load unseeded and silently vary from run to run.
fn seeded_segmenter(onnx: &[u8], seed: f32) -> Result<Vec<u8>, GameError> {
    let (seeded, n) = crate::onnx_seed::seed_random_nodes(onnx, seed).map_err(GameError::Ort)?;
    let mentions = crate::onnx_seed::RANDOM_OPS.iter().map(|op| onnx.windows(op.len()).filter(|w| w == op).count()).sum::<usize>();
    if n == 0 && mentions > 0 {
        return Err(GameError::Ort("segmenter.onnx has a random op outside its main graph; sona can't seed it, so its notes would vary from run to run".into()));
    }
    Ok(seeded)
}

/// The two GAME runs the learned cell stage reads (ADR-0026): the lead, octave-corrected, with the
/// octave check's per-note ratios, and the vocals; each with its encoder frames.
#[derive(Debug, Clone)]
pub struct StageRuns {
    pub lead_notes: Vec<crate::cellstage::StageNote>,
    pub lead_ratios: Vec<f32>,
    pub lead_frames: Vec<crate::cellstage::EncoderChunk>,
    pub vocals_notes: Vec<crate::cellstage::StageNote>,
    pub vocals_frames: Vec<crate::cellstage::EncoderChunk>,
}

impl StageRuns {
    pub fn compute(game: &GameModel, lead: &[f32], vocals: &[f32], sample_rate: u32) -> Result<Self, GameError> {
        let (mut lead_notes, lead_frames) = game.transcribe_with_frames(lead, sample_rate, game.params)?;
        let lead_ratios = harmonic_ratios(&lead_notes, lead, sample_rate);
        correct_octaves_with(&mut lead_notes, &lead_ratios, OctaveRule::default());
        let (vocals_notes, vocals_frames) = game.transcribe_with_frames(vocals, sample_rate, game.params)?;
        let stage = |ns: &[GameNote]| ns.iter().map(|n| crate::cellstage::StageNote { onset_s: n.onset_s, offset_s: n.offset_s, midi: n.midi, voiced: n.voiced }).collect();
        Ok(Self { lead_notes: stage(&lead_notes), lead_ratios, lead_frames, vocals_notes: stage(&vocals_notes), vocals_frames })
    }
}

/// The cell stage's notes on the grid (onset, offset, MIDI) and each note's confidence
/// ([`note_confidence`](crate::cellstage::note_confidence)), in the same order.
#[derive(Debug, Clone, PartialEq)]
pub struct StageNotes {
    pub notes: Vec<(f64, f64, i32)>,
    pub confidence: Vec<f32>,
}

/// The learned cell stage: decide every cell from `k0` for `n` cells, from the GAME runs, sona's grid
/// notes and the vocals' pitch track and energy; the result is notes on the grid with their confidence.
#[allow(clippy::too_many_arguments)]
pub fn cell_stage_notes(
    model: &CellModel,
    runs: &StageRuns,
    sona_notes: &[(f64, f64, i32)],
    f0: &[(f64, f64)],
    rms: &[(f64, f64)],
    spacing: f64,
    anchor: f64,
    cells: (i64, usize),
) -> Result<StageNotes, GameError> {
    use crate::cellstage::{cell_features, cells_to_notes, choose_class, note_confidence, StageInputs};
    let inputs = StageInputs {
        spacing,
        anchor,
        lead_notes: &runs.lead_notes,
        lead_ratios: &runs.lead_ratios,
        vocals_notes: &runs.vocals_notes,
        sona_notes,
        lead_frames: &runs.lead_frames,
        vocals_frames: &runs.vocals_frames,
        f0,
        rms,
    };
    let (k0, n) = cells;
    let (x, cands, sources) = cell_features(&inputs, k0, n);
    if n > 0 && x.len() != n * model.width {
        return Err(GameError::Ort(format!("the cell model expects {} features per cell, sona built {}", model.width, x.len() / n)));
    }
    let logits = model.logits(&x, n)?;
    let notes = cells_to_notes(&choose_class(&logits, &cands), &cands, &sources, k0, spacing, anchor);
    let confidence = note_confidence(&logits, k0, spacing, anchor, &notes);
    Ok(StageNotes { notes, confidence })
}

/// The grid phase sona's grid notes sit on, in `[0, spacing)`: every note starts on a cell boundary. A
/// phase within a rounding error of a whole cell is 0, so the cells keep their numbering.
pub fn phase_of(notes: &[(f64, f64, i32)], spacing: f64) -> f64 {
    let Some(first) = notes.first() else { return 0.0 };
    let phase = first.0.rem_euclid(spacing);
    if spacing - phase < 1e-6 * spacing {
        0.0
    } else {
        phase
    }
}

/// The cells covering `[0, duration)` on a grid: the first starts at or before 0.
pub fn cells_covering(duration_s: f64, spacing: f64, anchor: f64) -> (i64, usize) {
    let k0 = ((0.0 - anchor) / spacing).floor() as i64;
    let mut n = 0usize;
    while anchor + ((k0 + n as i64) as f64 + 0.5) * spacing < duration_s {
        n += 1;
    }
    (k0, n)
}

/// The chunk's valid frames, each `x_seg ‖ x_est`, rounded to half precision. GAME's encoder runs one
/// chunk at a time (batch 1), so its `maskT` is a leading run of valid frames.
fn encoder_rows(x_seg: &(Vec<i64>, Vec<f32>), x_est: &(Vec<i64>, Vec<f32>), mask: &[bool]) -> (usize, usize, Vec<f32>) {
    let (ds, de) = (*x_seg.0.last().unwrap_or(&0) as usize, *x_est.0.last().unwrap_or(&0) as usize);
    let n = mask.iter().filter(|&&m| m).count();
    let mut data = Vec::with_capacity(n * (ds + de));
    for r in 0..n {
        data.extend(x_seg.1[r * ds..(r + 1) * ds].iter().chain(&x_est.1[r * de..(r + 1) * de]).map(|&v| crate::cellstage::to_f16_and_back(v)));
    }
    (n, ds + de, data)
}

/// The learned cell stage's model (ADR-0026): `cellstage.onnx` and `cellstage.json` from
/// `crates/sona-eval/scripts/cellstage/export_onnx.py`. Trained on Pifuyuu's USTs, so it is yours to
/// build and keep locally, like GAME's weights.
pub struct CellModel {
    session: Mutex<Session>,
    /// Features per cell the model expects.
    pub width: usize,
}

impl std::fmt::Debug for CellModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CellModel").field("width", &self.width).finish_non_exhaustive()
    }
}

impl CellModel {
    pub fn load(dir: &Path, threads: usize) -> Result<Self, GameError> {
        let path = dir.join("cellstage.json");
        let meta: serde_json::Value = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
            .map_err(|e| GameError::Ort(format!("{}: {e}", path.display())))?;
        let (format, choices, width) = (meta["format"].as_u64(), meta["choices"].as_u64(), meta["features"].as_u64());
        let (Some(1), Some(6), Some(width)) = (format, choices, width) else {
            return Err(GameError::Ort(format!("{}: not a cell model sona can run (format {format:?}, {choices:?} choices)", path.display())));
        };
        Ok(Self { session: Mutex::new(session(Graph::File(&dir.join("cellstage.onnx")), threads)?), width: width as usize })
    }

    /// Logits, `cells` × 6, for row-major features `cells` × [`width`](Self::width).
    pub fn logits(&self, x: &[f32], cells: usize) -> Result<Vec<f32>, GameError> {
        if cells == 0 {
            return Ok(Vec::new());
        }
        let mut s = self.session.lock().expect("cell model lock");
        let out = s.run(ort::inputs! { "x" => Tensor::from_array(([1usize, cells, self.width], x.to_vec())).map_err(ort_err)? }).map_err(ort_err)?;
        let (_, d) = out["logits"].try_extract_tensor::<f32>().map_err(ort_err)?;
        Ok(d.to_vec())
    }
}

fn ort_err(e: impl std::fmt::Display) -> GameError {
    GameError::Ort(e.to_string())
}

/// The five GAME graphs, loaded once.
pub struct GameModel {
    encoder: Mutex<Session>,
    /// The segmenter graph as shipped. Each transcription seeds it and opens its own session: ONNX
    /// Runtime creates a seeded generator when a session starts and advances it on every run, with no
    /// reset, so a shared session would make a song's notes depend on what else ran.
    segmenter_onnx: Vec<u8>,
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
        let open = |name: &str| session(Graph::File(&dir.join(format!("{name}.onnx"))), params.threads).map(Mutex::new);
        let segmenter_onnx = std::fs::read(dir.join("segmenter.onnx")).map_err(ort_err)?;
        seeded_segmenter(&segmenter_onnx, params.segmenter_seed)?;
        Ok(Self {
            encoder: open("encoder")?,
            segmenter_onnx,
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
    /// thread count is fixed when the model loads). Safe to call from several threads at once: each call
    /// has its own segmenter session, so concurrent songs don't share random draws.
    pub fn transcribe_with(&self, samples: &[f32], sample_rate: u32, params: GameParams) -> Result<Vec<GameNote>, GameError> {
        self.transcribe_inner(samples, sample_rate, params, None)
    }

    /// [`transcribe_with`](Self::transcribe_with), also returning each chunk's encoder frames, rounded to
    /// half precision as the learned cell stage was trained on (ADR-0026).
    pub fn transcribe_with_frames(&self, samples: &[f32], sample_rate: u32, params: GameParams) -> Result<(Vec<GameNote>, Vec<crate::cellstage::EncoderChunk>), GameError> {
        let mut frames = Vec::new();
        let notes = self.transcribe_inner(samples, sample_rate, params, Some(&mut frames))?;
        Ok((notes, frames))
    }

    fn transcribe_inner(&self, samples: &[f32], sample_rate: u32, params: GameParams, mut frames: Option<&mut Vec<crate::cellstage::EncoderChunk>>) -> Result<Vec<GameNote>, GameError> {
        if sample_rate != GAME_SAMPLE_RATE {
            return Err(GameError::SampleRate(sample_rate));
        }
        let mut segmenter = session(Graph::Bytes(&seeded_segmenter(&self.segmenter_onnx, params.segmenter_seed)?), self.params.threads)?;
        let mut notes = Vec::new();
        let cuts = quiet_cuts(samples, sample_rate, params.chunk_s);
        for w in cuts.windows(2) {
            let t0 = w[0] as f64 / sample_rate as f64;
            let mut t = t0;
            let (chunk_notes, chunk_frames) = self.run_chunk(&mut segmenter, &samples[w[0]..w[1]], sample_rate, params, frames.is_some())?;
            if let (Some(out), Some((n, dim, data))) = (frames.as_deref_mut(), chunk_frames) {
                out.push(crate::cellstage::EncoderChunk { t0, frames: n, dim, data });
            }
            for (dur, midi, voiced) in chunk_notes {
                notes.push(GameNote { onset_s: t, offset_s: t + dur as f64, midi, voiced });
                t += dur as f64;
            }
        }
        Ok(notes)
    }

    #[allow(clippy::type_complexity)]
    fn run_chunk(&self, segmenter: &mut Session, y: &[f32], sr: u32, p: GameParams, want_frames: bool) -> Result<(Vec<(f32, f32, bool)>, Option<(usize, usize, Vec<f32>)>), GameError> {
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
            let out = segmenter
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
        let frames = want_frames.then(|| encoder_rows(&x_seg, &x_est, &mask_t.1));
        Ok(((0..n).map(|i| (durs[i], scores[i], presence[i])).collect(), frames))
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
    correct_octaves_with(notes, &ratios, rule)
}

/// [`correct_octaves`] with the notes' ratios already measured (`ratios[i]` belongs to `notes[i]`).
pub fn correct_octaves_with(notes: &mut [GameNote], ratios: &[f32], rule: OctaveRule) -> usize {
    let mut voiced: Vec<f32> = notes.iter().zip(ratios).filter(|(n, _)| n.voiced).map(|(_, &r)| r).collect();
    if voiced.is_empty() {
        return 0;
    }
    voiced.sort_by(f32::total_cmp);
    let mid = voiced.len() / 2;
    let median = if voiced.len() % 2 == 0 { (voiced[mid - 1] + voiced[mid]) / 2.0 } else { voiced[mid] };
    let threshold = if median < rule.song_threshold { rule.note_threshold_low_song } else { rule.note_threshold };
    let mut moved = 0;
    for (n, &r) in notes.iter_mut().zip(ratios) {
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
    grid_segments(&out)
}

/// The cell stage's notes (onset, offset, MIDI) as note segments; a note whose pitch is not a MIDI number
/// is dropped.
pub fn stage_segments(notes: &[(f64, f64, i32)]) -> Vec<NoteSegment> {
    let midi: Vec<(f64, f64, u8)> = notes.iter().filter_map(|&(a, b, p)| u8::try_from(p).ok().filter(|p| *p <= 127).map(|p| (a, b, p))).collect();
    grid_segments(&midi)
}

/// The cell stage's notes as annotation notes: timing, pitch and sub-scores as `scorer` gives them for
/// [`stage_segments`], and the stage's own confidence as the scalar (ADR-0033). A note whose pitch is not a
/// MIDI number is dropped.
pub fn stage_annotation_notes(stage: &StageNotes, scorer: &ConfidenceScorer, envelope: &[EnergyFrame]) -> Vec<Note> {
    let kept: Vec<(f64, f64, u8, f32)> = stage
        .notes
        .iter()
        .zip(&stage.confidence)
        .filter_map(|(&(a, b, p), &c)| u8::try_from(p).ok().filter(|p| *p <= 127).map(|p| (a, b, p, c)))
        .collect();
    let midi: Vec<(f64, f64, u8)> = kept.iter().map(|&(a, b, p, _)| (a, b, p)).collect();
    scorer
        .score_all(&grid_segments(&midi), envelope)
        .into_iter()
        .zip(&kept)
        .map(|(note, &(.., confidence))| Note { confidence: confidence.clamp(0.0, 1.0), ..note })
        .collect()
}

/// Grid notes (onset, offset, MIDI) as note segments, each offset marked as the next onset, an energy
/// decay or the stem's end.
pub fn grid_segments(out: &[(f64, f64, u8)]) -> Vec<NoteSegment> {
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

    fn melody(pitches: &[f32], seconds_each: f32) -> Vec<f32> {
        let sr = GAME_SAMPLE_RATE as f32;
        let mut seed = 99u32;
        let mut phase = 0.0f32;
        let mut out = Vec::new();
        for &m in pitches {
            for i in 0..(seconds_each * sr) as usize {
                let t = i as f32 / sr;
                let f = 440.0 * 2f32.powf((m - 69.0 + 0.3 * (std::f32::consts::TAU * 5.5 * t).sin()) / 12.0);
                phase += std::f32::consts::TAU * f / sr;
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let noise = ((seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5) * 0.02;
                out.push(0.3 * (phase.sin() + 0.4 * (2.0 * phase).sin() + 0.2 * (3.0 * phase).sin()) + noise);
            }
        }
        out
    }

    #[test]
    fn the_phase_of_notes_on_cell_zero_is_zero_not_one_cell() {
        let sp = 60.0 / 100.0 / 4.0;
        assert_eq!(phase_of(&[(3.0 * sp, 4.0 * sp, 60)], sp), 0.0);
        assert!((phase_of(&[(3.0 * sp + 0.04, 4.0 * sp, 60)], sp) - 0.04).abs() < 1e-9);
        assert_eq!(cells_covering(1.0, sp, phase_of(&[(3.0 * sp, 4.0 * sp, 60)], sp)).0, 0);
    }

    #[test]
    #[ignore = "needs ONNX Runtime and a downloaded GAME release"]
    fn same_notes_for_a_song_whatever_ran_before_it() {
        let dir = std::env::var("SONA_GAME_DIR").expect("SONA_GAME_DIR");
        let model = GameModel::load(Path::new(&dir), GameParams::default()).unwrap();
        let a = melody(&[64.0, 67.0, 69.0, 67.0, 64.0, 62.0, 60.0, 62.0], 0.35);
        let b = melody(&[72.0, 71.0, 69.0, 67.0], 0.5);
        let key = |ns: Vec<GameNote>| ns.iter().map(|n| (n.onset_s.to_bits(), n.offset_s.to_bits(), n.midi.to_bits(), n.voiced)).collect::<Vec<_>>();
        let first = key(model.transcribe(&a, GAME_SAMPLE_RATE).unwrap());
        let _ = model.transcribe(&b, GAME_SAMPLE_RATE).unwrap();
        let again = key(model.transcribe(&a, GAME_SAMPLE_RATE).unwrap());
        assert!(first.len() > 2, "GAME heard a melody: {} notes", first.len());
        assert_eq!(first, again);
        let fresh = GameModel::load(Path::new(&dir), GameParams::default()).unwrap();
        assert_eq!(key(fresh.transcribe(&a, GAME_SAMPLE_RATE).unwrap()), first);

        for _ in 0..4 {
            let (with_b, _) = std::thread::scope(|s| {
                let ta = s.spawn(|| key(model.transcribe(&a, GAME_SAMPLE_RATE).unwrap()));
                let tb = s.spawn(|| model.transcribe(&b, GAME_SAMPLE_RATE).unwrap());
                (ta.join().unwrap(), tb.join().unwrap())
            });
            assert_eq!(with_b, first, "a song transcribed alongside another gets the same notes");
        }

        let seed2 = GameParams { segmenter_seed: 2.0, ..model.params };
        let other = key(model.transcribe_with(&a, GAME_SAMPLE_RATE, seed2).unwrap());
        assert_eq!(key(model.transcribe_with(&a, GAME_SAMPLE_RATE, seed2).unwrap()), other, "a per-call seed is honoured, and repeatable");
    }
}
