//! **UST phase alignment + onset-delta verification** (UST corpus gate, 05).
//!
//!     cargo run --release -p sona-eval --bin sona-eval-ust-align            # report
//!     cargo run --release -p sona-eval --bin sona-eval-ust-align -- --write # + manifest
//!
//! Per song: set `stem_time = ust_time + offset` WITHOUT sona (coarse: singing
//! mask vs vocal-stem loudness; fine: UST grid lines vs drum-stem onsets — see
//! `ust_align`), then VERIFY it with sona's onset detector: the signed delta
//! `detected − UST onset` over matched pairs, its spread, and its drift from the
//! first to the last third of the song.
//!
//! - deltas scattered around zero → aligned
//! - a constant shift → misaligned (or a perception bias common to every song)
//! - growing drift → the single-BPM assumption fails for the song
//!
//! `--write` stores `alignment_offset_s` and `alignment_status` (`aligned` /
//! `flagged: …`) in the manifest; flagged songs are excluded from the gate (06).

use std::process::ExitCode;

use sona_core::onset::OnsetDetector;
use sona_eval::metrics::matching::maximum_matching;
use sona_eval::ust_align::{coarse_offset, drum_novelty, fine_offset, log_rms, zscore, COARSE_HOP_S, MIN_DRUM_PEAK_RATIO};
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest, SongUsts};
use sona_eval::{ust, wav, EvalError};

/// Pairs further apart than this are not matched (seconds).
const MATCH_WINDOW_S: f64 = 0.100;
/// |median delta| above this reads as a constant shift.
const SHIFT_S: f64 = 0.035;
/// |last-third median − first-third median| above this reads as drift.
const DRIFT_S: f64 = 0.030;
/// Fewer matched onsets than this can't verify anything.
const MIN_MATCHED: usize = 30;

fn main() -> ExitCode {
    let write = std::env::args().any(|a| a == "--write");
    match run(write) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("sona-eval-ust-align: {e}");
            ExitCode::FAILURE
        }
    }
}

struct Verify {
    matched: usize,
    n_ref: usize,
    q: [f64; 5], // p10 p25 p50 p75 p90
    drift: f64,
}

fn run(write: bool) -> Result<(), EvalError> {
    let path = manifest_path();
    let mut manifest = Manifest::load(&path)?;
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let detector = OnsetDetector::default();

    println!("== UST phase alignment (set without sona) + onset-delta check (with sona) ==\n");
    println!(
        "{:<32} {:>8} {:>7} {:>6} {:>5}  {:>5} {:>5} {:>5} {:>5} {:>5} {:>6}  status",
        "song", "offset s", "fine Δ", "drums", "match", "p10", "p25", "p50", "p75", "p90", "drift"
    );
    println!("{:<32} {:>8} {:>7} {:>6} {:>5}  {:>35}", "", "", "ms", "peak", "", "detected − UST onset (ms)");

    for song in &mut manifest.songs {
        let dir = stems.join(&song.id);
        let (vocals_p, drums_p) = (dir.join("vocals.wav"), dir.join("drums.wav"));
        if !vocals_p.is_file() || !drums_p.is_file() {
            println!("{:<32} (no stems — run separate-stems.sh)", song.id);
            continue;
        }
        let usts = SongUsts::load(song, &cache)?;
        let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
        let beat_s = 60.0 / bpm;

        // 1. Coarse: any-voice singing mask vs. vocal loudness.
        let (vocals, sr) = wav::read_mono(&vocals_p)?;
        let mut env = log_rms(&vocals, sr, COARSE_HOP_S, 0.030);
        zscore(&mut env);
        let spans = usts.all_voice_spans(0.0);
        let Some(coarse) = coarse_offset(&spans, &env, COARSE_HOP_S, 2.0) else {
            println!("{:<32} UST longer than the stem — cannot align", song.id);
            song.alignment_status = Some("flagged: UST longer than the stem".into());
            continue;
        };

        // 2. Fine: every grid line across the sung span vs. drum onsets, ±½ cell.
        let (drums, dsr) = wav::read_mono(&drums_p)?;
        let nov = drum_novelty(&drums, dsr);
        let reference0 = usts.reference(0.0);
        let first = reference0.notes.first().map_or(0.0, |n| n.onset_s);
        let last = reference0.notes.iter().map(|n| n.offset_s).fold(0.0, f64::max);
        let cell_s = beat_s * 4.0 / song.grid_division.unwrap_or(16) as f64;
        let lines: Vec<f64> = (0..)
            .map(|k| k as f64 * cell_s)
            .skip_while(|&t| t < first - beat_s)
            .take_while(|&t| t <= last + beat_s)
            .collect();
        let fine = fine_offset(coarse, cell_s, &lines, &nov);
        let drums_ok = fine.peak_ratio >= MIN_DRUM_PEAK_RATIO;
        let offset = if drums_ok { fine.offset_s } else { coarse };
        let offset = (offset * 1000.0).round() / 1000.0;

        // 3. Verify with sona's onsets.
        let reference = usts.reference(offset);
        let detected: Vec<f64> = detector.detect(&vocals, sr).onsets.iter().map(|o| o.time_s).collect();
        let v = verify(&reference.notes.iter().map(|n| n.onset_s).collect::<Vec<_>>(), &detected);

        let mut reasons = Vec::new();
        if !drums_ok {
            reasons.push(format!("weak drum grid (peak {:.2}); coarse offset only", fine.peak_ratio));
        }
        if v.matched < MIN_MATCHED {
            reasons.push(format!("only {} matched onsets", v.matched));
        } else {
            if v.q[2].abs() > SHIFT_S {
                reasons.push(format!("constant shift {:+.0} ms", v.q[2] * 1000.0));
            }
            if v.drift.abs() > DRIFT_S {
                reasons.push(format!("drift {:+.0} ms over the song", v.drift * 1000.0));
            }
        }
        // A weak drum grid alone is a caveat, not a failure, if sona's onsets agree.
        let fatal = reasons.iter().any(|r| !r.starts_with("weak drum grid"));
        let status = if fatal { format!("flagged: {}", reasons.join("; ")) } else if reasons.is_empty() { "aligned".into() } else { format!("aligned ({})", reasons.join("; ")) };

        println!(
            "{:<32} {:>8.3} {:>+7.0} {:>6.2} {:>4.0}%  {:>+5.0} {:>+5.0} {:>+5.0} {:>+5.0} {:>+5.0} {:>+6.0}  {}",
            song.id,
            offset,
            (fine.offset_s - coarse) * 1000.0,
            fine.peak_ratio,
            100.0 * v.matched as f64 / v.n_ref.max(1) as f64,
            v.q[0] * 1000.0,
            v.q[1] * 1000.0,
            v.q[2] * 1000.0,
            v.q[3] * 1000.0,
            v.q[4] * 1000.0,
            v.drift * 1000.0,
            status
        );
        if write {
            song.alignment_offset_s = Some(offset);
            song.alignment_status = Some(status);
        }
    }

    println!(
        "\nfine Δ = drum-phase correction to the coarse offset · drums peak ≥ {MIN_DRUM_PEAK_RATIO} = grid trusted\n\
         flag when |p50| > {:.0} ms (shift), |drift| > {:.0} ms (first→last third), or < {MIN_MATCHED} matches",
        SHIFT_S * 1000.0,
        DRIFT_S * 1000.0
    );
    if write {
        manifest.save(&path)?;
        println!("wrote alignment_offset_s + alignment_status to {}", path.display());
    }
    Ok(())
}

/// Signed deltas `detected − reference` over a maximum matching within
/// ±[`MATCH_WINDOW_S`]: quantiles and early-vs-late drift.
fn verify(refs: &[f64], det: &[f64]) -> Verify {
    let adjacency: Vec<Vec<usize>> = refs
        .iter()
        .map(|&r| det.iter().enumerate().filter(|(_, &d)| (d - r).abs() <= MATCH_WINDOW_S).map(|(j, _)| j).collect())
        .collect();
    let mut pairs: Vec<(f64, f64)> = maximum_matching(&adjacency).into_iter().map(|(r, d)| (refs[r], det[d] - refs[r])).collect();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut deltas: Vec<f64> = pairs.iter().map(|p| p.1).collect();
    deltas.sort_by(f64::total_cmp);
    let q = |p: f64| if deltas.is_empty() { 0.0 } else { deltas[((deltas.len() - 1) as f64 * p).round() as usize] };
    let third = pairs.len() / 3;
    let med = |xs: &[(f64, f64)]| {
        let mut d: Vec<f64> = xs.iter().map(|p| p.1).collect();
        d.sort_by(f64::total_cmp);
        if d.is_empty() { 0.0 } else { d[d.len() / 2] }
    };
    let drift = if third > 0 { med(&pairs[pairs.len() - third..]) - med(&pairs[..third]) } else { 0.0 };
    Verify { matched: pairs.len(), n_ref: refs.len(), q: [q(0.1), q(0.25), q(0.5), q(0.75), q(0.9)], drift }
}
