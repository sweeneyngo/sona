//! **UST corpus manifest check** (UST corpus gate, ticket 02).
//!
//!     cargo run -p sona-eval --bin sona-eval-ust-manifest            # validate + report
//!     cargo run -p sona-eval --bin sona-eval-ust-manifest -- --fill  # also write bpm/grid
//!
//! For every song in `corpus/ust-manifest.json` (USTs from the git-ignored cache,
//! populated by `scripts/fetch-pifuyuu-ust.sh`):
//! - the lead and other-voice USTs parse, share one tempo, and never change it;
//! - `bpm_from_ust` (exact, from the lead's `[#SETTING]`) and `grid_division`
//!   (16 or 24, whichever grid holds most lead note starts) — written with `--fill`;
//! - the lead's sung span fits inside the original's duration (a span longer
//!   than the original means the UST is timed to a different cut);
//! - how much of the lead is monophonic (no other voice sounding).
//!
//! Exits non-zero if any song fails a check.

use std::process::ExitCode;

use sona_eval::ust::{self, BlockKind};
use sona_eval::ust_audit::{ratio, GridFit, UstAudit};
use sona_eval::ust_corpus::{manifest_path, overlaps_any, Manifest, SongUsts};
use sona_eval::EvalError;

fn main() -> ExitCode {
    let fill = std::env::args().any(|a| a == "--fill");
    match run(fill) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("sona-eval-ust-manifest: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(fill: bool) -> Result<bool, EvalError> {
    let path = manifest_path();
    let mut manifest = Manifest::load(&path)?;
    let cache = ust::corpus_dir();
    let mut all_ok = true;

    println!("== UST corpus manifest: {} songs, {} excluded ==\n", manifest.songs.len(), manifest.excluded.len());
    println!(
        "{:<32} {:<17} {:>6} {:>4} {:>5} {:>7} {:>7} {:>6} {:>6}  checks",
        "song", "stratum", "bpm", "grid", "notes", "span s", "orig s", "mono", "rt"
    );

    for song in &mut manifest.songs {
        let usts = SongUsts::load(song, &cache)?;
        let mut problems: Vec<String> = Vec::new();

        let tempo = usts.lead[0].tempo;
        for u in usts.lead.iter().chain(&usts.other) {
            if (u.tempo - tempo).abs() > 1e-6 {
                problems.push(format!("tempo mismatch across files ({} vs {})", tempo, u.tempo));
            }
            if !u.tempo_changes().is_empty() {
                problems.push("mid-song tempo change".into());
            }
        }

        // Grid: which grid holds most lead note starts.
        let mut starts = GridFit::default();
        let audits: Vec<UstAudit> = usts.lead.iter().map(|u| UstAudit::of(u, 0.0)).collect();
        for a in &audits {
            starts.on_16th += a.starts.on_16th;
            starts.on_triplet += a.starts.on_triplet;
        }
        let grid = if starts.on_triplet > starts.on_16th { 24 } else { 16 };

        // Sung span vs. the original's length.
        let reference = usts.reference(0.0);
        let span = match (reference.notes.first(), reference.notes.iter().map(|n| n.offset_s).reduce(f64::max)) {
            (Some(first), Some(last)) => last - first.onset_s,
            _ => 0.0,
        };
        if let Some(orig) = song.original_duration_s {
            if span > orig + 1.0 {
                problems.push(format!("sung span {span:.0}s exceeds the original's {orig:.0}s — a different cut?"));
            }
        }

        // Monophonic share of the lead, and of its genuine offset labels.
        let poly = usts.polyphonic_spans(0.0);
        let mono: Vec<bool> = reference.notes.iter().map(|n| !overlaps_any(&poly, n.onset_s, n.offset_s)).collect();
        let n_mono = mono.iter().filter(|&&m| m).count();
        let n_rt_mono = mono.iter().zip(&reference.rest_terminated).filter(|(&m, &rt)| m && rt).count();
        let n_breath: usize = usts.lead.iter().map(|u| u.notes.iter().filter(|n| n.kind() == BlockKind::Breath).count()).sum();

        println!(
            "{:<32} {:<17} {:>6} {:>4} {:>5} {:>7.0} {:>7} {:>5.0}% {:>6}  {}",
            song.id,
            song.stratum,
            format!("{tempo}"),
            grid,
            reference.notes.len(),
            span,
            song.original_duration_s.map_or("?".into(), |d| format!("{d:.0}")),
            ratio(n_mono, reference.notes.len()) * 100.0,
            n_rt_mono,
            if problems.is_empty() { "ok".to_string() } else { problems.join("; ") }
        );
        if n_breath > 0 {
            println!("{:<32} ({n_breath} breath blocks in the lead, treated as gaps)", "");
        }
        all_ok &= problems.is_empty();

        if fill {
            song.bpm_from_ust = Some(tempo);
            song.grid_division = Some(grid);
        }
    }

    println!("\nmono = lead notes with no other voice sounding · rt = rest-terminated lead notes in monophonic regions");
    if fill {
        manifest.save(&path)?;
        println!("wrote bpm_from_ust + grid_division to {}", path.display());
    }
    Ok(all_ok)
}
