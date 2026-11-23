//! **Dev-split iteration harness** for the in-domain UST corpus: score one or
//! more segmentation configs on the manifest's `dev` songs (`--split NAME
//! --reason "…"` reads a held-out split instead, logged), with Vocadito as a
//! no-regression guard.
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-ust-dev -- \
//!     --cfg base: --cfg grid:grid=est [--vocadito] [--diag] [--stem lead]
//! ```
//!
//! Each stem's f0 contour is cached under `target/ust-dev-cache/` (or `$SONA_DEV_CACHE`), keyed by the
//! tracker's settings. A config is `name:key=value,key=value`; see [`apply`]
//! for the keys. `--diag` prints where the first config's misses go.
//!
//! `cands=1` (or any `c.*` knob) adds harmonic-salience pitch candidates to
//! the cell emission (`GridDecoder::candidates`); they are cached beside the
//! contour. `--cand-diag` prints how often the lead's pitch is pYIN's f0 or a
//! top-k candidate, monophonic vs polyphonic.
//! The spectral re-articulation cue (`g.cue=spectral`, `g.sc.*`) needs
//! `--features spectral`.

use std::path::PathBuf;
use std::process::ExitCode;

use sona_core::grid::CandidateEvidence;
use sona_core::pitch::{HarmonicSalience, PitchCandidate};
use sona_core::{Annotation, CueModel, F0Frame, GridDecoder, GridSpec, PYin, PitchTracker, Producer, SpectralCue, TimbreFrames, Transcriber};
use sona_eval::estimate::EstNote;
use sona_eval::metrics::note::{match_notes, NoteParams};
use sona_eval::metrics::Prf1;
use sona_eval::quantize::{grid_spacing_seconds, quantize_note, resolve_overlaps, QuantizeOptions};
use sona_eval::reference::RefNote;
use sona_eval::ust_corpus::{manifest_path, overlaps_any, stems_dir, Manifest, SongUsts};
use sona_eval::{ust, vocadito, wav, EvalError};

struct Item {
    id: String,
    samples: Vec<f32>,
    sr: u32,
    contour: Vec<F0Frame>,
    /// Per-frame harmonic-salience pitch candidates, aligned with `contour`;
    /// loaded only when a config weighs them (`c.*`) or `--cand-diag` asks.
    cands: Option<Vec<Vec<PitchCandidate>>>,
    timbre: Option<TimbreFrames>,
    grid: Option<Grid>,
    refs: Vec<RefNote>,
    /// Every other voice's notes (harmonies, other lead parts' overlaps).
    others: Vec<RefNote>,
    poly: Vec<(f64, f64)>,
}

struct Grid {
    opts: QuantizeOptions,
    bpm: f64,
    division: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum GridChoice {
    Off,
    Estimated,
    UstAnchor,
}

fn cache_dir() -> PathBuf {
    std::env::var_os("SONA_DEV_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ust-dev-cache"))
}

fn tracker_key(p: &impl std::fmt::Debug) -> String {
    let s = format!("{p:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// An external front end's output (e.g. a neural f0 model run offline), when
/// `$env` names a directory holding `<id>.json`. Lets a new pitch source be scored
/// by the real decoder before it has a Rust backend.
fn external<T: serde::de::DeserializeOwned>(env: &str, id: &str) -> Option<Result<T, EvalError>> {
    let dir = std::env::var_os(env)?;
    let path = PathBuf::from(dir).join(format!("{id}.json"));
    Some(std::fs::read_to_string(&path).map_err(|e| EvalError::io(&path, e)).and_then(|t| serde_json::from_str(&t).map_err(|e| EvalError::parse(&path, e.to_string()))))
}

fn contour_cached(id: &str, samples: &[f32], sr: u32, pyin: &PYin) -> Result<Vec<F0Frame>, EvalError> {
    if let Some(c) = external("SONA_CONTOUR_DIR", id) {
        return c;
    }
    let path = cache_dir().join(format!("{id}.{}.f0.json", tracker_key(pyin)));
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(c) = serde_json::from_str(&text) {
            return Ok(c);
        }
    }
    eprintln!("  tracking pitch for {id} (cached after this run)…");
    let c = pyin.track(samples, sr);
    std::fs::create_dir_all(cache_dir()).map_err(|e| EvalError::io(cache_dir(), e))?;
    let text = serde_json::to_string(&c).map_err(|e| EvalError::parse(&path, e.to_string()))?;
    // Write-then-rename, so parallel runs sharing SONA_DEV_CACHE never read a torn file.
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| EvalError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| EvalError::io(&path, e))?;
    Ok(c)
}

fn cands_cached(id: &str, samples: &[f32], sr: u32) -> Result<Vec<Vec<PitchCandidate>>, EvalError> {
    if let Some(c) = external("SONA_CANDS_DIR", id) {
        return c;
    }
    let hs = HarmonicSalience::default();
    let path = cache_dir().join(format!("{id}.hs{}.cand.json", tracker_key(&hs)));
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(c) = serde_json::from_str(&text) {
            return Ok(c);
        }
    }
    eprintln!("  harmonic salience for {id} (cached after this run)…");
    let c = hs.track(samples, sr);
    std::fs::create_dir_all(cache_dir()).map_err(|e| EvalError::io(cache_dir(), e))?;
    let text = serde_json::to_string(&c).map_err(|e| EvalError::parse(&path, e.to_string()))?;
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, text).map_err(|e| EvalError::io(&tmp, e))?;
    std::fs::rename(&tmp, &path).map_err(|e| EvalError::io(&path, e))?;
    Ok(c)
}

#[derive(Clone)]
struct Cfg {
    t: Transcriber,
    grid: GridChoice,
    cell: GridDecoder,
    /// GAME note extraction at this presence threshold, snapped to the song's grid.
    game: Option<f32>,
    /// Apply GAME's subharmonic octave rule (`oct=1`).
    game_oct: bool,
}

fn apply(cfg: &mut Cfg, key: &str, val: &str) -> Result<(), String> {
    let f = || val.parse::<f64>().map_err(|e| format!("{key}={val}: {e}"));
    let g = &mut cfg.cell;
    match key {
        "game" => {
            cfg.game = Some(f()? as f32);
            return Ok(());
        }
        "oct" => {
            cfg.game_oct = f()? != 0.0;
            return Ok(());
        }
        "grid" => {
            cfg.grid = match val {
                "est" | "1" => GridChoice::Estimated,
                "ust" => GridChoice::UstAnchor,
                "0" | "off" => GridChoice::Off,
                _ => return Err(format!("grid={val}: want est|ust|off")),
            };
            return Ok(());
        }
        "g.sigma" => g.sigma = f()? as f32,
        "g.cost_cap" => g.cost_cap = f()? as f32,
        "g.unvoiced_in_note" => g.unvoiced_in_note = f()? as f32,
        "g.voiced_in_rest" => g.voiced_in_rest = f()? as f32,
        "g.voicing_pow" => g.voicing_pow = f()? as f32,
        "g.loud_frac" => g.loud_frac = f()? as f32,
        "g.onset_cost" => g.onset_cost = f()? as f32,
        "g.offset_cost" => g.offset_cost = f()? as f32,
        "g.rearticulate_cost" => g.rearticulate_cost = f()? as f32,
        "g.cue_weight" => g.cue_weight = f()? as f32,
        "g.change_cost" => g.change_cost = f()? as f32,
        "g.interval_cost" => g.interval_cost = f()? as f32,
        "g.estimate_tuning" => g.estimate_tuning = f()? != 0.0,
        "g.fill_gap_cells" => g.fill_gap_cells = f()? as u32,
        "g.phase_candidates" => g.phase_candidates = f()? as usize,
        "cands" => g.candidates = (f()? != 0.0).then(CandidateEvidence::default),
        k if k.starts_with("c.") => {
            let e = g.candidates.get_or_insert_with(CandidateEvidence::default);
            match k {
                "c.weight" => e.weight = f()? as f32,
                "c.rank_cost" => e.rank_cost = f()? as f32,
                "c.rest_cost" => e.rest_cost = f()? as f32,
                "c.rest_pow" => e.rest_pow = f()? as f32,
                "c.pct" => e.strength_percentile = f()? as f32,
                "c.contour_weight" => e.contour_weight = f()? as f32,
                "c.lag_s" => e.lag_s = f()?,
                _ => return Err(format!("unknown knob {k}")),
            }
        }
        "g.cue" => {
            g.cue_model = match val {
                "ve" | "voicing-energy" => CueModel::VoicingEnergy,
                "spectral" => CueModel::Spectral(SpectralCue::default()),
                _ => return Err(format!("g.cue={val}: want ve|spectral")),
            }
        }
        k if k.starts_with("g.sc.") => {
            if !matches!(g.cue_model, CueModel::Spectral(_)) {
                g.cue_model = CueModel::Spectral(SpectralCue::default());
            }
            let CueModel::Spectral(m) = &mut g.cue_model else { unreachable!("set above") };
            let x = f()? as f32;
            match &k[5..] {
                "flux" => m.flux = x,
                "ceps" => m.ceps = x,
                "flux_lo" => m.flux_lo = x,
                "bias" => m.bias = x,
                "threshold" => m.threshold = x,
                _ => return Err(format!("unknown knob {k}")),
            }
        }
        _ => return apply_t(&mut cfg.t, key, val),
    }
    Ok(())
}

fn apply_t(t: &mut Transcriber, key: &str, val: &str) -> Result<(), String> {
    let f = || val.parse::<f64>().map_err(|e| format!("{key}={val}: {e}"));
    let s = &mut t.segmenter;
    let p = &mut s.params;
    let o = &mut t.onset;
    match key {
        "min_note_s" => s.min_note_s = f()?,
        "bleed_floor_frac" => s.bleed_floor_frac = f()? as f32,
        "onset_match_s" => s.onset_match_s = f()?,
        "alpha_trim" => s.alpha_trim = f()? as f32,
        "release_drop_threshold" => s.release_drop_threshold = f()? as f32,
        "median_window" => s.prefilter.median_window = f()? as usize,
        "stable_self" => p.stable_self = f()? as f32,
        "attack_self" => p.attack_self = f()? as f32,
        "silent_self" => p.silent_self = f()? as f32,
        "interval_lambda" => p.interval_lambda = f()? as f32,
        "onset_attack_bonus" => p.onset_attack_bonus = f()? as f32,
        "pitch_trust" => p.pitch_trust = f()? as f32,
        "voicing_prior" => p.voicing_prior = f()? as f32,
        "sigma_stable" => p.sigma_stable = f()? as f32,
        "sigma_attack" => p.sigma_attack = f()? as f32,
        "stable_to_silent_frac" => p.stable_to_silent_frac = f()? as f32,
        "onset.threshold_delta" => o.threshold_delta = f()? as f32,
        "onset.min_gap_s" => o.min_gap_s = f()?,
        "onset.bleed_floor_frac" => o.bleed_floor_frac = f()? as f32,
        "onset.threshold_window_s" => o.threshold_window_s = f()?,
        _ => return Err(format!("unknown knob {key}")),
    }
    Ok(())
}

fn parse_cfg(spec: &str) -> Result<(String, Cfg), String> {
    let (name, rest) = spec.split_once(':').unwrap_or((spec, ""));
    let mut cfg = Cfg { t: Transcriber::default(), grid: GridChoice::Off, cell: GridDecoder::default(), game: None, game_oct: false };
    for kv in rest.split(',').filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("bad knob {kv}"))?;
        apply(&mut cfg, k, v)?;
    }
    if matches!(cfg.cell.cue_model, CueModel::Spectral(_)) && !cfg!(feature = "spectral") {
        return Err(format!("{name}: the spectral cue needs --features spectral"));
    }
    Ok((name.to_string(), cfg))
}

#[cfg(feature = "game")]
fn game_segments(it: &Item, pthr: f32, oct: bool, spacing: f64) -> Vec<sona_core::NoteSegment> {
    use sona_core::game::{correct_octaves, snap_to_grid, GameModel, GameNote, GameParams, OctaveRule};
    static MODEL: std::sync::OnceLock<GameModel> = std::sync::OnceLock::new();
    let dir = std::env::var("SONA_GAME_DIR").expect("game=… needs SONA_GAME_DIR (a GAME ONNX release folder)");
    let tag = std::path::Path::new(&dir).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    // The stem is part of the key: a cheap fingerprint of the audio itself.
    let print = it.samples.iter().step_by(997).fold(it.samples.len() as u64, |h, x| h.rotate_left(5) ^ u64::from(x.to_bits()));
    let seed = GameParams::default().segmenter_seed;
    let path = cache_dir().join(format!("{}.game-{tag}-p{pthr}-seed{seed}-{print:016x}.json", it.id));
    let mut notes: Vec<GameNote> = match std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<Vec<(f64, f64, f32, bool)>>(&t).ok()) {
        Some(v) => v.into_iter().map(|(a, b, m, v)| GameNote { onset_s: a, offset_s: b, midi: m, voiced: v }).collect(),
        None => {
            let threads = std::env::var("SONA_GAME_THREADS").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
            let model = MODEL.get_or_init(|| {
                GameModel::load(std::path::Path::new(&dir), GameParams { threads, ..GameParams::default() }).expect("load GAME")
            });
            eprintln!("  GAME for {} (cached after this run)…", it.id);
            let notes = model.transcribe_with(&it.samples, it.sr, GameParams { presence_threshold: pthr, ..model.params }).expect("GAME inference");
            let raw: Vec<(f64, f64, f32, bool)> = notes.iter().map(|n| (n.onset_s, n.offset_s, n.midi, n.voiced)).collect();
            let _ = std::fs::write(&path, serde_json::to_string(&raw).unwrap_or_default());
            notes
        }
    };
    if oct {
        correct_octaves(&mut notes, &it.samples, it.sr, OctaveRule::default());
    }
    snap_to_grid(&notes, spacing)
}

#[cfg(not(feature = "game"))]
fn game_segments(_: &Item, _: f32, _: bool, _: f64) -> Vec<sona_core::NoteSegment> {
    panic!("game=… configs need `--features game`");
}

fn transcribe(cfg: &Cfg, it: &Item) -> Annotation {
    let t = &cfg.t;
    let onset = t.onset.detect(&it.samples, it.sr);
    let segs = match (&it.grid, cfg.grid, cfg.game) {
        (Some(g), _, Some(pthr)) => game_segments(it, pthr, cfg.game_oct, g.opts.spacing),
        (Some(g), GridChoice::Estimated | GridChoice::UstAnchor, None) => {
            let anchor = (cfg.grid == GridChoice::UstAnchor).then_some(g.opts.anchor);
            let grid = GridSpec { bpm: g.bpm, division: g.division, anchor_s: anchor };
            let cands = it.cands.as_deref().unwrap_or(&[]);
            cfg.cell.segment_with_evidence(&grid, &onset.onsets, &it.contour, cands, &onset.energy_envelope, it.timbre.as_ref())
        }
        _ => t.segmenter.segment(&onset.onsets, &it.contour, &onset.energy_envelope),
    };
    let notes = t.scorer.score_all(&segs, &onset.energy_envelope);
    Annotation {
        version: sona_core::SCHEMA_VERSION,
        sample_rate: it.sr,
        bpm: 120.0,
        duration_s: it.samples.len() as f64 / it.sr as f64,
        producer: Producer::Pyin,
        notes,
        f0_contour: Vec::new(),
        energy_envelope: Vec::new(),
        provenance: None,
    }
}

fn est(a: &Annotation, q: Option<&QuantizeOptions>) -> Vec<EstNote> {
    let raw = a.notes.iter().map(|n| EstNote { onset_s: n.onset_s, offset_s: n.offset_s, midi_pitch: n.midi_pitch, confidence: n.confidence });
    let Some(o) = q else { return raw.collect() };
    let mut qs: Vec<_> = a.notes.iter().map(|n| quantize_note(n.onset_s, n.offset_s, Some(n.sub_scores.onset_clarity), o)).collect();
    resolve_overlaps(&mut qs, o.spacing);
    raw.zip(qs).map(|(e, q)| EstNote { onset_s: q.onset_s, offset_s: q.offset_s, ..e }).collect()
}

#[derive(Default, Clone, Copy)]
struct Counts {
    n_ref: usize,
    n_est: usize,
    conp: usize,
    conpoff: usize,
    cells: [(usize, usize); 4],
    /// Sung lead cells in polyphonic regions: right, rest, a harmony's pitch,
    /// octave error, other pitch.
    poly_cells: [usize; 5],
}

impl Counts {
    fn of(refs: &[RefNote], est: &[EstNote], p: &NoteParams) -> Self {
        Counts {
            n_ref: refs.len(),
            n_est: est.len(),
            conp: match_notes(refs, est, p, false).len(),
            conpoff: match_notes(refs, est, p, true).len(),
            cells: [(0, 0); 4],
            poly_cells: [0; 5],
        }
    }
    fn add(&mut self, o: Counts) {
        self.n_ref += o.n_ref;
        self.n_est += o.n_est;
        self.conp += o.conp;
        self.conpoff += o.conpoff;
        for i in 0..4 {
            self.cells[i].0 += o.cells[i].0;
            self.cells[i].1 += o.cells[i].1;
        }
        for i in 0..5 {
            self.poly_cells[i] += o.poly_cells[i];
        }
    }
    fn conp(&self) -> Prf1 {
        Prf1::from_counts(self.conp, self.n_ref, self.n_est)
    }
    fn conpoff(&self) -> Prf1 {
        Prf1::from_counts(self.conpoff, self.n_ref, self.n_est)
    }
}

type CellStats = ([(usize, usize); 4], [usize; 5]);

fn cell_accuracy(it: &Item, est: &[EstNote], spacing: f64, anchor: f64) -> CellStats {
    let (refs, poly) = (&it.refs, &it.poly);
    let mut out = [(0, 0); 4];
    let mut pc = [0usize; 5];
    let (Some(first), Some(last)) = (refs.iter().map(|r| r.onset_s).reduce(f64::min), refs.iter().map(|r| r.offset_s).reduce(f64::max)) else {
        return (out, pc);
    };
    fn at(v: impl Iterator<Item = (f64, f64, u8)>, t: f64) -> Option<u8> {
        v.filter(|n| n.0 <= t && t < n.1).map(|n| n.2).next()
    }
    let mut k = ((first - anchor) / spacing).round() as i64;
    while anchor + (k as f64 + 0.5) * spacing < last {
        let t = anchor + (k as f64 + 0.5) * spacing;
        k += 1;
        let r = at(refs.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch)), t);
        let e = at(est.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch)), t);
        let ok = (r == e) as usize;
        let mono = !overlaps_any(poly, t - spacing / 2.0, t + spacing / 2.0);
        let mut add = |i: usize| {
            out[i].0 += ok;
            out[i].1 += 1;
        };
        add(0);
        if mono {
            add(1);
        }
        if let Some(rp) = r {
            add(2);
            if mono {
                add(3);
            } else {
                let harm = |p: u8| it.others.iter().any(|n| n.onset_s <= t && t < n.offset_s && n.midi_pitch == p);
                pc[match e {
                    Some(p) if p == rp => 0,
                    None => 1,
                    Some(p) if harm(p) => 2,
                    Some(p) if (p as i32 - rp as i32).abs() == 12 => 3,
                    Some(_) => 4,
                }] += 1;
            }
        }
    }
    (out, pc)
}

fn score_item(t: &Cfg, it: &Item, p: &NoteParams) -> (Vec<Counts>, Annotation) {
    let a = transcribe(t, it);
    let raw = est(&a, None);
    let Some(g) = &it.grid else {
        return (vec![Counts::of(&it.refs, &raw, p)], a);
    };
    let quant = est(&a, Some(&g.opts));
    let (cells, poly_cells) = cell_accuracy(it, &quant, g.opts.spacing, g.opts.anchor);
    let mono_refs: Vec<RefNote> = it.refs.iter().copied().filter(|r| !overlaps_any(&it.poly, r.onset_s, r.offset_s)).collect();
    let mono = |v: &[EstNote]| -> Vec<EstNote> { v.iter().copied().filter(|n| !overlaps_any(&it.poly, n.onset_s, n.offset_s)).collect() };
    (
        vec![
            Counts::of(&it.refs, &raw, p),
            Counts { cells, poly_cells, ..Counts::of(&it.refs, &quant, p) },
            Counts::of(&mono_refs, &mono(&raw), p),
            Counts::of(&mono_refs, &mono(&quant), p),
            // Mono references against the whole line: unlike [3], a note that
            // runs on into a polyphonic span still counts.
            Counts::of(&mono_refs, &quant, p),
        ],
        a,
    )
}

fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    let out = std::sync::Mutex::new(&mut slots);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(item) = items.get(i) else { break };
                let r = f(item);
                out.lock().expect("no worker panicked")[i] = Some(r);
            });
        }
    });
    slots.into_iter().map(|r| r.expect("processed")).collect()
}

fn load_ust_items(split: &str, pyin: &PYin, stem: &str, with_cands: bool) -> Result<Vec<Item>, EvalError> {
    let file = format!("{stem}.wav");
    let manifest = Manifest::load(&manifest_path())?;
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let songs: Vec<_> = manifest
        .songs
        .iter()
        .filter(|s| s.split.as_deref() == Some(split) && s.stratum == "vocaloid-original")
        .filter(|s| s.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) && s.alignment_offset_s.is_some())
        .filter(|s| stems.join(&s.id).join(&file).is_file())
        // SONA_ONLY=id,id: score just these songs (quick checks).
        .filter(|s| std::env::var("SONA_ONLY").map_or(true, |only| only.split(',').any(|o| o == s.id)))
        .collect();
    let items: Vec<Result<Item, EvalError>> = parallel_map(&songs, |song| {
        let offset = song.alignment_offset_s.expect("filtered");
        let usts = SongUsts::load(song, &cache)?;
        let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
        let spacing = grid_spacing_seconds(bpm, song.grid_division.unwrap_or(16) as f64);
        let (samples, sr) = wav::read_mono(&stems.join(&song.id).join(&file))?;
        let key = if stem == "vocals" { song.id.clone() } else { format!("{}.{stem}", song.id) };
        let contour = contour_cached(&key, &samples, sr, pyin)?;
        let cands = if with_cands { Some(cands_cached(&key, &samples, sr)?) } else { None };
        let mut others: Vec<RefNote> = usts.other.iter().flat_map(|u| u.to_reference(offset).notes).collect();
        if usts.lead.len() > 1 {
            others.extend(usts.lead.iter().flat_map(|u| u.to_reference(offset).notes));
        }
        Ok(Item {
            id: song.id.clone(),
            timbre: timbre_of(&samples, sr),
            samples,
            sr,
            contour,
            cands,
            grid: Some(Grid { opts: QuantizeOptions::new(spacing, offset), bpm, division: song.grid_division.unwrap_or(16) }),
            refs: usts.reference(offset).notes,
            others,
            poly: usts.polyphonic_spans(offset),
        })
    });
    items.into_iter().collect()
}

/// Timbre frames on the default onset detector's frame grid (spectral builds only).
fn timbre_of(samples: &[f32], sr: u32) -> Option<TimbreFrames> {
    #[cfg(feature = "spectral")]
    {
        let o = sona_core::OnsetDetector::default();
        Some(TimbreFrames::from_samples(samples, sr, o.frame_length, o.hop_length))
    }
    #[cfg(not(feature = "spectral"))]
    {
        let _ = (samples, sr);
        None
    }
}

fn load_vocadito(pyin: &PYin) -> Result<Vec<Item>, EvalError> {
    let corpus = vocadito::discover(&vocadito::corpus_dir())?;
    parallel_map(&corpus.tracks, |track| {
        let mut refs = vocadito::load_reference(track)?.notes;
        refs.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s));
        let (samples, sr) = wav::read_mono(&track.audio)?;
        let id = format!("vocadito-{}", track.audio.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let contour = contour_cached(&id, &samples, sr, pyin)?;
        // Vocadito has no grid, so grid mode (and its evidence tracks) never runs.
        Ok(Item { id, timbre: timbre_of(&samples, sr), samples, sr, contour, cands: None, grid: None, refs, others: Vec::new(), poly: Vec::new() })
    })
    .into_iter()
    .collect()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-ust-dev: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfgs = Vec::new();
    let (mut with_voc, mut diag, mut per_song) = (false, false, false);
    let mut dump: Option<PathBuf> = None;
    let mut split = "dev".to_string();
    let mut stem = "vocals".to_string();
    let (mut reason, mut reread): (Option<String>, bool) = (None, false);
    let mut pyin = PYin::default();
    let mut cand_diag = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cfg" => {
                i += 1;
                cfgs.push(parse_cfg(args.get(i).ok_or("--cfg needs a value")?)?);
            }
            "--vocadito" => with_voc = true,
            "--pyin" => {
                i += 1;
                let kv = args.get(i).ok_or("--pyin needs key=value")?;
                let (k, v) = kv.split_once('=').ok_or("--pyin needs key=value")?;
                let x: f64 = v.parse().map_err(|e| format!("--pyin {kv}: {e}"))?;
                match k {
                    "voiced_threshold" => pyin.voiced_threshold = x as f32,
                    "median_window" => pyin.median_window = x as usize,
                    "beta_b" => pyin.beta_b = x as f32,
                    "fmin" => pyin.fmin = x as f32,
                    _ => return Err(format!("unknown pyin knob {k}")),
                }
            }
            "--stem" => {
                i += 1;
                stem = args.get(i).ok_or("--stem needs vocals|lead")?.clone();
            }
            "--diag" => diag = true,
            "--cand-diag" => cand_diag = true,
            "--per-song" => per_song = true,
            "--dump" => {
                i += 1;
                dump = Some(PathBuf::from(args.get(i).ok_or("--dump needs a dir")?));
            }
            "--holdout-once" => split = "holdout".into(),
            "--split" => {
                i += 1;
                split = args.get(i).ok_or("--split needs a split name")?.clone();
            }
            "--reason" => {
                i += 1;
                reason = Some(args.get(i).ok_or("--reason needs text")?.clone());
            }
            "--reread" => reread = true,
            a => return Err(format!("unknown arg {a}")),
        }
        i += 1;
    }
    if cfgs.is_empty() {
        cfgs.push(parse_cfg("base:")?);
    }
    let params = NoteParams::default();
    if split != "dev" {
        let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
        sona_eval::holdout::authorize(&manifest, &split, "sona-eval-ust-dev", reason.as_deref(), reread)?;
    }
    let with_cands = cand_diag || cfgs.iter().any(|(_, c)| c.cell.candidates.is_some());
    let items = load_ust_items(&split, &pyin, &stem, with_cands).map_err(|e| e.to_string())?;
    if items.is_empty() {
        return Err(format!("no {split} songs have a {stem}.wav stem (for lead, run scripts/separate-lead.sh)"));
    }
    let voc = if with_voc { load_vocadito(&pyin).map_err(|e| e.to_string())? } else { Vec::new() };
    eprintln!("{} {split} songs ({stem} stems){}", items.len(), if with_voc { format!(" + {} Vocadito stems", voc.len()) } else { String::new() });
    if cand_diag {
        print_cand_oracle(&items);
    }

    println!(
        "{:<22} | {:^31} | {:^19} | {:^13} | {:^13}",
        "", "quantized, all notes", "raw, all", "quant, mono", "Vocadito raw"
    );
    println!(
        "{:<22} | {:>6} {:>6} {:>6} {:>9} | {:>9} {:>9} | {:>6} {:>6} | {:>6} {:>6}",
        "config", "P", "R", "F1", "COnPOff", "COnP F1", "COnPOff", "COnP", "Off", "COnP", "Off"
    );
    for (ci, (name, t)) in cfgs.iter().enumerate() {
        let scored = parallel_map(&items, |it| score_item(t, it, &params));
        let mut tot = [Counts::default(); 5];
        for (v, _) in &scored {
            for (k, c) in v.iter().enumerate() {
                tot[k].add(*c);
            }
        }
        let vtxt = if with_voc {
            let vs = parallel_map(&voc, |it| score_item(t, it, &params).0[0]);
            let n = vs.len() as f64;
            let mc = vs.iter().map(|c| c.conp().f1).sum::<f64>() / n;
            let mo = vs.iter().map(|c| c.conpoff().f1).sum::<f64>() / n;
            format!("{mc:>6.3} {mo:>6.3}")
        } else {
            format!("{:>6} {:>6}", "—", "—")
        };
        let q = tot[1].conp();
        println!(
            "{:<22} | {:>6.3} {:>6.3} {:>6.3} {:>9.3} | {:>9.3} {:>9.3} | {:>6.3} {:>6.3} | {vtxt}",
            name,
            q.precision,
            q.recall,
            q.f1,
            tot[1].conpoff().f1,
            tot[0].conp().f1,
            tot[0].conpoff().f1,
            tot[3].conp().f1,
            tot[3].conpoff().f1,
        );
        let c = tot[1].cells;
        let pc = |x: (usize, usize)| 100.0 * x.0 as f64 / x.1.max(1) as f64;
        println!(
            "{:<22}   cell accuracy (quantized, vs UST): all {:.1}% · mono {:.1}% · sung cells {:.1}% · sung mono {:.1}%",
            "", pc(c[0]), pc(c[1]), pc(c[2]), pc(c[3])
        );
        let b = tot[1].poly_cells;
        let n: usize = b.iter().sum();
        let f = |x: usize| 100.0 * x as f64 / n.max(1) as f64;
        println!(
            "{:<22}   poly sung cells (n={n}): right {:.1}% · rest {:.1}% · harmony {:.1}% · octave {:.1}% · other {:.1}%",
            "", f(b[0]), f(b[1]), f(b[2]), f(b[3]), f(b[4])
        );
        println!(
            "{:<22}   monophonic reference notes matched by the whole line (quantized): COnP {:.1}% · COnPOff {:.1}%",
            "",
            100.0 * tot[4].conp().recall,
            100.0 * tot[4].conpoff().recall
        );
        if per_song {
            for (it, (v, _)) in items.iter().zip(&scored) {
                let c = v[1].conp();
                println!(
                    "    {:<30} n={:<4} est={:<4} P {:.2} R {:.2} F1 {:.2} | COnPOff {:.2} | conp={} conpoff={} | mono n={} est={} conp={} conpoff={} | cells {}",
                    it.id,
                    v[1].n_ref,
                    v[1].n_est,
                    c.precision,
                    c.recall,
                    c.f1,
                    v[1].conpoff().f1,
                    v[1].conp,
                    v[1].conpoff,
                    v[3].n_ref,
                    v[3].n_est,
                    v[3].conp,
                    v[3].conpoff,
                    v[1].cells.iter().map(|(a, b)| format!("{a}/{b}")).collect::<Vec<_>>().join(" ")
                );
            }
        }
        if let (Some(dir), 0) = (&dump, ci) {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            for (it, (_, a)) in items.iter().zip(&scored) {
                let onset = t.t.onset.detect(&it.samples, it.sr);
                let j = serde_json::json!({
                    "refs": it.refs.iter().map(|r| (r.onset_s, r.offset_s, r.midi_pitch)).collect::<Vec<_>>(),
                    "poly": it.poly,
                    "est": a.notes.iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch)).collect::<Vec<_>>(),
                    "quant": it.grid.as_ref().map(|g| est(a, Some(&g.opts)).iter().map(|n| (n.onset_s, n.offset_s, n.midi_pitch)).collect::<Vec<_>>()),
                    "grid": it.grid.as_ref().map(|g| (g.opts.spacing, g.opts.anchor)),
                    "f0": it.contour.iter().map(|f| (f.time_s, f.frequency_hz, f.voicing)).collect::<Vec<_>>(),
                    "onsets": onset.onsets.iter().map(|o| (o.time_s, o.strength)).collect::<Vec<_>>(),
                    "rms": onset.energy_envelope.iter().map(|e| (e.time_s, e.rms)).collect::<Vec<_>>(),
                });
                let path = dir.join(format!("{}.json", it.id));
                std::fs::write(&path, j.to_string()).map_err(|e| e.to_string())?;
            }
        }
        if diag && ci == 0 {
            print_diag(&items, &scored, &params, t.t.segmenter.min_note_s);
        }
    }
    Ok(())
}

fn print_diag(items: &[Item], scored: &[(Vec<Counts>, Annotation)], p: &NoteParams, floor: f64) {
    let mut miss = std::collections::BTreeMap::<&str, usize>::new();
    let mut fp = std::collections::BTreeMap::<&str, usize>::new();
    let mut lag: Vec<f64> = Vec::new();
    let mut total_mono = 0;
    for (it, (_, a)) in items.iter().zip(scored) {
        let refs: Vec<RefNote> = it.refs.iter().copied().filter(|r| !overlaps_any(&it.poly, r.onset_s, r.offset_s)).collect();
        let est: Vec<EstNote> = est(a, None).into_iter().filter(|n| !overlaps_any(&it.poly, n.onset_s, n.offset_s)).collect();
        total_mono += refs.len();
        let m = match_notes(&refs, &est, p, false);
        let mut ref_hit = vec![false; refs.len()];
        let mut est_hit = vec![false; est.len()];
        for &(r, e) in &m {
            ref_hit[r] = true;
            est_hit[e] = true;
            lag.push(est[e].onset_s - refs[r].onset_s);
        }
        for (ri, r) in refs.iter().enumerate().filter(|(i, _)| !ref_hit[*i]) {
            let prev_same = ri > 0 && refs[ri - 1].midi_pitch == r.midi_pitch && (r.onset_s - refs[ri - 1].offset_s).abs() < 0.03;
            let near = |tol: f64| est.iter().filter(move |e| (e.onset_s - r.onset_s).abs() <= tol);
            let cat = if r.offset_s - r.onset_s < floor {
                "shorter than prune floor"
            } else if near(0.05).any(|e| (e.midi_pitch as i32 - r.midi_pitch as i32).abs() == 12) {
                "onset ok, octave error"
            } else if near(0.05).any(|e| e.midi_pitch != r.midi_pitch) {
                "onset ok, pitch off (non-octave)"
            } else if near(0.15).any(|e| e.midi_pitch == r.midi_pitch) {
                "right pitch, onset 50–150 ms off"
            } else if est.iter().any(|e| e.onset_s < r.onset_s && e.offset_s > r.onset_s + 0.02 && e.midi_pitch == r.midi_pitch) {
                if prev_same { "merged into previous same-pitch note" } else { "covered by earlier same-pitch note" }
            } else if est.iter().any(|e| e.onset_s < r.offset_s && e.offset_s > r.onset_s) {
                "overlapped by other-pitch note"
            } else {
                "no estimated note at all"
            };
            *miss.entry(cat).or_default() += 1;
        }
        for (_, e) in est.iter().enumerate().filter(|(i, _)| !est_hit[*i]) {
            let cat = if refs.iter().any(|r| r.onset_s < e.offset_s && r.offset_s > e.onset_s) {
                "overlaps a reference note"
            } else {
                "in a reference rest (spurious)"
            };
            *fp.entry(cat).or_default() += 1;
        }
    }
    let nmiss: usize = miss.values().sum();
    println!("\n  diag (raw, monophonic): {nmiss} of {total_mono} reference notes missed");
    let mut mv: Vec<_> = miss.into_iter().collect();
    mv.sort_by(|a, b| b.1.cmp(&a.1));
    for (k, v) in mv {
        println!("    {:<40} {:>5}  ({:.1}% of mono refs)", k, v, 100.0 * v as f64 / total_mono as f64);
    }
    println!("  unmatched estimates:");
    for (k, v) in fp {
        println!("    {k:<40} {v:>5}");
    }
    lag.sort_by(f64::total_cmp);
    if !lag.is_empty() {
        let q = |x: f64| 1000.0 * lag[((lag.len() - 1) as f64 * x).round() as usize];
        println!("  matched onset error (est − ref, ms): p10 {:+.0} p50 {:+.0} p90 {:+.0}", q(0.1), q(0.5), q(0.9));
    }
}


/// Oracle for multi-candidate evidence: over sung lead frames, how often the
/// lead's pitch is pYIN's f0, or among a frame's top-k candidates, split
/// monophonic / polyphonic.
fn print_cand_oracle(items: &[Item]) {
    // [mono, poly] x [pYIN f0 right, top-1, top-2, top-3, top-5, frames]
    let mut acc = [[0usize; 6]; 2];
    for it in items {
        let Some(cands) = &it.cands else { continue };
        let semi = |hz: f32| 69.0 + 12.0 * (hz / 440.0).log2();
        let (mut x, mut y) = (0.0f64, 0.0f64);
        for f in it.contour.iter().filter(|f| f.frequency_hz > 0.0) {
            let v = semi(f.frequency_hz);
            let a = std::f64::consts::TAU * (v - v.round()) as f64;
            x += a.cos();
            y += a.sin();
        }
        let tune = (y.atan2(x) / std::f64::consts::TAU) as f32;
        let near = |hz: f32, r: u8| hz > 0.0 && (semi(hz) - tune - r as f32).abs() < 0.5;
        for (fi, f) in it.contour.iter().enumerate() {
            let t = f.time_s;
            let Some(r) = it.refs.iter().find(|n| n.onset_s <= t && t < n.offset_s).map(|n| n.midi_pitch) else { continue };
            let m = overlaps_any(&it.poly, t, t + 1e-3) as usize;
            let c = &cands[fi];
            let a = &mut acc[m];
            a[0] += near(f.frequency_hz, r) as usize;
            for (slot, k) in [(1usize, 1usize), (2, 2), (3, 3), (4, 5)] {
                a[slot] += c.iter().take(k).any(|c| near(c.frequency_hz, r)) as usize;
            }
            a[5] += 1;
        }
    }
    for (m, name) in [(0, "mono"), (1, "poly")] {
        let a = acc[m];
        let pc = |x: usize| 100.0 * x as f64 / a[5].max(1) as f64;
        println!(
            "  cand oracle, {name} sung frames (n={}): pYIN f0 right {:.1}% · cand top1 {:.1}% · top2 {:.1}% · top3 {:.1}% · top5 {:.1}%",
            a[5], pc(a[0]), pc(a[1]), pc(a[2]), pc(a[3]), pc(a[4])
        );
    }
}
