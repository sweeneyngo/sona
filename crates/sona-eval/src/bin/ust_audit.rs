//! **UST format audit** — the gate before the UST corpus pipeline is built.
//!
//!     cargo run -p sona-eval --bin sona-eval-ust-audit                 # committed fixtures
//!     cargo run -p sona-eval --bin sona-eval-ust-audit -- --corpus     # the UST cache
//!     cargo run -p sona-eval --bin sona-eval-ust-audit -- a.ust dir/   # explicit files/dirs
//!
//! The corpus's value proposition is "USTs give in-domain offset labels". A UST
//! is contiguous, so a legato note's offset is just the next onset; only
//! **rest-terminated** notes carry a genuine musical end. This prints that
//! fraction (the headline) per UST and pooled, with a plain reading of what it
//! implies, plus grid-alignment tightness and notes below sona's prune floor.
//!
//! The cache (`$SONA_UST_DIR`, default `crates/sona-eval/data/pifuyuu-ust`) is
//! git-ignored: the USTs' terms forbid redistribution. No audio is needed.

use std::path::PathBuf;
use std::process::ExitCode;

use sona_core::NoteHmm;
use sona_eval::ust::{self, Ust};
use sona_eval::ust_audit::{median_sorted, offset_signal_reading, ratio, GridFit, UstAudit, ONSET_TOLERANCE_S};
use sona_eval::EvalError;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-ust-audit: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), EvalError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (source, paths) = resolve_inputs(&args)?;

    // Notes sona structurally cannot emit: the note-HMM's post-processing prune
    // floor (ADR-0010), read from the core so the audit never drifts from it.
    let prune_s = NoteHmm::default().min_note_s;

    println!("== UST format audit ==\n");
    println!("source: {source}   files: {}   prune floor: {:.0} ms\n", paths.len(), prune_s * 1000.0);

    let mut audits = Vec::with_capacity(paths.len());
    for (k, path) in paths.iter().enumerate() {
        let ust = Ust::load(path)?;
        let audit = UstAudit::of(&ust, prune_s);
        let name = display_name(path);
        print_one(k + 1, &name, &ust, &audit);
        audits.push(audit);
    }

    let fractions: Vec<f64> = audits.iter().filter(|a| a.notes > 0).map(UstAudit::rest_terminated_fraction).collect();
    let pooled = UstAudit::pool(&audits);
    print_pooled(&pooled, &fractions, audits.len());
    Ok(())
}

/// Turn CLI args into a source label and a sorted list of UST files.
fn resolve_inputs(args: &[String]) -> Result<(String, Vec<PathBuf>), EvalError> {
    if args.iter().any(|a| a == "--corpus") {
        let root = ust::corpus_dir();
        return Ok((format!("UST cache {}", root.display()), ust::discover(&root)?));
    }
    if args.is_empty() {
        let root = ust::fixtures_dir();
        return Ok(("committed synthetic fixtures (run with --corpus for real USTs)".into(), ust::discover(&root)?));
    }
    let mut paths = Vec::new();
    for arg in args {
        let p = PathBuf::from(arg);
        if p.is_dir() {
            paths.extend(ust::discover(&p)?);
        } else {
            paths.push(p);
        }
    }
    Ok(("command-line paths".into(), paths))
}

fn print_one(k: usize, name: &str, ust: &Ust, a: &UstAudit) {
    let sixteenth_ms = a.sixteenth_s().unwrap_or(0.0) * 1000.0;
    println!(
        "[{k}] {name}   ({}, {} BPM, 16th = {:.0} ms){}",
        ust.encoding.label(),
        fmt_bpm(ust.tempo),
        sixteenth_ms,
        ust.project_name.as_deref().map(|p| format!("   project: {p}")).unwrap_or_default()
    );
    if a.tempo_changes > 0 {
        let changes: Vec<String> = ust.tempo_changes().iter().map(|(i, t)| format!("→{} @ block {i}", fmt_bpm(*t))).collect();
        println!(
            "    ⚠ {} mid-song tempo change(s) [{}] — the single-BPM assumption is broken for this song",
            a.tempo_changes,
            changes.join(", ")
        );
    }
    println!(
        "    blocks {}: notes {}, rests {}, breaths {}   duration {:.1} s",
        ust.notes.len(),
        a.notes,
        a.rests,
        a.breaths,
        a.duration_s
    );
    println!(
        "    rest-terminated   {} / {} = {}   (legato {})",
        a.rest_terminated,
        a.notes,
        pct(a.rest_terminated_fraction()),
        a.legato()
    );
    println!("    note starts       {}", fmt_grid(&a.starts));
    println!("    rest-term. ends   {}", fmt_grid(&a.rest_terminated_ends));
    println!(
        "    note length       shortest {} · longest {} · below {:.0} ms prune floor: {}\n",
        fmt_ms(a.shortest_s),
        fmt_ms(a.longest_s),
        a.prune_s * 1000.0,
        a.below_prune
    );
}

fn print_pooled(p: &UstAudit, fractions: &[f64], n_files: usize) {
    println!("== Pooled: {n_files} USTs, {} sung notes, {:.0} s ==\n", p.notes, p.duration_s);

    let mut sorted = fractions.to_vec();
    sorted.sort_by(f64::total_cmp);
    let spread = match (sorted.first(), median_sorted(&sorted), sorted.last()) {
        (Some(lo), Some(mid), Some(hi)) => {
            format!("   per-song min {} · median {} · max {}", pct(*lo), pct(mid), pct(*hi))
        }
        _ => String::new(),
    };
    println!(
        "rest-terminated     {} / {} = {}{spread}",
        p.rest_terminated,
        p.notes,
        pct(p.rest_terminated_fraction())
    );
    println!("note starts         {}", fmt_grid(&p.starts));
    println!("rest-term. ends     {}", fmt_grid(&p.rest_terminated_ends));
    println!(
        "below prune floor   {} / {} = {}   (notes sona structurally cannot emit)",
        p.below_prune,
        p.notes,
        pct(ratio(p.below_prune, p.notes))
    );
    println!("tempo changes       {} across {n_files} USTs", p.tempo_changes);

    println!("\nREADING");
    println!(
        "  · {} of sung notes are rest-terminated ({} genuine offset labels).",
        pct(p.rest_terminated_fraction()),
        p.rest_terminated
    );
    println!("  · {}", wrap(offset_signal_reading(p.rest_terminated_fraction()), 92, "    "));
    let starts_on_16th = ratio(p.starts.on_16th, p.starts.total());
    println!(
        "  · {} of note starts sit on a 16th line. Off-16th starts more than {:.0} ms from the nearest\n    \
         16th: {} — for those, a 16th hard-snap puts even a perfect detection outside the COnP window.",
        pct(starts_on_16th),
        ONSET_TOLERANCE_S * 1000.0,
        p.starts.beyond_tolerance()
    );
    if p.tempo_changes > 0 {
        println!(
            "  · {} tempo change(s): those songs need per-segment BPM or exclusion before scoring.",
            p.tempo_changes
        );
    }
}

/// `16th 5 · 32nd 0 · triplet 0 · off-grid 0   (off-16th residual median 42 ms, max 50 ms)`
fn fmt_grid(g: &GridFit) -> String {
    if g.total() == 0 {
        return "—".into();
    }
    let mut s = format!(
        "16th {} · 32nd {} · triplet {} · off-grid {}   → {} on 16th",
        g.on_16th,
        g.on_32nd,
        g.on_triplet,
        g.off_grid,
        pct(ratio(g.on_16th, g.total()))
    );
    if !g.off_16th_residual_s.is_empty() {
        let mut r = g.off_16th_residual_s.clone();
        r.sort_by(f64::total_cmp);
        let median = median_sorted(&r).unwrap_or(0.0);
        let max = r[r.len() - 1];
        s.push_str(&format!(
            "   (off-16th residual median {:.0} ms, max {:.0} ms)",
            median * 1000.0,
            max * 1000.0
        ));
    }
    s
}

/// `song/main.ust` — UST archives name their files generically (`main.ust`,
/// `cho.ust`), so the parent directory (the song) is what tells them apart.
fn display_name(path: &std::path::Path) -> String {
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let parent = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    match (parent, file) {
        (Some(p), Some(f)) => format!("{p}/{f}"),
        (None, Some(f)) => f,
        _ => path.display().to_string(),
    }
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

fn fmt_ms(s: Option<f64>) -> String {
    s.map_or_else(|| "—".into(), |s| format!("{:.0} ms", s * 1000.0))
}

fn fmt_bpm(bpm: f64) -> String {
    if bpm.fract() == 0.0 {
        format!("{bpm:.0}")
    } else {
        format!("{bpm:.2}")
    }
}

/// Greedy word-wrap with a hanging indent for continuation lines.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    let mut line_len = 0;
    for word in text.split_whitespace() {
        if line_len > 0 && line_len + 1 + word.chars().count() > width {
            out.push('\n');
            out.push_str(indent);
            line_len = 0;
        } else if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += word.chars().count();
    }
    out
}
