//! Frame metrics against the UST corpus (`sona_eval::frame`): tempo multiple and BPM drift from the
//! full mix, bar offset from the vocals stem at the UST's exact tempo. The `dev` songs only.
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-frame
//! ```
//! `drift@int` is the drift had the estimate been rounded to a whole BPM. The corpus keeps only
//! integer tempos, so it flatters rounding.
//!
//! `--bar-one` prints the bar-one study's table instead (`reports/bar-one.md`): where sona's tick 0
//! lands under today's rule and under "nearest grid line to 0 s", against the UST's bar line nearest
//! 0 s, and the beat phase `grid_phase` finds at one-beat spacing on the drums stem and on the mix,
//! in cells after the UST's beat lines. All at the UST's exact tempo.
use std::process::ExitCode;
use sona_core::tempo::grid_phase;
use sona_eval::frame::{cells_after, drift_beats, measure, nearest_tick0_s, sona_tick0_s, Frame, TempoMultiple, BEATS_PER_BAR, DRIFT_PASS_BEATS};
use sona_eval::quantize::grid_spacing_seconds;
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
    if std::env::args().any(|a| a == "--bar-one") {
        return bar_one();
    }
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

/// The bar-one table: every time in ms of audio, every shift in cells.
fn bar_one() -> Result<(), String> {
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let stems = stems_dir();
    println!(
        "{:28} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}",
        "song", "cell", "UST bar", "today", "nearest", "today-", "near-", "drums", "mix"
    );
    println!("{:28} {:>6} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}", "", "ms", "ms", "ms", "ms", "UST", "UST", "beat", "beat");
    for song in manifest.songs.iter().filter(|s| s.split.as_deref() == Some("dev")) {
        let (Some(offset), Some(bpm)) = (song.alignment_offset_s, song.bpm_from_ust) else { continue };
        if !song.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) {
            continue;
        }
        let cell = grid_spacing_seconds(bpm, f64::from(song.grid_division.unwrap_or(16)));
        let beat = 60.0 / bpm;
        let bar = BEATS_PER_BAR * beat;
        let read = |p: std::path::PathBuf| wav::read_mono(&p).map_err(|e| format!("{}: {e}", song.id));
        let (vocals, vsr) = read(stems.join(&song.id).join("vocals.wav"))?;
        let (drums, dsr) = read(stems.join(&song.id).join("drums.wav"))?;
        let (mix, msr) = read(audio.join(format!("{}.wav", song.id)))?;
        let phase = grid_phase(&vocals, vsr, cell).ok_or_else(|| format!("{}: no vocal onsets", song.id))?;
        let ust_bar = offset + (-offset / bar).round() * bar;
        let (today, nearest) = (sona_tick0_s(phase, cell), nearest_tick0_s(phase, cell));
        let beat_shift = |x: &[f32], sr: u32| grid_phase(x, sr, beat).map(|p| cells_after(p, offset, beat, cell));
        let fmt = |v: Option<f64>| v.map_or(format!("{:>7}", "-"), |x| format!("{x:>7.2}"));
        println!(
            "{:28} {:>6.1} {:>+8.1} {:>+8.1} {:>+8.1} {:>+7.2} {:>+7.2} {} {}",
            song.id,
            cell * 1e3,
            ust_bar * 1e3,
            today * 1e3,
            nearest * 1e3,
            (today - ust_bar) / cell,
            (nearest - ust_bar) / cell,
            fmt(beat_shift(&drums, dsr)),
            fmt(beat_shift(&mix, msr)),
        );
    }
    println!("\ntoday = first grid line at or after 0 s (the export); nearest = grid line nearest 0 s");
    println!("today-UST, near-UST: cells after the UST bar line nearest 0 s; drums/mix beat: grid_phase at one beat, cells after the UST's beat lines");
    Ok(())
}
