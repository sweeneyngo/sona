//! The learned cell stage, ported (ADR-0026), scored per grid cell against the lead UST over the UST's
//! span — the prototype's own metric — with the stage on sona's estimated grid phase (what ships) and,
//! for comparison with the prototype's 78.8%, on the UST's true phase.
//!
//! ```sh
//! ORT_DYLIB_PATH=… SONA_GAME_DIR=… cargo run --release -p sona-eval --features game --bin sona-eval-cellstage -- \
//!     --model ~/.cache/sona/cellstage-D [--split holdout3 --reason "…" --reread]
//! ```
use std::path::PathBuf;
use std::process::ExitCode;
use sona_core::cellstage::snap_notes;
use sona_core::game::{cell_stage_notes, cells_covering, phase_of, CellModel, GameModel, GameParams, StageRuns};
use sona_core::{GridMode, PYin, Producer, Transcriber};
use sona_eval::holdout::authorize;
use sona_eval::quantize::grid_spacing_seconds;
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest, SongUsts};
use sona_eval::{ust, wav};

type Notes = Vec<(f64, f64, i32)>;

fn cells(reference: &Notes, est: &Notes, spacing: f64, anchor: f64) -> (usize, usize) {
    let at = |ns: &Notes, t: f64| -> i32 {
        let i = ns.partition_point(|n| n.0 <= t);
        if i > 0 && ns[i - 1].0 <= t && t < ns[i - 1].1 {
            ns[i - 1].2
        } else {
            -1
        }
    };
    let (first, last) = (reference[0].0, reference[reference.len() - 1].1);
    let mut k = ((first - anchor) / spacing).round_ties_even() as i64;
    let (mut ok, mut n) = (0, 0);
    while anchor + (k as f64 + 0.5) * spacing < last {
        let t = anchor + (k as f64 + 0.5) * spacing;
        ok += usize::from(at(reference, t) == at(est, t));
        n += 1;
        k += 1;
    }
    (ok, n)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let model_dir = PathBuf::from(flag("--model").ok_or("--model DIR (an exported cell stage) is required")?);
    let split = flag("--split").unwrap_or_else(|| "dev".into());
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    if split != "dev" {
        authorize(&manifest, &split, "sona-eval-cellstage", flag("--reason").as_deref(), args.iter().any(|a| a == "--reread"))?;
    }
    let game_dir = std::env::var("SONA_GAME_DIR").map_err(|_| "set SONA_GAME_DIR")?;
    let game = GameModel::load(std::path::Path::new(&game_dir), GameParams::default()).map_err(|e| e.to_string())?;
    let model = CellModel::load(&model_dir, GameParams::default().threads).map_err(|e| e.to_string())?;
    let (cache, stems) = (ust::corpus_dir(), stems_dir());
    let songs: Vec<_> = manifest
        .songs
        .iter()
        .filter(|s| s.split.as_deref() == Some(split.as_str()) && s.stratum == "vocaloid-original")
        .filter(|s| s.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) && s.alignment_offset_s.is_some())
        .filter(|s| stems.join(&s.id).join("lead.wav").is_file() && stems.join(&s.id).join("vocals.wav").is_file())
        .filter(|s| std::env::var("SONA_ONLY").map_or(true, |only| only.split(',').any(|o| o == s.id)))
        .collect();
    let mut tot = [(0usize, 0usize); 4];
    println!("{:28} {:>9} {:>9} {:>9} {:>9}", "song", "lead/est", "stage/est", "lead/UST", "stage/UST");
    for song in songs {
        let offset = song.alignment_offset_s.expect("filtered");
        let usts = SongUsts::load(song, &cache).map_err(|e| e.to_string())?;
        let bpm = song.bpm_from_ust.unwrap_or(usts.lead[0].tempo);
        let division = song.grid_division.unwrap_or(16);
        let spacing = grid_spacing_seconds(bpm, f64::from(division));
        let reference: Notes = usts.reference(offset).notes.iter().map(|r| (r.onset_s, r.offset_s, i32::from(r.midi_pitch))).collect();
        let (lead, sr) = wav::read_mono(&stems.join(&song.id).join("lead.wav")).map_err(|e| e.to_string())?;
        let (vocals, _) = wav::read_mono(&stems.join(&song.id).join("vocals.wav")).map_err(|e| e.to_string())?;
        let t = Transcriber { grid: Some(GridMode { division, ..GridMode::sixteenths() }), ..Transcriber::default() };
        let a = t.transcribe(&PYin::default(), &vocals, sr, bpm, Producer::Pyin);
        let sona: Notes = a.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
        let estimated = phase_of(&sona, spacing);
        let f0: Vec<(f64, f64)> = a.f0_contour.iter().map(|f| (f.time_s, f64::from(f.frequency_hz))).collect();
        let rms: Vec<(f64, f64)> = a.energy_envelope.iter().map(|e| (e.time_s, f64::from(e.rms))).collect();
        let duration = vocals.len() as f64 / f64::from(sr);
        let runs = StageRuns::compute(&game, &lead, &vocals, sr).map_err(|e| e.to_string())?;
        let mut row = [(0usize, 0usize); 4];
        for (j, anchor) in [estimated, offset.rem_euclid(spacing)].into_iter().enumerate() {
            let lead_only = snap_notes(&runs.lead_notes, spacing, anchor);
            let stage = cell_stage_notes(&model, &runs, &sona, &f0, &rms, spacing, anchor, cells_covering(duration, spacing, anchor)).map_err(|e| e.to_string())?;
            row[2 * j] = cells(&reference, &lead_only, spacing, offset);
            row[2 * j + 1] = cells(&reference, &stage, spacing, offset);
        }
        let pct = |(o, n): (usize, usize)| 100.0 * o as f64 / n.max(1) as f64;
        println!("{:28} {:>8.1}% {:>8.1}% {:>8.1}% {:>8.1}%", song.id, pct(row[0]), pct(row[1]), pct(row[2]), pct(row[3]));
        for (t, r) in tot.iter_mut().zip(row) {
            t.0 += r.0;
            t.1 += r.1;
        }
    }
    let pct = |(o, n): (usize, usize)| 100.0 * o as f64 / n.max(1) as f64;
    println!("{:28} {:>8.2}% {:>8.2}% {:>8.2}% {:>8.2}%   ({} cells)", "ALL", pct(tot[0]), pct(tot[1]), pct(tot[2]), pct(tot[3]), tot[0].1);
    Ok(())
}
