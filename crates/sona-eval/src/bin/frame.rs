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
//! `--estimate-bar-one` scores bar 1 from the music (`sona_core::bar_one`, utau-edit-cost 05) against
//! the manifest's music frame, at the UST's exact tempo (a typed BPM, so the frame passes exactly
//! when the bar offset is 0): today's tick 0, the oracle with the music's own beat (first beat at or
//! after 0 s, and at or before), the estimator's beat alone, its harmonic bar, and the shipped rule
//! (harmonic bar, beat when the confidence is low). The music's bar is itself a harmonic estimate,
//! so the bar columns are circular until it is checked by ear.
//!
//! `--estimate-bar-one --beat-this PROGRAM [--beat-this-threads N] [--beat-this-out DIR]` also runs the
//! Beat This tool (`scripts/sona-beat-this.py`, ADR-0036) on each song's mix, one song at a time, and
//! scores a third rule, bar 1 from its downbeats on the estimator's beat (`bar_one::tracked_bar`), in a
//! second table; the first table is unchanged. It also prints the tool's vote against the music's own bar
//! line and its beat-cell vote, as ticket 15's `score.py` computes them (the port check), and its CPU and
//! wall seconds with the load average when it started. `--beat-this-out` keeps the tool's JSON in
//! `DIR/beat_this/<song>.json`, `score.py`'s layout (default: a temporary folder, removed after).
//!
//! `--bar-one` prints the bar-one study's table instead (`reports/bar-one.md`): where sona's tick 0
//! lands under today's rule and under "nearest grid line to 0 s", against the UST's bar line nearest
//! 0 s, and the beat phase `grid_phase` finds at one-beat spacing on the drums stem and on the mix,
//! in cells after the UST's beat lines. All at the UST's exact tempo.
use std::process::ExitCode;
use sona_core::tempo::grid_phase;
use sona_eval::frame::{
    cells_after, drift_beats, measure, music_bar_offset, nearest_tick0_s, overlay_clicks, sona_tick0_from_audio, sona_tick0_s, song_bootstrap, BarOffset, Frame,
    TempoMultiple, BEATS_PER_BAR, BOOTSTRAP_ITERS, DRIFT_PASS_BEATS,
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
    if args.iter().any(|a| a == "--estimate-bar-one") {
        return estimate_bar_one(&args);
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

/// Beat This (ADR-0036) on one song's mix, as the eval runs it: what it wrote, read back.
struct BeatThisRun {
    downbeats: Vec<f64>,
    beats: Vec<f64>,
    cpu_s: f64,
    wall_s: f64,
    load: f64,
}

struct BeatThisArgs {
    program: std::path::PathBuf,
    threads: u32,
    out: std::path::PathBuf,
    keep: bool,
}

impl BeatThisArgs {
    fn parse(args: &[String]) -> Result<Option<BeatThisArgs>, String> {
        let flag = |k: &str| args.iter().position(|a| a == k).map(|i| args.get(i + 1).cloned().ok_or(format!("{k} expects a value"))).transpose();
        let Some(program) = flag("--beat-this")? else {
            if flag("--beat-this-threads")?.is_some() || flag("--beat-this-out")?.is_some() {
                return Err("--beat-this-threads and --beat-this-out need --beat-this PROGRAM".into());
            }
            return Ok(None);
        };
        let threads = flag("--beat-this-threads")?.map_or(Ok(1), |v| v.parse().ok().filter(|n| *n >= 1).ok_or(format!("--beat-this-threads {v}: a whole number, at least 1")))?;
        let keep = flag("--beat-this-out")?;
        let out = keep.clone().map_or_else(|| std::env::temp_dir().join(format!("sona-eval-beat-this-{}", std::process::id())), std::path::PathBuf::from);
        Ok(Some(BeatThisArgs { program: program.into(), threads, out, keep: keep.is_some() }))
    }

    fn run(&self, id: &str, mix: &std::path::Path) -> Result<BeatThisRun, String> {
        let dir = self.out.join("beat_this");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let out = dir.join(format!("{id}.json"));
        let status = std::process::Command::new(&self.program)
            .args(["--threads", &self.threads.to_string(), "--out"])
            .arg(&out)
            .arg(mix)
            .current_dir(&dir)
            .envs(["OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"].map(|k| (k, self.threads.to_string())))
            .status()
            .map_err(|e| format!("{}: {e}", self.program.display()))?;
        if !status.success() {
            return Err(format!("{id}: Beat This failed ({status})"));
        }
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&out).map_err(|e| format!("{}: {e}", out.display()))?).map_err(|e| format!("{}: {e}", out.display()))?;
        let times = |k: &str| v[k].as_array().and_then(|a| a.iter().map(serde_json::Value::as_f64).collect::<Option<Vec<f64>>>()).ok_or(format!("{id}: no {k} in {}", out.display()));
        let number = |k: &str| v[k].as_f64().unwrap_or(f64::NAN);
        let (downbeats, beats) = (times("downbeats")?, times("beats")?);
        let mut fields = serde_json::Map::new();
        fields.insert("song".into(), id.into());
        fields.insert("candidate".into(), "beat_this".into());
        if let serde_json::Value::Object(mut written) = v.clone() {
            written.append(&mut fields);
            std::fs::write(&out, serde_json::Value::Object(written).to_string()).map_err(|e| e.to_string())?;
        }
        Ok(BeatThisRun { downbeats, beats, cpu_s: number("cpu_s"), wall_s: number("wall_s"), load: number("load") })
    }
}

impl Drop for BeatThisArgs {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.out);
        }
    }
}

/// `score.py`'s `fold_beat`: the cell (after the UST's beat lines, mod one beat) most of the tracker's
/// beats fall on, and the share that do; ties to the cell voted for first.
fn beat_cell_vote(beats: &[f64], ust_tick0_s: f64, cell: f64, cells_per_beat: i64) -> Option<(i64, f64)> {
    let cells: Vec<i64> = beats.iter().map(|t| ((t - ust_tick0_s) / cell).round() as i64 % cells_per_beat).map(|k| k.rem_euclid(cells_per_beat)).collect();
    let mut order: Vec<i64> = Vec::new();
    for &k in &cells {
        if !order.contains(&k) {
            order.push(k);
        }
    }
    let count = |k: i64| cells.iter().filter(|&&c| c == k).count();
    let best = order.iter().copied().fold(None, |b: Option<i64>, k| if b.is_none_or(|b| count(k) > count(b)) { Some(k) } else { b })?;
    Some((best, count(best) as f64 / cells.len() as f64))
}

/// The bar-1 estimator against the music frame: one row per dev song, bar offsets in beats.
fn estimate_bar_one(args: &[String]) -> Result<(), String> {
    let beat_this = BeatThisArgs::parse(args)?;
    let manifest = Manifest::load(&manifest_path()).map_err(|e| e.to_string())?;
    let audio = manifest_path().parent().map(|p| p.join("../data/pifuyuu-audio")).unwrap_or_default();
    let stems = stems_dir();
    const RULES: [&str; 6] = ["today", "beat@>=0", "beat@<=0", "est beat", "est harm", "shipped"];
    println!(
        "{:28} {:>4} {:>6} {:>6} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>5} {:>5}",
        "song", "grid", "ref", "est", "beat", RULES[0], RULES[1], RULES[2], RULES[3], RULES[4], RULES[5], "conf", "from"
    );
    println!("{:28} {:>4} {:>6} {:>6} {:>6}   (whole beats off the music's bar line, residual in beats)", "", "", "bar", "bar", "Δcell");
    let mut rows: Vec<[bool; 6]> = Vec::new();
    let mut tracked_rows: Vec<String> = Vec::new();
    let mut tracked_pass: Vec<bool> = Vec::new();
    for song in manifest.songs.iter().filter(|s| s.split.as_deref() == Some("dev")) {
        let (Some(offset), Some(bpm), Some(music)) = (song.alignment_offset_s, song.bpm_from_ust, song.music_frame()) else { continue };
        if !song.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")) {
            continue;
        }
        let division = song.grid_division.unwrap_or(16);
        let cell = grid_spacing_seconds(bpm, f64::from(division));
        let beat = 60.0 / bpm;
        let (mix, msr) = wav::read_mono(&audio.join(format!("{}.wav", song.id))).map_err(|e| format!("{}: {e}", song.id))?;
        let (vocals, vsr) = wav::read_mono(&stems.join(&song.id).join("vocals.wav")).map_err(|e| format!("{}: {e}", song.id))?;
        if msr != vsr {
            return Err(format!("{}: mix {msr} Hz, vocals {vsr} Hz", song.id));
        }
        let today = sona_tick0_from_audio(&vocals, vsr, bpm, division).ok_or_else(|| format!("{}: no vocal onsets", song.id))?;
        let est = sona_core::bar_one::estimate(&mix, &vocals, msr, bpm, division).ok_or_else(|| format!("{}: no estimate", song.id))?;
        let ref_beat = music.beat_line_s(offset, cell).rem_euclid(beat);
        let ref_before = if ref_beat > 1e-9 { ref_beat - beat } else { ref_beat };
        let tick0s = [today, ref_beat, ref_before, est.beat_s, est.beat_s + est.harmonic_beat as f64 * beat, est.bar_s];
        let offsets: Vec<BarOffset> = tick0s.iter().map(|&t| music_bar_offset(t, offset, bpm, division, music)).collect();
        let cells_per_beat = (f64::from(division) / 4.0).round();
        let beat_cells = cells_after(est.beat_s, music.beat_line_s(offset, cell), beat, cell);
        let beat_cells = if beat_cells > cells_per_beat / 2.0 { beat_cells - cells_per_beat } else { beat_cells };
        let cells_per_bar = cells_per_beat * BEATS_PER_BAR;
        let est_bar = cells_after(est.beat_s + est.harmonic_beat as f64 * beat, offset, BEATS_PER_BAR * beat, cell).round().rem_euclid(cells_per_bar);
        let cellsfmt = |b: &BarOffset| format!("{}{:+.2}", b.beats, b.residual_beats);
        println!(
            "{:28} {:>4} {:>6} {:>6} {:>+6.2} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>5.2} {:>5}",
            song.id,
            division,
            music.bar_shift_cells,
            est_bar,
            beat_cells,
            cellsfmt(&offsets[0]),
            cellsfmt(&offsets[1]),
            cellsfmt(&offsets[2]),
            cellsfmt(&offsets[3]),
            cellsfmt(&offsets[4]),
            cellsfmt(&offsets[5]),
            est.confidence,
            if est.from_harmony() { "harm" } else { "beat" },
        );
        let mut pass = [false; 6];
        for (p, b) in pass.iter_mut().zip(&offsets) {
            *p = b.beats == 0;
        }
        rows.push(pass);
        if let Some(bt) = &beat_this {
            let run = bt.run(&song.id, &audio.join(format!("{}.wav", song.id)))?;
            let tracked = sona_core::bar_one::tracked_bar(est.beat_s, bpm, &run.downbeats);
            let off = music_bar_offset(tracked.bar_s, offset, bpm, division, music);
            let reference = sona_core::bar_one::downbeat_vote(&run.downbeats, music.bar_line_s(offset, cell), beat);
            let beat_vote = beat_cell_vote(&run.beats, offset, cell, cells_per_beat as i64);
            let fmt_vote = |v: Option<(String, f64)>| v.map_or(format!("{:>4} {:>5}", "-", "-"), |(k, s)| format!("{k:>4} {s:>5.2}"));
            let duration_s = mix.len() as f64 / f64::from(msr);
            tracked_rows.push(format!(
                "{:28} {:>8} {:>5.2} {:>8} {} {} {:>4} {:>6.1} {:>6.1} {:>6.1} {:>5.1}",
                song.id,
                cellsfmt(&off),
                tracked.share,
                cellsfmt(&offsets[5]),
                fmt_vote(reference.map(|(k, s)| (k.to_string(), s))),
                fmt_vote(beat_vote.map(|(k, s)| (k.to_string(), s))),
                music.beat_shift_cells,
                run.cpu_s,
                run.wall_s,
                run.wall_s * 240.0 / duration_s,
                run.load,
            ));
            tracked_pass.push(off.beats == 0);
        }
    }
    let n = rows.len();
    println!("\n{n} songs, typed (UST) BPM: frame pass = bar offset 0 against the music's bar line ({BOOTSTRAP_ITERS} song resamples, 95%)");
    let as_f = |i: usize| -> Vec<f64> { rows.iter().map(|r| f64::from(u8::from(r[i]))).collect() };
    for (i, rule) in RULES.iter().enumerate() {
        let (lo, hi) = song_bootstrap(&as_f(i));
        let diff: Vec<f64> = rows.iter().map(|r| f64::from(u8::from(r[i])) - f64::from(u8::from(r[0]))).collect();
        let (dlo, dhi) = song_bootstrap(&diff);
        println!(
            "  {rule:<9} {}/{n}  [{lo:.0}, {hi:.0}]   vs today {:+}  [{dlo:+.0}, {dhi:+.0}]",
            rows.iter().filter(|r| r[i]).count(),
            diff.iter().sum::<f64>(),
        );
    }
    println!("ref bar / est bar: cells after the UST's bar line; beat Δcell: the estimator's beat minus the music's, in cells");
    println!("beat@>=0 / beat@<=0: the music's own beat (manifest) as bar 1, the first at or after 0 s / at or before (oracle for the beat alone)");
    if let Some(bt) = &beat_this {
        println!("\nBeat This ({}, {} thread{}): bar 1 from its downbeats on the estimator's beat (ADR-0036)", bt.program.display(), bt.threads, if bt.threads == 1 { "" } else { "s" });
        println!(
            "{:28} {:>8} {:>5} {:>8} {:>4} {:>5} {:>4} {:>5} {:>4} {:>6} {:>6} {:>6} {:>5}",
            "song", "beatthis", "share", "shipped", "ref", "share", "beat", "share", "ref", "cpu s", "wall s", "w/4min", "load"
        );
        for row in &tracked_rows {
            println!("{row}");
        }
        let as_f = |v: &[bool]| -> Vec<f64> { v.iter().map(|&p| f64::from(u8::from(p))).collect() };
        let ours = as_f(&tracked_pass);
        let (lo, hi) = song_bootstrap(&ours);
        let n = ours.len();
        for (name, col) in [("today", 0), ("shipped", 5)] {
            let diff: Vec<f64> = ours.iter().zip(&rows).map(|(b, r)| b - f64::from(u8::from(r[col]))).collect();
            let (dlo, dhi) = song_bootstrap(&diff);
            println!("  beat this {}/{n}  [{lo:.0}, {hi:.0}]   vs {name} {:+}  [{dlo:+.0}, {dhi:+.0}]", ours.iter().sum::<f64>(), diff.iter().sum::<f64>());
        }
        println!("beatthis / shipped: whole beats off the music's bar line + residual; ref: the downbeats' vote against the music's bar line");
        println!("(score.py's bar vote and share); beat: the beats' cell vote after the UST's beat lines (score.py's beat cell), ref = the music's");
        println!("cpu s / wall s: the tool's own clock, model loading included; w/4min: wall per 4 minutes of song; load: 1-minute load average at its start");
    }
    Ok(())
}
