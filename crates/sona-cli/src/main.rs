//! `sona` — command-line front-end to the perception core.
//!
//! `sona transcribe <STEM> --bpm <BPM> [--midi <PATH>]` decodes a WAV vocal stem,
//! runs the perception pipeline ([`sona_core::Transcriber`], pYIN backend), and
//! prints the annotation JSON contract to stdout. With `--midi` it also writes a
//! raw, unquantized `.mid` as a convenience — *not* the authoritative export,
//! which the web review surface produces from the JSON (ADR-0004).
//!
//! `sona serve` hosts the review surface and runs jobs (`serve`, `jobs`). Argument
//! parsing is hand-rolled: two subcommands with about a dozen options each, so a
//! dependency-free parser keeps the build as lean as the core's.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::ExitCode;

use sona_core::{CueModel, GridMode, PYin, Producer, Transcriber};

mod beat_this;
mod http;
mod jobs;
mod provenance;
mod serve;
mod stems;

/// `sona transcribe` with every option it takes: `--help` and the missing-stem error both print it.
macro_rules! transcribe_synopsis {
    () => {
        "sona transcribe <STEM> --bpm <BPM> [--midi <PATH>] [--grid <16|24>] [--no-salience]\n                    \
         [--no-spectral-cue] [--game <DIR>] [--no-octave-check] [--cell-model <DIR>]\n                    \
         [--vocals <WAV>] [--bar-one <MIX>] [--bar-one-tracker <harmonic|beat-this>]\n                    \
         [--beat-this <PROGRAM>] [--beat-this-threads <N>] [--tool-timeout <S>]"
    };
}

const USAGE: &str = concat!("\
sona — vocal-to-MIDI perception core (CLI)

USAGE:
    sona <COMMAND> [OPTIONS]
    ", transcribe_synopsis!(), "
    sona serve [OPTIONS]

COMMANDS:
    transcribe <STEM> --bpm <BPM>    Transcribe a WAV vocal stem to annotation JSON
                                     (printed to stdout)
    serve                            Run the review surface with one-click generation
                                     (song → stem → annotation) on http://127.0.0.1:8977

OPTIONS (transcribe):
    --bpm <BPM>      Tempo, carried into the annotation for the UI's grid (required)
    --midi <PATH>    Also write a raw, unquantized .mid alongside the JSON
    --grid <16|24>   Decode notes on a 16th (16) or 16th-triplet (24) grid at --bpm,
                     for grid-locked material such as Vocaloid songs (phase is estimated)
    --no-salience    With --grid: turn off harmonic-salience pitch evidence, which
                     follows the lead where other voices sing (on by default, ADR-0016)
    --no-spectral-cue
                     With --grid: turn off the spectral model that splits held pitches
                     at syllable changes (on by default, ADR-0016)
    --game <DIR>     With --grid: take the notes from openvpi GAME instead (experimental,
                     ADR-0017). DIR is a GAME ONNX release folder you downloaded
                     (e.g. GAME-1.0.3-medium-onnx; weights are CC BY-NC-SA, non-commercial).
                     Needs a build with `--features game`, ORT_DYLIB_PATH set to an ONNX
                     Runtime library, and a 44.1 kHz stem (a lead-vocal stem works best)
    --no-octave-check
                     With --game: skip the subharmonic octave check (on by default)
    --cell-model <DIR>
                     With --game: let the learned cell stage choose each cell (ADR-0026). DIR is a
                     folder from crates/sona-eval/scripts/cellstage/export_onnx.py; the stem is then
                     the lead, and --vocals names the vocals stem it was separated from
    --vocals <WAV>   With --cell-model: the vocals stem (demucs) beside the lead stem; with
                     --bar-one: the vocals stem the mix is measured without (default: the stem)
    --bar-one <MIX>  Also estimate where bar 1 falls in the music (experimental, opt-in): the beat
                     from the full mix MIX (a WAV at the stem's rate), the bar from its harmony
                     without the vocals. Written to the annotation's `bar_one`; at --grid, else 16
    --bar-one-tracker <harmonic|beat-this>
                     With --bar-one: where the bar comes from (default harmonic). beat-this takes it
                     from the downbeats of Beat This! (MIT), a tool that `scripts/setup-separators.sh
                     beat-this` installs; the harmony's bar 1 is kept beside it (ADR-0036)
    --beat-this <PROGRAM>
                     The Beat This tool (default <separator venv>/bin/sona-beat-this, the venv from
                     SONA_LEAD_VENV, else ~/.local/share/sona/separator-venv)
    --beat-this-threads <N>
                     CPU threads Beat This may use (default 1)
    --tool-timeout <S>
                     Stop Beat This after S seconds (default 600, or 20 s per second of the mix if longer)
    -h, --help       Print this usage
    -V, --version    Print version

OPTIONS (serve):
    --port <N>             Listen on 127.0.0.1:<N> (default 8977; 0 picks a free port)
    --web-dir <DIR>        The built review surface (default: web/dist in this checkout)
    --cache-dir <DIR>      Where jobs and the stem cache go (default $XDG_CACHE_HOME/sona,
                           else ~/.cache/sona; ADR-0030)
    --jobs-dir <DIR>       Where uploads and results go (default <cache-dir>/jobs)
    --keep-jobs <N>        Finished jobs kept on disk, newest first (default 20)
    --keep-stems <N>       Separated songs kept in <cache-dir>/stems, so transcribing one again at
                           another BPM, grid or engine skips separation (default 10; 0 turns it off)
    --tool-timeout <S>     Stop a separator or ffmpeg run after S seconds (default 600) or after
    --tool-timeout-ratio <R>  R seconds per second of audio (default 20), whichever is longer
    --demucs-venv <DIR>    demucs venv for vocal separation (default ~/.local/share/sona/demucs-venv)
    --separator-venv <DIR> audio-separator venv for the lead (default ~/.local/share/sona/separator-venv)
    --game <DIR>           Offer GAME note extraction (needs --features game and ORT_DYLIB_PATH)
    --cell-model <DIR>     Offer GAME + the learned cell stage (ADR-0026; needs --game)
    --beat-this <PROGRAM>  The Beat This tool, an opt-in source of bar 1 (ADR-0036; default
                           <separator venv>/bin/sona-beat-this, from scripts/setup-separators.sh beat-this)
    --beat-this-threads <N>  CPU threads Beat This may use (default 1)

The core emits the annotation JSON contract (raw note events + confidence +
f0 contour + energy envelope); the web review surface interprets it (grid,
quantization, correction) and exports UST, USTX and MIDI. See CONTEXT.md.");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("-V") | Some("--version") => {
            println!("sona {}", sona_core::VERSION);
            ExitCode::SUCCESS
        }
        // No command, or an explicit help request: print usage.
        None | Some("-h") | Some("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("transcribe" | "serve") if args[1..].iter().any(|a| a == "-h" || a == "--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("transcribe") => match run_transcribe(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("sona: {e}");
                ExitCode::FAILURE
            }
        },
        Some("serve") => match serve::run_serve(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("sona: {e}");
                ExitCode::FAILURE
            }
        },
        Some(other) => {
            eprintln!("sona: unknown command '{other}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

/// Parsed `transcribe` arguments.
struct TranscribeArgs {
    stem: String,
    bpm: f64,
    midi: Option<String>,
    grid: Option<u32>,
    salience: bool,
    spectral_cue: bool,
    game: Option<String>,
    octave_check: bool,
    vocals: Option<String>,
    cell_model: Option<String>,
    bar_one: Option<String>,
    /// Bar 1 from Beat This's downbeats instead of the harmony (ADR-0036).
    beat_this: Option<BeatThisInput>,
}

/// Where Beat This's downbeats come from: a run of the tool (`sona transcribe`), or the file a `sona
/// serve` job's own run of it wrote.
enum BeatThisInput {
    Run { program: std::path::PathBuf, threads: u32, timeout_s: Option<f64> },
    Read(std::path::PathBuf),
}

/// The `transcribe` subcommand: parse args, decode the stem, run the pipeline,
/// emit JSON (and optional MIDI). Returns a human-readable message on any error.
fn run_transcribe(args: &[String]) -> Result<(), String> {
    let parsed = parse_transcribe_args(args)?;
    let annotation = transcribe(&parsed)?;

    // The JSON contract is the primary artifact: it goes to stdout so the command
    // composes in a pipeline.
    let json = serde_json::to_string_pretty(&annotation)
        .map_err(|e| format!("serializing annotation: {e}"))?;
    println!("{json}");

    if let Some(midi_path) = &parsed.midi {
        write_midi_file(&annotation, midi_path)?;
        // Progress on stderr keeps stdout pure JSON.
        eprintln!(
            "sona: wrote raw MIDI ({} notes) to {midi_path}",
            annotation.notes.len()
        );
    }
    Ok(())
}

fn transcribe(parsed: &TranscribeArgs) -> Result<sona_core::Annotation, String> {
    let (samples, sample_rate) = load_wav_mono(&parsed.stem)?;
    // With --game, GAME supplies the notes; the default pipeline still supplies
    // the f0 contour and energy envelope the review surface draws, so skip its
    // (replaced) grid decode.
    let grid = if parsed.game.is_some() { None } else { parsed.grid.map(|division| grid_mode(division, parsed)) };
    let transcriber = Transcriber { grid, ..Transcriber::default() };
    #[allow(unused_mut)]
    let mut annotation = transcriber.transcribe(&PYin::default(), &samples, sample_rate, parsed.bpm, Producer::Pyin);
    if let (Some(dir), Some(division), Some(model), Some(vocals)) = (&parsed.game, parsed.grid, &parsed.cell_model, &parsed.vocals) {
        cell_stage(&mut annotation, &transcriber, &samples, vocals, dir, model, division, parsed)?;
    } else if let (Some(dir), Some(division)) = (&parsed.game, parsed.grid) {
        game_notes(&mut annotation, &transcriber, &samples, dir, division, parsed.octave_check)?;
    }
    let mut downbeats = None;
    if let Some(mix) = &parsed.bar_one {
        (annotation.bar_one, downbeats) = bar_one(mix, parsed.vocals.as_deref().unwrap_or(&parsed.stem), parsed.bpm, parsed.grid.unwrap_or(16), parsed.beat_this.as_ref())?;
        if annotation.bar_one.is_none() {
            eprintln!("sona: --bar-one: too little audio to hear a beat, so no bar 1");
        }
    }
    annotation.provenance = Some(transcribe_provenance(parsed, downbeats.as_ref())?);
    Ok(annotation)
}

type BarOneResult = (Option<sona_core::BarOneEstimate>, Option<beat_this::Downbeats>);

/// Bar 1 from the full mix and the vocals stem (utau-edit-cost 05): from the harmony, or from Beat This's
/// downbeats on the same beat with the harmony's bar 1 kept beside it (ADR-0036). With Beat This, also
/// what the tool reported, for the provenance.
#[cfg(feature = "spectral")]
fn bar_one(mix_path: &str, vocals_path: &str, bpm: f64, division: u32, beat_this: Option<&BeatThisInput>) -> Result<BarOneResult, String> {
    let (mix, mix_rate) = load_wav_mono(mix_path)?;
    let (vocals, vocals_rate) = load_wav_mono(vocals_path)?;
    if mix_rate != vocals_rate {
        return Err(format!("--bar-one needs the mix at the vocals' rate: '{mix_path}' is {mix_rate} Hz, '{vocals_path}' {vocals_rate} Hz"));
    }
    let Some(harmony) = sona_core::bar_one::estimate(&mix, &vocals, mix_rate, bpm, division) else { return Ok((None, None)) };
    let Some(input) = beat_this else {
        return Ok((Some(sona_core::BarOneEstimate { time_s: harmony.bar_s, confidence: harmony.confidence, harmonic: harmony.from_harmony(), source: None, harmonic_time_s: None }), None));
    };
    let downbeats = match input {
        BeatThisInput::Run { program, threads, timeout_s } => {
            let audio_s = mix.len() as f64 / f64::from(mix_rate);
            let limit = timeout_s.unwrap_or_else(|| (20.0 * audio_s).max(600.0));
            beat_this::run(program, *threads, Path::new(mix_path), std::time::Duration::from_secs_f64(limit))?
        }
        BeatThisInput::Read(path) => beat_this::read(path)?,
    };
    let tracked = sona_core::bar_one::tracked_bar(harmony.beat_s, bpm, &downbeats.downbeats);
    let estimate = sona_core::BarOneEstimate {
        time_s: tracked.bar_s,
        confidence: tracked.share,
        harmonic: tracked.bar_beat.is_some(),
        source: Some(sona_core::BarOneSource::BeatThis),
        harmonic_time_s: Some(harmony.bar_s),
    };
    Ok((Some(estimate), Some(downbeats)))
}

#[cfg(not(feature = "spectral"))]
fn bar_one(_: &str, _: &str, _: f64, _: u32, _: Option<&BeatThisInput>) -> Result<BarOneResult, String> {
    Err("--bar-one needs sona built with the `spectral` feature".into())
}

fn transcribe_provenance(parsed: &TranscribeArgs, downbeats: Option<&beat_this::Downbeats>) -> Result<sona_core::Provenance, String> {
    let input = provenance::fingerprint_file("input", Path::new(&parsed.stem))?;
    let mut settings = std::collections::BTreeMap::new();
    let mut set = |k: &str, v: String| settings.insert(k.to_string(), v);
    set("bpm", parsed.bpm.to_string());
    set("grid", parsed.grid.map_or("none".into(), |g| g.to_string()));
    set("engine", if parsed.cell_model.is_some() { "game+cells" } else if parsed.game.is_some() { "game" } else { "pyin" }.into());
    if parsed.grid.is_some() && parsed.game.is_none() {
        set("salience", parsed.salience.to_string());
        set("spectral_cue", parsed.spectral_cue.to_string());
    }
    let mut models = Vec::new();
    if let Some(dir) = &parsed.game {
        set("octave_check", parsed.octave_check.to_string());
        for (k, v) in game_settings() {
            set(&k, v);
        }
        if let Some(lib) = std::env::var_os("ORT_DYLIB_PATH") {
            set("onnxruntime", Path::new(&lib).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        }
        for graph in ["encoder", "segmenter", "estimator", "dur2bd", "bd2dur"] {
            models.push(provenance::fingerprint_file(&format!("game/{graph}"), &Path::new(dir).join(format!("{graph}.onnx")))?);
        }
    }
    if let (Some(model), Some(vocals)) = (&parsed.cell_model, &parsed.vocals) {
        models.push(provenance::fingerprint_file("cells/model", &Path::new(model).join("cellstage.onnx"))?);
        models.push(provenance::fingerprint_file("input/vocals", Path::new(vocals))?);
    }
    if let Some(mix) = &parsed.bar_one {
        set("bar_one", "mix".into());
        models.push(provenance::fingerprint_file("input/mix", Path::new(mix))?);
        if let (Some(vocals), None) = (&parsed.vocals, &parsed.cell_model) {
            models.push(provenance::fingerprint_file("input/vocals", Path::new(vocals))?);
        }
    }
    if let Some(d) = downbeats {
        set("bar_one_source", "beat-this".into());
        set("beat_this", d.version.clone());
        set("beat_this_checkpoint", d.checkpoint.clone());
        if let Some(BeatThisInput::Run { threads, .. }) = &parsed.beat_this {
            set("beat_this_threads", threads.to_string());
        }
        if let Some(path) = &d.checkpoint_path {
            models.push(provenance::fingerprint_file("bar_one/beat-this", path)?);
        }
    }
    Ok(provenance::provenance(input, settings, models))
}

#[cfg(feature = "game")]
fn game_settings() -> Vec<(String, String)> {
    let p = sona_core::game::GameParams::default();
    vec![
        ("game_presence_threshold".into(), p.presence_threshold.to_string()),
        ("game_segmenter_seed".into(), p.segmenter_seed.to_string()),
        ("game_steps".into(), p.steps.to_string()),
        ("game_boundary_threshold".into(), p.boundary_threshold.to_string()),
        ("game_deterministic_kernels".into(), "true".into()),
    ]
}

#[cfg(not(feature = "game"))]
fn game_settings() -> Vec<(String, String)> {
    Vec::new()
}

#[cfg(feature = "game")]
#[allow(clippy::too_many_arguments)]
fn cell_stage(annotation: &mut sona_core::Annotation, transcriber: &Transcriber, lead: &[f32], vocals_path: &str, game_dir: &str, model_dir: &str, division: u32, args: &TranscribeArgs) -> Result<(), String> {
    use sona_core::tempo::grid_phase;
    use sona_core::game::{cell_stage_notes, cells_covering, grid_spacing_s, phase_of, stage_annotation_notes, CellModel, GameModel, GameParams, StageRuns, GAME_SAMPLE_RATE};
    let (vocals, sr) = load_wav_mono(vocals_path)?;
    if sr != GAME_SAMPLE_RATE || annotation.sample_rate != GAME_SAMPLE_RATE {
        return Err(format!("the cell stage needs {GAME_SAMPLE_RATE} Hz stems"));
    }
    let spacing = grid_spacing_s(annotation.bpm, division);
    // The grid phase from the vocals' onsets (ADR-0027); sona's vocals notes are decoded on it so every
    // candidate shares one grid.
    let decode = |anchor_s: Option<f64>| Transcriber { grid: Some(GridMode { anchor_s, ..grid_mode(division, args) }), ..Transcriber::default() }.transcribe(&PYin::default(), &vocals, sr, annotation.bpm, Producer::Pyin);
    let (on_vocals, anchor) = match grid_phase(&vocals, sr, spacing) {
        Some(phase) => (decode(Some(phase)), phase),
        None => {
            let a = decode(None);
            let notes: Vec<(f64, f64, i32)> = a.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
            let phase = phase_of(&notes, spacing);
            (a, phase)
        }
    };
    let sona: Vec<(f64, f64, i32)> = on_vocals.notes.iter().map(|n| (n.onset_s, n.offset_s, i32::from(n.midi_pitch))).collect();
    let f0: Vec<(f64, f64)> = on_vocals.f0_contour.iter().map(|f| (f.time_s, f64::from(f.frequency_hz))).collect();
    let rms: Vec<(f64, f64)> = on_vocals.energy_envelope.iter().map(|e| (e.time_s, f64::from(e.rms))).collect();
    let game = GameModel::load(Path::new(game_dir), GameParams::default()).map_err(|e| e.to_string())?;
    let model = CellModel::load(Path::new(model_dir), GameParams::default().threads).map_err(|e| e.to_string())?;
    let runs = StageRuns::compute(&game, lead, &vocals, sr).map_err(|e| e.to_string())?;
    let duration = lead.len().max(vocals.len()) as f64 / f64::from(sr);
    let stage = cell_stage_notes(&model, &runs, &sona, &f0, &rms, spacing, anchor, cells_covering(duration, spacing, anchor)).map_err(|e| e.to_string())?;
    annotation.notes = stage_annotation_notes(&stage, &transcriber.scorer, &annotation.energy_envelope);
    annotation.producer = Producer::Game;
    Ok(())
}

#[cfg(not(feature = "game"))]
#[allow(clippy::too_many_arguments)]
fn cell_stage(_: &mut sona_core::Annotation, _: &Transcriber, _: &[f32], _: &str, _: &str, _: &str, _: u32, _: &TranscribeArgs) -> Result<(), String> {
    Err("--cell-model needs sona built with `--features game`".into())
}

/// Replace the annotation's notes with GAME's, snapped to the grid (ADR-0017).
#[cfg(feature = "game")]
fn game_notes(annotation: &mut sona_core::Annotation, transcriber: &Transcriber, samples: &[f32], dir: &str, division: u32, octave_check: bool) -> Result<(), String> {
    use sona_core::game::{correct_octaves, grid_spacing_s, snap_to_grid, GameModel, GameParams, OctaveRule, GAME_SAMPLE_RATE};
    if annotation.sample_rate != GAME_SAMPLE_RATE {
        return Err(format!(
            "--game needs a {GAME_SAMPLE_RATE} Hz stem, got {} Hz (resample it first, e.g. ffmpeg -i in.wav -ar {GAME_SAMPLE_RATE} out.wav)",
            annotation.sample_rate
        ));
    }
    let model = GameModel::load(Path::new(dir), GameParams::default()).map_err(|e| e.to_string())?;
    let mut notes = model.transcribe(samples, annotation.sample_rate).map_err(|e| e.to_string())?;
    if octave_check {
        let moved = correct_octaves(&mut notes, samples, annotation.sample_rate, OctaveRule::default());
        if moved > 0 {
            eprintln!("sona: octave check raised {moved} GAME notes by an octave");
        }
    }
    let segments = snap_to_grid(&notes, grid_spacing_s(annotation.bpm, division));
    annotation.notes = transcriber.scorer.score_all(&segments, &annotation.energy_envelope);
    annotation.producer = Producer::Game;
    Ok(())
}

#[cfg(not(feature = "game"))]
fn game_notes(_: &mut sona_core::Annotation, _: &Transcriber, _: &[f32], _: &str, _: u32, _: bool) -> Result<(), String> {
    Err("--game needs sona built with `--features game` (cargo build --release -p sona-cli --features game)".into())
}

/// Grid mode with the requested evidence (ADR-0015, ADR-0016).
fn grid_mode(division: u32, args: &TranscribeArgs) -> GridMode {
    let mut mode = GridMode { division, ..GridMode::sixteenths() };
    // Both refinements are grid mode's defaults (ADR-0016, confirmed on holdout2).
    if !args.salience {
        mode.decoder.candidates = None;
    }
    if !args.spectral_cue {
        mode.decoder.cue_model = CueModel::VoicingEnergy;
    }
    mode
}

/// Hand-rolled parse of the arguments `transcribe_synopsis!` lists. Accepts both `--flag value` and
/// `--flag=value` forms; the stem is the lone positional.
fn parse_transcribe_args(args: &[String]) -> Result<TranscribeArgs, String> {
    let mut stem: Option<String> = None;
    let mut bpm: Option<f64> = None;
    let mut midi: Option<String> = None;
    let mut grid: Option<u32> = None;
    let (mut salience, mut spectral_cue) = (true, cfg!(feature = "spectral"));
    let (mut game, mut octave_check): (Option<String>, bool) = (None, true);
    let (mut vocals, mut cell_model): (Option<String>, Option<String>) = (None, None);
    let mut bar_one: Option<String> = None;
    let mut tracker: Option<String> = None;
    let (mut beat_this_program, mut beat_this_threads, mut tool_timeout): (Option<String>, Option<u32>, Option<f64>) = (None, None, None);

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if let Some(rest) = arg.strip_prefix("--bpm") {
            let (raw, next) = flag_value(args, i, rest, "--bpm")?;
            i = next;
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("--bpm must be a number, got '{raw}'"))?;
            if !(value.is_finite() && value > 0.0) {
                return Err(format!("--bpm must be positive, got '{raw}'"));
            }
            bpm = Some(value);
        } else if let Some(rest) = arg.strip_prefix("--grid") {
            let (raw, next) = flag_value(args, i, rest, "--grid")?;
            let value: u32 = raw
                .parse()
                .map_err(|_| format!("--grid must be 16 or 24, got '{raw}'"))?;
            if value != 16 && value != 24 {
                return Err(format!("--grid must be 16 or 24, got '{raw}'"));
            }
            grid = Some(value);
            i = next;
        } else if let Some(rest) = arg.strip_prefix("--game") {
            let (raw, next) = flag_value(args, i, rest, "--game")?;
            i = next;
            game = Some(raw.to_string());
        } else if let Some(rest) = arg.strip_prefix("--vocals") {
            let (raw, next) = flag_value(args, i, rest, "--vocals")?;
            i = next;
            vocals = Some(raw);
        } else if let Some(rest) = arg.strip_prefix("--bar-one-tracker") {
            let (raw, next) = flag_value(args, i, rest, "--bar-one-tracker")?;
            i = next;
            if raw != "harmonic" && raw != "beat-this" {
                return Err(format!("--bar-one-tracker must be harmonic or beat-this, got '{raw}'"));
            }
            tracker = Some(raw);
        } else if let Some(rest) = arg.strip_prefix("--bar-one") {
            let (raw, next) = flag_value(args, i, rest, "--bar-one")?;
            i = next;
            bar_one = Some(raw);
        } else if let Some(rest) = arg.strip_prefix("--beat-this-threads") {
            let (raw, next) = flag_value(args, i, rest, "--beat-this-threads")?;
            i = next;
            beat_this_threads = Some(raw.parse().ok().filter(|n: &u32| *n >= 1).ok_or_else(|| format!("--beat-this-threads must be a whole number, at least 1, got '{raw}'"))?);
        } else if let Some(rest) = arg.strip_prefix("--beat-this") {
            let (raw, next) = flag_value(args, i, rest, "--beat-this")?;
            i = next;
            beat_this_program = Some(raw);
        } else if let Some(rest) = arg.strip_prefix("--tool-timeout") {
            let (raw, next) = flag_value(args, i, rest, "--tool-timeout")?;
            i = next;
            tool_timeout = Some(raw.parse().ok().filter(|s: &f64| s.is_finite() && *s > 0.0).ok_or_else(|| format!("--tool-timeout must be a positive number of seconds, got '{raw}'"))?);
        } else if let Some(rest) = arg.strip_prefix("--cell-model") {
            let (raw, next) = flag_value(args, i, rest, "--cell-model")?;
            i = next;
            cell_model = Some(raw);
        } else if arg == "--no-octave-check" {
            octave_check = false;
        } else if arg == "--no-salience" {
            salience = false;
        } else if arg == "--no-spectral-cue" {
            spectral_cue = false;
        } else if let Some(rest) = arg.strip_prefix("--midi") {
            let (value, next) = flag_value(args, i, rest, "--midi")?;
            i = next;
            midi = Some(value);
        } else if arg.starts_with('-') {
            return Err(format!("unknown option '{arg}'"));
        } else if stem.is_none() {
            stem = Some(arg.clone());
        } else {
            return Err(format!("unexpected argument '{arg}'"));
        }
        i += 1;
    }

    let stem = stem.ok_or(concat!("transcribe needs a <STEM> path\n\nUSAGE:\n    ", transcribe_synopsis!()))?;
    let bpm = bpm.ok_or("transcribe needs --bpm <BPM>")?;
    let refinement = args.iter().any(|a| matches!(a.as_str(), "--no-salience" | "--no-spectral-cue"));
    if refinement && grid.is_none() {
        return Err("--no-salience / --no-spectral-cue refine grid mode; add --grid 16 or --grid 24".into());
    }
    if game.is_some() && grid.is_none() {
        return Err("--game snaps GAME's notes to the grid; add --grid 16 or --grid 24".into());
    }
    if !octave_check && game.is_none() {
        return Err("--no-octave-check only applies with --game".into());
    }
    if cell_model.is_some() && (game.is_none() || vocals.is_none()) {
        return Err("--cell-model needs --game DIR and --vocals VOCALS.wav (the stem is then the lead)".into());
    }
    if vocals.is_some() && cell_model.is_none() && bar_one.is_none() {
        return Err("--vocals is only used with --cell-model or --bar-one".into());
    }
    if tracker.is_some() && bar_one.is_none() {
        return Err("--bar-one-tracker chooses where --bar-one MIX takes the bar from; add --bar-one".into());
    }
    let beat_this = if tracker.as_deref() == Some("beat-this") {
        let program = beat_this_program.map_or_else(|| beat_this::in_venv(&default_separator_venv()), std::path::PathBuf::from);
        Some(BeatThisInput::Run { program, threads: beat_this_threads.unwrap_or(1), timeout_s: tool_timeout })
    } else {
        if beat_this_program.is_some() || beat_this_threads.is_some() || tool_timeout.is_some() {
            return Err("--beat-this, --beat-this-threads and --tool-timeout are for --bar-one-tracker beat-this".into());
        }
        None
    };
    Ok(TranscribeArgs { stem, bpm, midi, grid, salience, spectral_cue, game, octave_check, vocals, cell_model, bar_one, beat_this })
}

/// The lead separator's venv, where `scripts/setup-separators.sh beat-this` puts Beat This.
pub(crate) fn default_separator_venv() -> std::path::PathBuf {
    std::env::var_os("SONA_LEAD_VENV").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(".")).join(".local/share/sona/separator-venv")
    })
}

/// Resolve a flag's value at index `i`, where `rest` is whatever followed the
/// flag name (`""` for a separate value, `=value` for the inline form). Returns
/// the value and the index the value was consumed from (so the caller advances
/// past a separate value). Rejects a name run together with junk (e.g. `--bpmx`).
fn flag_value(
    args: &[String],
    i: usize,
    rest: &str,
    name: &str,
) -> Result<(String, usize), String> {
    if let Some(inline) = rest.strip_prefix('=') {
        Ok((inline.to_string(), i))
    } else if rest.is_empty() {
        let value = args
            .get(i + 1)
            .cloned()
            .ok_or_else(|| format!("{name} expects a value"))?;
        Ok((value, i + 1))
    } else {
        Err(format!("unknown option '{name}{rest}'"))
    }
}

/// Decode a WAV file to mono `f32` samples in `-1.0..=1.0`, returning them with
/// the sample rate. Integer PCM is normalized by its full-scale value; multi-
/// channel audio is downmixed by averaging (a vocal stem is effectively mono, so
/// an average is the honest reduction).
fn load_wav_mono(path: &str) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("opening '{path}': {e}"))?;
    let spec = reader.spec();

    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| format!("reading '{path}': {e}"))?,
        hound::SampleFormat::Int => {
            // Normalize by full-scale for the actual bit depth (e.g. 32768 for
            // 16-bit), so the samples land in -1.0..=1.0 regardless of depth.
            let full_scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 / full_scale))
                .collect::<Result<_, _>>()
                .map_err(|e| format!("reading '{path}': {e}"))?
        }
    };

    let channels = spec.channels.max(1) as usize;
    let mono = if channels == 1 {
        interleaved
    } else {
        interleaved
            .chunks(channels)
            .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    if mono.is_empty() {
        return Err(format!("'{path}' contains no audio samples"));
    }
    Ok((mono, spec.sample_rate))
}

/// Write the annotation's notes as a raw MIDI file at `path`.
fn write_midi_file(annotation: &sona_core::Annotation, path: &str) -> Result<(), String> {
    let file = File::create(Path::new(path)).map_err(|e| format!("creating '{path}': {e}"))?;
    sona_core::write_midi(annotation, BufWriter::new(file))
        .map_err(|e| format!("writing MIDI to '{path}': {e}"))
}
