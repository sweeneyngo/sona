//! Frame metrics against the UST corpus (`sona_eval::frame`): tempo multiple and BPM drift from the
//! full mix, bar offset from the vocals stem at the UST's exact tempo. The `dev` songs only.
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-frame
//! ```
//! `drift@int` is the drift had the estimate been rounded to a whole BPM. The corpus keeps only
//! integer tempos, so it flatters rounding.
use std::process::ExitCode;
use sona_eval::frame::{drift_beats, measure, Frame, TempoMultiple, DRIFT_PASS_BEATS};
use sona_eval::ust_corpus::{manifest_path, stems_dir, Manifest};
use sona_eval::wav;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn opt(v: Option<f64>, width: usize, digits: usize) -> String {
    v.map_or(format!("{:>width$}", "-"), |x| format!("{x:>width$.digits$}"))
}

fn run() -> Result<(), String> {
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let stems = stems_dir();
    println!(
        "{:28} {:>4} {:>7} {:>7} {:>5} {:>6} {:>9} {:>4} {:>9} {:>5}",
        "song", "grid", "UST", "est", "mult", "drift", "drift@int", "bar", "residual", "frame"
    );
    let mut frames: Vec<Frame> = Vec::new();
    for song in manifest.songs.iter().filter(|s| s.split.as_deref() == Some("dev")) {
        let (Some(offset), Some(bpm)) = (song.alignment_offset_s, song.bpm_from_ust) else { continue };
        if !song.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) {
            continue;
        }
        let division = song.grid_division.unwrap_or(16);
        let (mix, mix_sr) = wav::read_mono(&audio.join(format!("{}.wav", song.id))).map_err(|e| format!("{}: {e}", song.id))?;
        let (vocals, vocals_sr) = wav::read_mono(&stems.join(&song.id).join("vocals.wav")).map_err(|e| format!("{}: {e}", song.id))?;
        let f = measure(&mix, mix_sr, &vocals, vocals_sr, bpm, division, offset);
        let duration_s = mix.len() as f64 / f64::from(mix_sr);
        let rounded = f.estimated_bpm.filter(|_| f.multiple == Some(TempoMultiple::One)).map(|e| drift_beats(e.round(), bpm, duration_s));
        println!(
            "{:28} {:>4} {:>7.1} {} {:>5} {} {} {:>4} {} {:>5}",
            song.id,
            division,
            bpm,
            opt(f.estimated_bpm, 7, 2),
            f.multiple.map_or("-", TempoMultiple::label),
            opt(f.drift_beats, 6, 2),
            opt(rounded, 9, 2),
            f.bar_offset.map_or("-".into(), |b| b.beats.to_string()),
            opt(f.bar_offset.map(|b| b.residual_beats), 9, 3),
            if f.passes() { "pass" } else { "FAIL" },
        );
        frames.push(f);
    }
    let n = frames.len();
    let count = |p: &dyn Fn(&Frame) -> bool| frames.iter().filter(|f| p(f)).count();
    println!(
        "\n{n} songs: frame pass {} · 1x tempo {} · drift < {DRIFT_PASS_BEATS} beat {} · bar offset 0 {} · |residual| < 1/8 beat {}",
        count(&|f| f.passes()),
        count(&|f| f.multiple == Some(TempoMultiple::One)),
        count(&|f| f.drift_beats.is_some_and(|d| d < DRIFT_PASS_BEATS)),
        count(&|f| f.bar_offset.is_some_and(|b| b.beats == 0)),
        count(&|f| f.bar_offset.is_some_and(|b| b.residual_beats.abs() < 0.125)),
    );
    Ok(())
}
