//! **Dev-split iteration harness** for the in-domain UST corpus: score one or
//! more segmentation configs on the manifest's `dev` songs (never `holdout`),
//! with Vocadito as a no-regression guard.
//!
//!     cargo run --release -p sona-eval --bin sona-eval-ust-dev -- \
//!         --cfg base: --cfg floor50:min_note_s=0.05 [--vocadito] [--diag]
//!
//! pYIN is the cost, and it does not depend on any segmentation knob, so each
//! stem's f0 contour is cached under `target/ust-dev-cache/` (keyed by the
//! tracker's settings). A config is `name:key=value,key=value`; see [`apply`]
//! for the keys. `--diag` prints where the first config's misses go.
//!
//! Scoring matches `sona-eval-ust-gate` exactly (same views, same quantizer,
//! same anchor), so a dev number here is the gate's dev number.

use std::path::PathBuf;
use std::process::ExitCode;

use sona_core::{Annotation, F0Frame, GridDecoder, GridSpec, PYin, PitchTracker, Producer, Transcriber};
use sona_eval::estimate::EstNote;
use sona_eval::metrics::note::{match_notes, NoteParams};
use sona_eval::metrics::Prf1;
use sona_eval::quantize::{grid_spacing_seconds, quantize_note, QuantizeOptions};
use sona_eval::reference::RefNote;
use sona_eval::ust_corpus::{manifest_path, overlaps_any, stems_dir, Manifest, SongUsts};
use sona_eval::{ust, vocadito, wav, EvalError};

/// One stem with everything segmentation needs, loaded once.
struct Item {
    id: String,
    samples: Vec<f32>,
    sr: u32,
    contour: Vec<F0Frame>,
    /// `None` for Vocadito (raw only, no grid).
    grid: Option<Grid>,
    refs: Vec<RefNote>,
    poly: Vec<(f64, f64)>,
}

struct Grid {
    opts: QuantizeOptions,
    bpm: f64,
    division: u32,
}

/// How a config decodes: the note-HMM, or the cell decoder on each song's grid
/// with an estimated phase (what a user gets) or the UST-derived one (an upper bound).
#[derive(Clone, Copy, PartialEq)]
enum GridChoice {
    Off,
    Estimated,
    UstAnchor,
}

fn cache_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ust-dev-cache")
}

/// The tracker's settings as a cache key, so a PYin change can't read a stale contour.
fn tracker_key(p: &PYin) -> String {
    let s = format!("{p:?}");
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn contour_cached(id: &str, samples: &[f32], sr: u32, pyin: &PYin) -> Result<Vec<F0Frame>, EvalError> {
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
    std::fs::write(&path, text).map_err(|e| EvalError::io(&path, e))?;
    Ok(c)
}

/// A config: the transcriber's stages plus how (and whether) to decode on a grid.
#[derive(Clone)]
struct Cfg {
    t: Transcriber,
    grid: GridChoice,
    cell: GridDecoder,
}

/// Set one `key=value` knob on a config.
fn apply(cfg: &mut Cfg, key: &str, val: &str) -> Result<(), String> {
    let f = || val.parse::<f64>().map_err(|e| format!("{key}={val}: {e}"));
    let g = &mut cfg.cell;
    match key {
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
    let mut cfg = Cfg { t: Transcriber::default(), grid: GridChoice::Off, cell: GridDecoder::default() };
    for kv in rest.split(',').filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once('=').ok_or_else(|| format!("bad knob {kv}"))?;
        apply(&mut cfg, k, v)?;
    }
    Ok((name.to_string(), cfg))
}

fn transcribe(cfg: &Cfg, it: &Item) -> Annotation {
    // The pipeline, with the cached contour standing in for `tracker.track`.
    let t = &cfg.t;
    let onset = t.onset.detect(&it.samples, it.sr);
    let segs = match (&it.grid, cfg.grid) {
        (Some(g), GridChoice::Estimated | GridChoice::UstAnchor) => {
            let anchor = (cfg.grid == GridChoice::UstAnchor).then_some(g.opts.anchor);
            cfg.cell.segment(&GridSpec { bpm: g.bpm, division: g.division, anchor_s: anchor }, &onset.onsets, &it.contour, &onset.energy_envelope)
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
    }
}

fn est(a: &Annotation, q: Option<&QuantizeOptions>) -> Vec<EstNote> {
    a.notes
        .iter()
        .map(|n| match q {
            None => EstNote { onset_s: n.onset_s, offset_s: n.offset_s, midi_pitch: n.midi_pitch, confidence: n.confidence },
            Some(o) => {
                let qn = quantize_note(n.onset_s, n.offset_s, Some(n.sub_scores.onset_clarity), o);
                EstNote { onset_s: qn.onset_s, offset_s: qn.offset_s, midi_pitch: n.midi_pitch, confidence: n.confidence }
            }
        })
        .collect()
}

#[derive(Default, Clone, Copy)]
struct Counts {
    n_ref: usize,
    n_est: usize,
    conp: usize,
    conpoff: usize,
}

impl Counts {
    fn of(refs: &[RefNote], est: &[EstNote], p: &NoteParams) -> Self {
        Counts { n_ref: refs.len(), n_est: est.len(), conp: match_notes(refs, est, p, false).len(), conpoff: match_notes(refs, est, p, true).len() }
    }
    fn add(&mut self, o: Counts) {
        self.n_ref += o.n_ref;
        self.n_est += o.n_est;
        self.conp += o.conp;
        self.conpoff += o.conpoff;
    }
    fn conp(&self) -> Prf1 {
        Prf1::from_counts(self.conp, self.n_ref, self.n_est)
    }
    fn conpoff(&self) -> Prf1 {
        Prf1::from_counts(self.conpoff, self.n_ref, self.n_est)
    }
}

/// [raw-all, quant-all, raw-mono, quant-mono] for a UST song; [raw] for Vocadito.
fn score_item(t: &Cfg, it: &Item, p: &NoteParams) -> (Vec<Counts>, Annotation) {
    let a = transcribe(t, it);
    let raw = est(&a, None);
    let Some(g) = &it.grid else {
        return (vec![Counts::of(&it.refs, &raw, p)], a);
    };
    let quant = est(&a, Some(&g.opts));
    let mono_refs: Vec<RefNote> = it.refs.iter().copied().filter(|r| !overlaps_any(&it.poly, r.onset_s, r.offset_s)).collect();
    let mono = |v: &[EstNote]| -> Vec<EstNote> { v.iter().copied().filter(|n| !overlaps_any(&it.poly, n.onset_s, n.offset_s)).collect() };
    (
        vec![
            Counts::of(&it.refs, &raw, p),
            Counts::of(&it.refs, &quant, p),
            Counts::of(&mono_refs, &mono(&raw), p),
            Counts::of(&mono_refs, &mono(&quant), p),
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

fn load_ust_items(split: &str, pyin: &PYin) -> Result<Vec<Item>, EvalError> {
    let manifest = Manifest::load(&manifest_path())?;
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let songs: Vec<_> = manifest
        .songs
        .iter()
        .filter(|s| s.split.as_deref() == Some(split) && s.stratum == "vocaloid-original")
        .filter(|s| s.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) && s.alignment_offset_s.is_some())
        .filter(|s| stems.join(&s.id).join("vocals.wav").is_file())
        .collect();
    let items: Vec<Result<Item, EvalError>> = parallel_map(&songs, |song| {
        let offset = song.alignment_offset_s.expect("filtered");
        let usts = SongUsts::load(song, &cache)?;
        let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
        let spacing = grid_spacing_seconds(bpm, song.grid_division.unwrap_or(16) as f64);
        let (samples, sr) = wav::read_mono(&stems.join(&song.id).join("vocals.wav"))?;
        let contour = contour_cached(&song.id, &samples, sr, pyin)?;
        Ok(Item {
            id: song.id.clone(),
            samples,
            sr,
            contour,
            grid: Some(Grid { opts: QuantizeOptions::new(spacing, offset), bpm, division: song.grid_division.unwrap_or(16) }),
            refs: usts.reference(offset).notes,
            poly: usts.polyphonic_spans(offset),
        })
    });
    items.into_iter().collect()
}

fn load_vocadito(pyin: &PYin) -> Result<Vec<Item>, EvalError> {
    let corpus = vocadito::discover(&vocadito::corpus_dir())?;
    parallel_map(&corpus.tracks, |track| {
        let mut refs = vocadito::load_reference(track)?.notes;
        refs.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s));
        let (samples, sr) = wav::read_mono(&track.audio)?;
        let id = format!("vocadito-{}", track.audio.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        let contour = contour_cached(&id, &samples, sr, pyin)?;
        Ok(Item { id, samples, sr, contour, grid: None, refs, poly: Vec::new() })
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
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cfg" => {
                i += 1;
                cfgs.push(parse_cfg(args.get(i).ok_or("--cfg needs a value")?)?);
            }
            "--vocadito" => with_voc = true,
            "--diag" => diag = true,
            "--per-song" => per_song = true,
            "--dump" => {
                i += 1;
                dump = Some(PathBuf::from(args.get(i).ok_or("--dump needs a dir")?));
            }
            // Reading the holdout is a one-time act (see the manifest's split rule).
            "--holdout-once" => split = "holdout".into(),
            a => return Err(format!("unknown arg {a}")),
        }
        i += 1;
    }
    if cfgs.is_empty() {
        cfgs.push(parse_cfg("base:")?);
    }
    let pyin = PYin::default();
    let params = NoteParams::default();
    let items = load_ust_items(&split, &pyin).map_err(|e| e.to_string())?;
    let voc = if with_voc { load_vocadito(&pyin).map_err(|e| e.to_string())? } else { Vec::new() };
    eprintln!("{} {split} songs{}", items.len(), if with_voc { format!(" + {} Vocadito stems", voc.len()) } else { String::new() });

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
        let mut tot = [Counts::default(); 4];
        for (v, _) in &scored {
            for k in 0..4 {
                tot[k].add(v[k]);
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
        if per_song {
            for (it, (v, _)) in items.iter().zip(&scored) {
                let c = v[1].conp();
                println!(
                    "    {:<30} n={:<4} est={:<4} P {:.2} R {:.2} F1 {:.2} | COnPOff {:.2} | conp={} conpoff={}",
                    it.id, v[1].n_ref, v[1].n_est, c.precision, c.recall, c.f1, v[1].conpoff().f1, v[1].conp, v[1].conpoff
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

/// Where the reference notes the raw output misses go (monophonic regions only,
/// so harmony bleed doesn't pollute the taxonomy), and what the unmatched
/// estimates are.
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

