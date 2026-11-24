//! Frame metrics against the UST corpus (`sona_eval::frame`): tempo multiple and BPM drift from the
//! full mix, bar offset from the vocals stem at the UST's exact tempo. The `dev` songs only.
//!
//! ```sh
//! cargo run --release -p sona-eval --bin sona-eval-frame
//! ```
//! `drift@int` is the drift had the estimate been rounded to a whole BPM. The corpus keeps only
//! integer tempos, so it flatters rounding.
//!
//! Bar offset and frame pass are given twice: against the UST's bar lines (`UST` columns) and, when
//! the manifest has `music_bar_shift_cells`, against the music's (`music` columns; `est` marks an
//! unverified estimate, `ear` an ear-checked one). The UST's lines are tied to the file, not the
//! music (`reports/bar-one.md`), so the music columns are the ones that say what a user hears.
//!
//! `--clicks SONG [--bar-shift CELLS] [--from S] [--seconds S] [--out PATH]` writes the song's mix
//! with clicks on a candidate grid instead: a high click on the music's bar line, a lower one on
//! each other beat, with the bar line `CELLS` cells after the UST's (default: the manifest's
//! `music_bar_shift_cells`, else 0). A 24 s excerpt from mid-song unless `--from`/`--seconds` say
//! otherwise; `--seconds 0` keeps the whole song. It reads no sona output, so it is safe for
//! labelling a holdout by ear (`reports/bar-one.md`, "Ear-labelling the music frame").
//!
//! `--bar-one` prints the bar-one study's table instead (`reports/bar-one.md`): where sona's tick 0
//! lands under today's rule and under "nearest grid line to 0 s", against the UST's bar line nearest
//! 0 s, and the beat phase `grid_phase` finds at one-beat spacing on the drums stem and on the mix,
//! in cells after the UST's beat lines. All at the UST's exact tempo.
use std::process::ExitCode;
use sona_core::tempo::grid_phase;
use sona_eval::frame::{
    cells_after, drift_beats, measure, nearest_tick0_s, overlay_clicks, sona_tick0_s, BarOffset, Frame, TempoMultiple, BEATS_PER_BAR, DRIFT_PASS_BEATS,
};
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

fn bar_cells(b: Option<BarOffset>) -> (String, String) {
    (b.map_or("-".into(), |b| b.beats.to_string()), opt(b.map(|b| b.residual_beats), 8, 3))
}

fn verdict(pass: Option<bool>) -> &'static str {
    match pass {
        Some(true) => "pass",
        Some(false) => "FAIL",
        None => "-",
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--bar-one") {
        return bar_one();
    }
    if args.iter().any(|a| a == "--clicks") {
        return clicks(&args);
    }
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let stems = stems_dir();
    println!(
        "{:28} {:>4} {:>7} {:>7} {:>5} {:>6} {:>9}   {:>4} {:>8} {:>5}   {:>4} {:>8} {:>5} {:>4}",
        "", "", "", "", "", "", "", "UST", "", "", "music", "", "", ""
    );
    println!(
        "{:28} {:>4} {:>7} {:>7} {:>5} {:>6} {:>9}   {:>4} {:>8} {:>5}   {:>4} {:>8} {:>5} {:>4}",
        "song", "grid", "BPM", "est", "mult", "drift", "drift@int", "bar", "residual", "frame", "bar", "residual", "frame", "src"
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
        let music = song.music_frame();
        let f = measure(&mix, mix_sr, &vocals, vocals_sr, bpm, division, offset, music);
        let duration_s = mix.len() as f64 / f64::from(mix_sr);
        let rounded = f.estimated_bpm.filter(|_| f.multiple == Some(TempoMultiple::One)).map(|e| drift_beats(e.round(), bpm, duration_s));
        let (ust_bar, ust_residual) = bar_cells(f.bar_offset);
        let (music_bar, music_residual) = bar_cells(f.music_bar_offset);
        println!(
            "{:28} {:>4} {:>7.1} {} {:>5} {} {}   {:>4} {} {:>5}   {:>4} {} {:>5} {:>4}",
            song.id,
            division,
            bpm,
            opt(f.estimated_bpm, 7, 2),
            f.multiple.map_or("-", TempoMultiple::label),
            opt(f.drift_beats, 6, 2),
            opt(rounded, 9, 2),
            ust_bar,
            ust_residual,
            verdict(Some(f.passes())),
            music_bar,
            music_residual,
            verdict(f.passes_on_music()),
            music.map_or("-", |m| if m.ear_checked { "ear" } else { "est" }),
        );
        frames.push(f);
    }
    let n = frames.len();
    let count = |p: &dyn Fn(&Frame) -> bool| frames.iter().filter(|f| p(f)).count();
    println!(
        "\n{n} songs: 1x tempo {} · drift < {DRIFT_PASS_BEATS} beat {}",
        count(&|f| f.multiple == Some(TempoMultiple::One)),
        count(&|f| f.drift_beats.is_some_and(|d| d < DRIFT_PASS_BEATS)),
    );
    println!(
        "against the UST: frame pass {} · bar offset 0 {} · |residual| < 1/8 beat {}",
        count(&|f| f.passes()),
        count(&|f| f.bar_offset.is_some_and(|b| b.beats == 0)),
        count(&|f| f.bar_offset.is_some_and(|b| b.residual_beats.abs() < 0.125)),
    );
    println!(
        "against the music ({} songs with a music frame): frame pass {} · bar offset 0 {} · |residual| < 1/8 beat {}",
        count(&|f| f.music_bar_offset.is_some()),
        count(&|f| f.passes_on_music() == Some(true)),
        count(&|f| f.music_bar_offset.is_some_and(|b| b.beats == 0)),
        count(&|f| f.music_bar_offset.is_some_and(|b| b.residual_beats.abs() < 0.125)),
    );
    Ok(())
}

/// Writes a song's mix with clicks on a candidate beat and bar grid, for an ear check.
fn clicks(args: &[String]) -> Result<(), String> {
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let number = |k: &str| flag(k).map(|v| v.parse::<f64>().map_err(|e| format!("{k} {v}: {e}"))).transpose();
    let id = flag("--clicks").ok_or("--clicks needs a song id")?;
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let song = manifest.songs.iter().find(|s| s.id == id).ok_or_else(|| format!("{id}: not in the manifest"))?;
    let (Some(offset), Some(bpm)) = (song.alignment_offset_s, song.bpm_from_ust) else {
        return Err(format!("{id}: no alignment or BPM in the manifest"));
    };
    let division = song.grid_division.unwrap_or(16);
    let cell = grid_spacing_seconds(bpm, f64::from(division));
    let beat = 60.0 / bpm;
    let cells_per_bar = (BEATS_PER_BAR * f64::from(division) / 4.0).round() as u32;
    let shift = match flag("--bar-shift") {
        Some(v) => v.parse::<u32>().map_err(|e| format!("--bar-shift {v}: {e}"))?,
        None => song.music_bar_shift_cells.unwrap_or(0),
    };
    if shift >= cells_per_bar {
        return Err(format!("--bar-shift {shift}: a bar is {cells_per_bar} cells"));
    }
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let (mix, sr) = wav::read_mono(&audio.join(format!("{id}.wav"))).map_err(|e| format!("{id}: {e}"))?;
    let bar_line = offset + f64::from(shift) * cell;
    let with_clicks = overlay_clicks(&mix, sr, bar_line, beat);
    let duration = mix.len() as f64 / f64::from(sr);
    let seconds = number("--seconds")?.unwrap_or(24.0);
    let from = number("--from")?.unwrap_or(if seconds > 0.0 { (duration / 2.0 - seconds / 2.0).max(0.0) } else { 0.0 });
    let start = ((from * f64::from(sr)) as usize).min(with_clicks.len());
    let end = if seconds > 0.0 { (start + (seconds * f64::from(sr)) as usize).min(with_clicks.len()) } else { with_clicks.len() };
    let out = match flag("--out") {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let dir = manifest_path().parent().map(|p| p.join("../data/click-overlays")).unwrap_or_default();
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            dir.join(format!("{id}-bar{shift}.wav"))
        }
    };
    wav::write_mono_16bit_at(&out, &with_clicks[start..end], sr).map_err(|e| e.to_string())?;
    println!(
        "{}: {id}, {bpm} BPM, bar line {shift} cells after the UST's ({:.1} ms of audio, mod one bar), {:.1}–{:.1} s",
        out.display(),
        bar_line.rem_euclid(BEATS_PER_BAR * beat) * 1e3,
        start as f64 / f64::from(sr),
        end as f64 / f64::from(sr),
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
