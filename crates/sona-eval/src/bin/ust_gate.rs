//! **The in-domain gate** (UST corpus gate, ticket 06): sona vs. UST ground truth
//! on the songs it targets, quantized and raw, per stratum — never pooled across.
//!
//!     cargo run --release -p sona-eval --bin sona-eval-ust-gate               # UST corpus, dev split
//!     cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --include-split holdout2 --reason "…"  # + a held-out split (logged)
//!     cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --vocadito # + baseline
//!     cargo run --release -p sona-eval --bin sona-eval-ust-gate -- --grid     # cell decoder (ADR-0015)
//!
//! Only songs whose `alignment_status` starts with `aligned` are scored; the rest
//! are listed with their reason (no silent truncation).
//!
//! Views: {raw, quantized} × {all lead notes, monophonic regions}. *Quantized* =
//! the review surface's hard-snap (`quantize.rs`, ported from `grid.ts`) on the
//! song's own grid (16th or 16th-triplet), anchored at the UST-derived offset.
//! *Monophonic* drops every reference and estimated note that overlaps another
//! voice (harmony, doubling, duet) — the stem holds those voices too, and sona is
//! monophonic.
//!
//! **The offset number** (per ticket 01): only ~14% of UST notes end in a rest,
//! and a legato note's UST offset is just the next onset. So offsets are read as
//! *offset accuracy given a correct onset + pitch* (COnP-matched pairs whose
//! offset is within the COnPOff tolerance), split rest-terminated vs. legato.
//! Vocadito gets the same conditional measure (rest-terminated = followed by a
//! gap > 50 ms, or last).
//!
//! F1 is reported two ways: **mean over songs** (how the Vocadito baseline was
//! recorded) and **pooled** (note-weighted counts).

use std::path::Path;
use std::process::ExitCode;

use sona_core::{Annotation, GridMode, Producer, PYin, Transcriber};
use sona_eval::estimate::EstNote;
use sona_eval::metrics::note::{match_notes, NoteParams};
use sona_eval::metrics::Prf1;
use sona_eval::quantize::{grid_spacing_seconds, quantize_note, resolve_overlaps, QuantizeOptions};
use sona_eval::reference::RefNote;
use sona_eval::ust_corpus::{manifest_path, overlaps_any, stems_dir, Manifest, Song, SongUsts};
use sona_eval::{ust, vocadito, wav, EvalError};

/// Below this a note can't be emitted: the note-HMM's prune floor (ADR-0010).
fn prune_s() -> f64 {
    sona_core::NoteHmm::default().min_note_s
}

/// Matched / reference / estimate counts for one view.
#[derive(Debug, Default, Clone, Copy)]
struct Counts {
    n_ref: usize,
    n_est: usize,
    conp: usize,
    conpoff: usize,
}

impl Counts {
    fn add(&mut self, o: &Counts) {
        self.n_ref += o.n_ref;
        self.n_est += o.n_est;
        self.conp += o.conp;
        self.conpoff += o.conpoff;
    }
    fn conp_f1(&self) -> f64 {
        Prf1::from_counts(self.conp, self.n_ref, self.n_est).f1
    }
    fn conpoff_f1(&self) -> f64 {
        Prf1::from_counts(self.conpoff, self.n_ref, self.n_est).f1
    }
}

/// Offset accuracy given a correct onset + pitch, split by note kind, plus the
/// quantized offset error in grid cells for rest-terminated pairs.
#[derive(Debug, Default, Clone, Copy)]
struct Offsets {
    rt_pairs: usize,
    rt_ok: usize,
    legato_pairs: usize,
    legato_ok: usize,
    /// Rest-terminated pairs by |offset error| in cells: 0, 1, ≥2 (quantized only).
    cells: [usize; 3],
}

impl Offsets {
    fn add(&mut self, o: &Offsets) {
        self.rt_pairs += o.rt_pairs;
        self.rt_ok += o.rt_ok;
        self.legato_pairs += o.legato_pairs;
        self.legato_ok += o.legato_ok;
        for i in 0..3 {
            self.cells[i] += o.cells[i];
        }
    }
}

/// One scored view.
#[derive(Debug, Default, Clone, Copy)]
struct View {
    counts: Counts,
    offsets: Offsets,
}

fn score(refs: &[RefNote], rest_term: &[bool], est: &[EstNote], params: &NoteParams, spacing: Option<f64>) -> View {
    let counts = Counts {
        n_ref: refs.len(),
        n_est: est.len(),
        conp: match_notes(refs, est, params, false).len(),
        conpoff: match_notes(refs, est, params, true).len(),
    };
    let mut offsets = Offsets::default();
    for (r, e) in match_notes(refs, est, params, false) {
        let (rn, en) = (refs[r], est[e]);
        let tol = (params.offset_ratio * (rn.offset_s - rn.onset_s)).max(params.offset_min_tolerance_s);
        let err = (en.offset_s - rn.offset_s).abs();
        let ok = (err * 1e4).round() / 1e4 <= tol;
        if rest_term[r] {
            offsets.rt_pairs += 1;
            offsets.rt_ok += ok as usize;
            if let Some(sp) = spacing {
                offsets.cells[((err / sp).round() as usize).min(2)] += 1;
            }
        } else {
            offsets.legato_pairs += 1;
            offsets.legato_ok += ok as usize;
        }
    }
    View { counts, offsets }
}

/// Pitch sanity check: match on onset alone (±50 ms, pitch ignored) and
/// histogram `estimated − reference` semitones. A UST re-keyed for the cover's
/// voicebank shows up as a dominant non-zero interval; octave errors as ±12.
fn pitch_offsets(refs: &[RefNote], est: &[EstNote]) -> Vec<(i32, usize)> {
    let adjacency: Vec<Vec<usize>> = refs
        .iter()
        .map(|r| est.iter().enumerate().filter(|(_, e)| (e.onset_s - r.onset_s).abs() <= 0.05).map(|(j, _)| j).collect())
        .collect();
    let mut hist = std::collections::BTreeMap::<i32, usize>::new();
    for (r, e) in sona_eval::metrics::matching::maximum_matching(&adjacency) {
        *hist.entry(est[e].midi_pitch as i32 - refs[r].midi_pitch as i32).or_default() += 1;
    }
    let mut v: Vec<(i32, usize)> = hist.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v
}

fn transcribe(samples: &[f32], sr: u32, bpm: f64, grid: Option<u32>) -> Annotation {
    let t = Transcriber { grid: grid.map(|division| GridMode { division, ..GridMode::sixteenths() }), ..Transcriber::default() };
    t.transcribe(&PYin::default(), samples, sr, bpm, Producer::Pyin)
}

fn raw_notes(a: &Annotation) -> Vec<EstNote> {
    a.notes
        .iter()
        .map(|n| EstNote { onset_s: n.onset_s, offset_s: n.offset_s, midi_pitch: n.midi_pitch, confidence: n.confidence })
        .collect()
}

fn quantized_notes(a: &Annotation, opts: &QuantizeOptions) -> Vec<EstNote> {
    let mut q: Vec<_> = a.notes.iter().map(|n| quantize_note(n.onset_s, n.offset_s, Some(n.sub_scores.onset_clarity), opts)).collect();
    resolve_overlaps(&mut q, opts.spacing);
    a.notes
        .iter()
        .zip(q)
        .map(|(n, q)| EstNote { onset_s: q.onset_s, offset_s: q.offset_s, midi_pitch: n.midi_pitch, confidence: n.confidence })
        .collect()
}

/// Per-song results: [raw-all, quant-all, raw-mono, quant-mono].
struct SongResult {
    id: String,
    stratum: String,
    /// `dev` or `holdout` (see the manifest's split rule).
    split: String,
    grid: u32,
    views: [View; 4],
    mono_share: f64,
    below_prune: f64,
    /// Signed raw offset errors (estimate − reference, seconds) on COnP-matched,
    /// rest-terminated, monophonic pairs: which way sona's offsets miss.
    rt_offset_err: Vec<f64>,
}

/// Map `f` over `items` on a bounded pool of scoped threads (one per core),
/// returning results in input order.
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
                out.lock().expect("no worker panicked holding the lock")[i] = Some(r);
            });
        }
    });
    slots.into_iter().map(|r| r.expect("every item was processed")).collect()
}

/// Transcribe one aligned song and score every view.
fn score_song(song: &Song, offset: f64, cache: &Path, stems: &Path, params: &NoteParams, grid_mode: bool) -> Result<SongResult, EvalError> {
    eprintln!("transcribing {}…", song.id);
    let usts = SongUsts::load(song, cache)?;
    let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
    let grid = song.grid_division.unwrap_or(16);
    let spacing = grid_spacing_seconds(bpm, grid as f64);
    let opts = QuantizeOptions::new(spacing, offset);

    let (samples, sr) = wav::read_mono(&stems.join(&song.id).join("vocals.wav"))?;
    let annotation = transcribe(&samples, sr, bpm, grid_mode.then_some(grid));
    let (raw, quant) = (raw_notes(&annotation), quantized_notes(&annotation, &opts));

    let reference = usts.reference(offset);
    let poly = usts.polyphonic_spans(offset);
    let mono_ref: Vec<usize> =
        (0..reference.notes.len()).filter(|&i| !overlaps_any(&poly, reference.notes[i].onset_s, reference.notes[i].offset_s)).collect();
    let mono_refs: Vec<RefNote> = mono_ref.iter().map(|&i| reference.notes[i]).collect();
    let mono_rt: Vec<bool> = mono_ref.iter().map(|&i| reference.rest_terminated[i]).collect();
    let mono_est = |v: &[EstNote]| -> Vec<EstNote> { v.iter().copied().filter(|n| !overlaps_any(&poly, n.onset_s, n.offset_s)).collect() };

    let po = pitch_offsets(&reference.notes, &raw);
    let total: usize = po.iter().map(|p| p.1).sum();
    eprintln!(
        "  pitch check {}: est−ref semitones over {} onset matches: {}",
        song.id,
        total,
        po.iter().take(4).map(|(d, n)| format!("{d:+}: {:.0}%", 100.0 * *n as f64 / total.max(1) as f64)).collect::<Vec<_>>().join(", ")
    );
    let views = [
        score(&reference.notes, &reference.rest_terminated, &raw, params, None),
        score(&reference.notes, &reference.rest_terminated, &quant, params, Some(spacing)),
        score(&mono_refs, &mono_rt, &mono_est(&raw), params, None),
        score(&mono_refs, &mono_rt, &mono_est(&quant), params, Some(spacing)),
    ];
    let mono_raw = mono_est(&raw);
    let rt_offset_err: Vec<f64> = match_notes(&mono_refs, &mono_raw, params, false)
        .into_iter()
        .filter(|&(r, _)| mono_rt[r])
        .map(|(r, e)| mono_raw[e].offset_s - mono_refs[r].offset_s)
        .collect();
    let n = reference.notes.len().max(1) as f64;
    Ok(SongResult {
        rt_offset_err,
        id: song.id.clone(),
        stratum: song.stratum.clone(),
        split: song.split.clone().unwrap_or_default(),
        grid,
        views,
        mono_share: mono_refs.len() as f64 / n,
        below_prune: reference.notes.iter().filter(|r| r.offset_s - r.onset_s < prune_s()).count() as f64 / n,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let with_vocadito = args.iter().any(|a| a == "--vocadito");
    let grid_mode = args.iter().any(|a| a == "--grid");
    let named = args.iter().position(|a| a == "--include-split").and_then(|i| args.get(i + 1)).cloned();
    let split = named.or_else(|| args.iter().any(|a| a == "--include-holdout").then(|| "holdout".to_string()));
    let holdout = split.map(|split| HoldoutRead {
        split,
        reason: args.iter().position(|a| a == "--reason").and_then(|i| args.get(i + 1)).cloned(),
        reread: args.iter().any(|a| a == "--reread"),
    });
    match run(with_vocadito, grid_mode, holdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-ust-gate: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `--include-split NAME --reason "…" [--reread]` (`--include-holdout` = `--include-split holdout`):
/// also score that held-out split. Logged in the holdout ledger.
struct HoldoutRead {
    split: String,
    reason: Option<String>,
    reread: bool,
}

fn run(with_vocadito: bool, grid_mode: bool, holdout: Option<HoldoutRead>) -> Result<(), EvalError> {
    if grid_mode {
        println!("Mode: grid-conditioned cell decoder (ADR-0015), phase estimated from each stem. Vocadito stays on the default path (no grid).\n");
    }
    let manifest = Manifest::load(&manifest_path())?;
    if let Some(h) = &holdout {
        sona_eval::holdout::authorize(&manifest, &h.split, "sona-eval-ust-gate", h.reason.as_deref(), h.reread)
            .map_err(|e| EvalError::parse(sona_eval::holdout::ledger_path(), e))?;
    } else {
        println!("Scoring the dev split only (add --include-split NAME --reason \"…\" to read a held-out split; it is logged).\n");
    }
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let params = NoteParams::default();
    let mut dropped: Vec<(String, String)> = Vec::new();

    // Serial: decide eligibility (no silent truncation). Parallel: transcribe +
    // score, one song per worker — songs are independent and pYIN is the cost.
    let mut eligible: Vec<(&Song, f64)> = Vec::new();
    for song in &manifest.songs {
        // Dev always; a held-out split only when asked for (and logged above).
        let wanted = song.split.as_deref() == Some("dev") || holdout.as_ref().is_some_and(|h| song.split.as_deref() == Some(h.split.as_str()));
        if !wanted {
            continue;
        }
        let status = song.alignment_status.clone().unwrap_or_else(|| "not aligned".into());
        let (Some(offset), true) = (song.alignment_offset_s, status.starts_with("aligned")) else {
            dropped.push((song.id.clone(), status));
            continue;
        };
        if !stems.join(&song.id).join("vocals.wav").is_file() {
            dropped.push((song.id.clone(), "no vocal stem".into()));
            continue;
        }
        eligible.push((song, offset));
    }
    let results: Vec<SongResult> = parallel_map(&eligible, |&(song, offset)| score_song(song, offset, &cache, &stems, &params, grid_mode))
        .into_iter()
        .collect::<Result<_, _>>()?;

    print_songs(&results);
    for stratum in ["vocaloid-original", "human-sung"] {
        print_stratum(stratum, &results, grid_mode);
    }
    if !dropped.is_empty() {
        println!("\nNot scored (excluded by 05, never silently):");
        for (id, why) in &dropped {
            println!("  {id}: {why}");
        }
    }
    if with_vocadito {
        print_vocadito(&params)?;
    }
    Ok(())
}

fn pct(num: usize, den: usize) -> String {
    if den == 0 { "  —".into() } else { format!("{:>3.0}%", 100.0 * num as f64 / den as f64) }
}

fn print_songs(results: &[SongResult]) {
    println!("== Per song (COnP / COnPOff F1; off|rt = offset accuracy on rest-terminated COnP pairs) ==\n");
    println!(
        "{:<32} {:<5} {:>5} {:>5}   {:^23}   {:^23}   {:>5} {:>6}",
        "song", "grid", "notes", "mono", "ALL: raw → quantized", "MONO: raw → quantized", "<100", "ceil"
    );
    println!("{:<50}  {:^23}   {:^23}", "", "COnP  COnPOff  off|rt", "COnP  COnPOff  off|rt");
    for r in results {
        let cell = |raw: &View, q: &View| {
            format!(
                "{:.2}→{:.2} {:.2}→{:.2} {}→{}",
                raw.counts.conp_f1(),
                q.counts.conp_f1(),
                raw.counts.conpoff_f1(),
                q.counts.conpoff_f1(),
                pct(raw.offsets.rt_ok, raw.offsets.rt_pairs).trim(),
                pct(q.offsets.rt_ok, q.offsets.rt_pairs).trim()
            )
        };
        println!(
            "{:<32} {:<5} {:>5} {:>4.0}%   {:<27} {:<27} {:>4.0}% {:>5.0}%{}",
            r.id,
            if r.grid == 24 { "trip" } else { "16th" },
            r.views[0].counts.n_ref,
            r.mono_share * 100.0,
            cell(&r.views[0], &r.views[1]),
            cell(&r.views[2], &r.views[3]),
            r.below_prune * 100.0,
            (1.0 - r.below_prune) * 100.0,
            if r.stratum == "human-sung" { "  (human-sung)" } else { "" }
        );
    }
}

fn print_stratum(stratum: &str, results: &[SongResult], grid_mode: bool) {
    let rs: Vec<&SongResult> = results.iter().filter(|r| r.stratum == stratum).collect();
    if rs.is_empty() {
        return;
    }
    println!("\n== Stratum: {stratum} ({} songs{}) ==", rs.len(), if stratum == "human-sung" { " — WATCHED, never tuned against" } else { " — THE GATE" });
    let names = ["raw, all notes", "quantized, all notes", "raw, monophonic", "quantized, monophonic"];
    println!(
        "\n{:<23} {:>12} {:>15}   {:>11} {:>14} {:>9} {:>9}   {:>16} {:>16}",
        "view", "COnP F1 mean", "COnPOff F1 mean", "COnP pooled", "COnPOff pooled", "COnP P", "COnP R", "off|rest-term", "off|legato"
    );
    for (v, name) in names.iter().enumerate() {
        let mut counts = Counts::default();
        let mut offs = Offsets::default();
        for r in &rs {
            counts.add(&r.views[v].counts);
            offs.add(&r.views[v].offsets);
        }
        let mean = |f: &dyn Fn(&Counts) -> f64| rs.iter().map(|r| f(&r.views[v].counts)).sum::<f64>() / rs.len() as f64;
        let prf = Prf1::from_counts(counts.conp, counts.n_ref, counts.n_est);
        println!(
            "{:<23} {:>12.3} {:>15.3}   {:>11.3} {:>14.3} {:>9.3} {:>9.3}   {:>9} (n={:<4}) {:>8} (n={:<4})",
            name,
            mean(&|c| c.conp_f1()),
            mean(&|c| c.conpoff_f1()),
            counts.conp_f1(),
            counts.conpoff_f1(),
            prf.precision,
            prf.recall,
            pct(offs.rt_ok, offs.rt_pairs),
            offs.rt_pairs,
            pct(offs.legato_ok, offs.legato_pairs),
            offs.legato_pairs
        );
        if v % 2 == 1 {
            let c = offs.cells;
            let total = (c[0] + c[1] + c[2]).max(1);
            println!(
                "{:<23}   quantized rest-terminated offset error: exact {} · 1 cell {} · ≥2 cells {}   (n={})",
                "",
                pct(c[0], total),
                pct(c[1], total),
                pct(c[2], total),
                total
            );
        }
    }
    let mut errs: Vec<f64> = rs.iter().flat_map(|r| r.rt_offset_err.iter().copied()).collect();
    errs.sort_by(f64::total_cmp);
    if !errs.is_empty() {
        let q = |p: f64| errs[((errs.len() - 1) as f64 * p).round() as usize] * 1000.0;
        let early = errs.iter().filter(|&&e| e < -0.05).count();
        let late = errs.iter().filter(|&&e| e > 0.05).count();
        println!(
            "\n  raw offset error, rest-terminated mono pairs (est − ref, ms; n={}): p10 {:+.0} · p25 {:+.0} · p50 {:+.0} · p75 {:+.0} · p90 {:+.0}   |   >50 ms early {} · >50 ms late {}",
            errs.len(), q(0.1), q(0.25), q(0.5), q(0.75), q(0.9), pct(early, errs.len()), pct(late, errs.len())
        );
    }
    print_bootstrap(&rs);
    // Per split: iterate on `dev` only; `holdout` is read once, at the end.
    for split in ["dev", "holdout", "holdout2", "holdout3"] {
        let part: Vec<&&SongResult> = rs.iter().filter(|r| r.split == split).collect();
        if part.is_empty() {
            continue;
        }
        let (mut c, mut o) = (Counts::default(), Offsets::default());
        for r in &part {
            c.add(&r.views[3].counts);
            o.add(&r.views[3].offsets);
        }
        println!(
            "  split {split:<8} ({:>2} songs)  quantized mono: off|rest-term {} (n={})  COnPOff pooled {:.3}  COnP pooled {:.3}",
            part.len(),
            pct(o.rt_ok, o.rt_pairs),
            o.rt_pairs,
            c.conpoff_f1(),
            c.conp_f1()
        );
    }
    let notes: usize = rs.iter().map(|r| r.views[0].counts.n_ref).sum();
    let below: f64 = rs.iter().map(|r| r.below_prune * r.views[0].counts.n_ref as f64).sum();
    if grid_mode {
        println!("\n{notes} reference notes; the cell decoder has no prune floor (a note is ≥ 1 cell), so the <100 / ceil columns describe the note-HMM only");
        return;
    }
    println!(
        "\n{notes} reference notes; {:.1}% are shorter than the {:.0} ms prune floor (a recall ceiling of {:.1}%)",
        100.0 * below / notes.max(1) as f64,
        prune_s() * 1000.0,
        100.0 - 100.0 * below / notes.max(1) as f64
    );
}

/// Resample songs with replacement and report 95% percentile intervals for the
/// pooled headline numbers. sona is deterministic, so song sampling is the noise
/// that matters (pstack principle-explain-the-number: state the noise floor).
fn print_bootstrap(rs: &[&SongResult]) {
    const ITERS: usize = 2000;
    if rs.len() < 3 {
        return;
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15; // fixed seed: reproducible intervals
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    // (label, statistic over a resampled set)
    type Stat = fn(&[&SongResult]) -> f64;
    let stats: [(&str, Stat); 4] = [
        ("off|rest-term, quantized mono", |s| {
            let (ok, n) = s.iter().fold((0, 0), |(a, b), r| (a + r.views[3].offsets.rt_ok, b + r.views[3].offsets.rt_pairs));
            if n == 0 { f64::NAN } else { ok as f64 / n as f64 }
        }),
        ("off|rest-term, raw mono", |s| {
            let (ok, n) = s.iter().fold((0, 0), |(a, b), r| (a + r.views[2].offsets.rt_ok, b + r.views[2].offsets.rt_pairs));
            if n == 0 { f64::NAN } else { ok as f64 / n as f64 }
        }),
        ("COnPOff pooled, quantized mono", |s| {
            let mut c = Counts::default();
            s.iter().for_each(|r| c.add(&r.views[3].counts));
            c.conpoff_f1()
        }),
        ("COnPOff pooled, quantized all", |s| {
            let mut c = Counts::default();
            s.iter().for_each(|r| c.add(&r.views[1].counts));
            c.conpoff_f1()
        }),
    ];
    println!("\n  95% bootstrap intervals ({ITERS} song resamples):");
    for (label, f) in stats {
        let point = f(rs);
        let mut draws: Vec<f64> = (0..ITERS)
            .map(|_| {
                let sample: Vec<&SongResult> = (0..rs.len()).map(|_| rs[(next() % rs.len() as u64) as usize]).collect();
                f(&sample)
            })
            .filter(|x| x.is_finite())
            .collect();
        draws.sort_by(f64::total_cmp);
        let q = |p: f64| draws[((draws.len() - 1) as f64 * p).round() as usize];
        println!("    {label:<32} {point:.3}  [{:.3}, {:.3}]", q(0.025), q(0.975));
    }
}

fn print_vocadito(params: &NoteParams) -> Result<(), EvalError> {
    let corpus = vocadito::discover(&vocadito::corpus_dir())?;
    eprintln!("transcribing {} Vocadito stems for the baseline…", corpus.tracks.len());
    let mut total = Counts::default();
    let mut offs = Offsets::default();
    let (mut sum_conp, mut sum_conpoff) = (0.0, 0.0);
    let views: Vec<View> = parallel_map(&corpus.tracks, |track| -> Result<View, EvalError> {
        let reference = vocadito::load_reference(track)?;
        let (samples, sr) = wav::read_mono(&track.audio)?;
        let est = raw_notes(&transcribe(&samples, sr, 120.0, None));
        let mut refs = reference.notes.clone();
        refs.sort_by(|a, b| a.onset_s.total_cmp(&b.onset_s));
        let rest_term: Vec<bool> =
            (0..refs.len()).map(|i| refs.get(i + 1).is_none_or(|n| n.onset_s - refs[i].offset_s > 0.05)).collect();
        Ok(score(&refs, &rest_term, &est, params, None))
    })
    .into_iter()
    .collect::<Result<_, _>>()?;
    for v in &views {
        sum_conp += v.counts.conp_f1();
        sum_conpoff += v.counts.conpoff_f1();
        total.add(&v.counts);
        offs.add(&v.offsets);
    }
    let n = corpus.tracks.len() as f64;
    println!("\n== Baseline: Vocadito ({} stems, raw — no grid exists) ==\n", corpus.tracks.len());
    println!(
        "COnP F1 mean {:.3}   COnPOff F1 mean {:.3}   |   pooled {:.3} / {:.3}   |   off|rest-term {} (n={})   off|legato {} (n={})",
        sum_conp / n,
        sum_conpoff / n,
        total.conp_f1(),
        total.conpoff_f1(),
        pct(offs.rt_ok, offs.rt_pairs),
        offs.rt_pairs,
        pct(offs.legato_ok, offs.legato_pairs),
        offs.legato_pairs
    );
    Ok(())
}
