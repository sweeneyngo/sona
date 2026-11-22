//! `sona` — command-line front-end to the perception core.
//!
//! `sona transcribe <STEM> --bpm <BPM> [--midi <PATH>]` decodes a WAV vocal stem,
//! runs the perception pipeline ([`sona_core::Transcriber`], pYIN backend), and
//! prints the annotation JSON contract to stdout. With `--midi` it also writes a
//! raw, unquantized `.mid` as a convenience — *not* the authoritative export,
//! which the web review surface produces from the JSON (ADR-0004).
//!
//! Argument parsing is hand-rolled: the surface is one subcommand and three
//! options, so a dependency-free parser keeps the build as lean as the core's.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::ExitCode;

use sona_core::{CueModel, GridMode, PYin, Producer, Transcriber};

const USAGE: &str = "\
sona — vocal-to-MIDI perception core (CLI)

USAGE:
    sona <COMMAND> [OPTIONS]

COMMANDS:
    transcribe <STEM> --bpm <BPM>    Transcribe a WAV vocal stem to annotation JSON
                                     (printed to stdout)

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
    -h, --help       Print this usage
    -V, --version    Print version

The core emits the annotation JSON contract (raw note events + confidence +
f0 contour + energy envelope); the web review surface interprets it (grid,
quantization, correction, MIDI export). See CONTEXT.md.";

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
        Some("transcribe") => match run_transcribe(&args[1..]) {
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
}

/// The `transcribe` subcommand: parse args, decode the stem, run the pipeline,
/// emit JSON (and optional MIDI). Returns a human-readable message on any error.
fn run_transcribe(args: &[String]) -> Result<(), String> {
    let parsed = parse_transcribe_args(args)?;

    let (samples, sample_rate) = load_wav_mono(&parsed.stem)?;
    // With --game, GAME supplies the notes; the default pipeline still supplies
    // the f0 contour and energy envelope the review surface draws, so skip its
    // (replaced) grid decode.
    let grid = if parsed.game.is_some() { None } else { parsed.grid.map(|division| grid_mode(division, &parsed)) };
    let transcriber = Transcriber { grid, ..Transcriber::default() };
    #[allow(unused_mut)]
    let mut annotation = transcriber.transcribe(
        &PYin::default(),
        &samples,
        sample_rate,
        parsed.bpm,
        Producer::Pyin,
    );

    if let (Some(dir), Some(division)) = (&parsed.game, parsed.grid) {
        game_notes(&mut annotation, &transcriber, &samples, dir, division, parsed.octave_check)?;
    }

    // The JSON contract is the primary artifact: it goes to stdout so the command
    // composes in a pipeline.
    let json = serde_json::to_string_pretty(&annotation)
        .map_err(|e| format!("serializing annotation: {e}"))?;
    println!("{json}");

    if let Some(midi_path) = parsed.midi {
        write_midi_file(&annotation, &midi_path)?;
        // Progress on stderr keeps stdout pure JSON.
        eprintln!(
            "sona: wrote raw MIDI ({} notes) to {midi_path}",
            annotation.notes.len()
        );
    }
    Ok(())
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

/// Hand-rolled parse of `<STEM> --bpm <BPM> [--midi <PATH>] [--grid <16|24>]`. Accepts both
/// `--flag value` and `--flag=value` forms; the stem is the lone positional.
fn parse_transcribe_args(args: &[String]) -> Result<TranscribeArgs, String> {
    let mut stem: Option<String> = None;
    let mut bpm: Option<f64> = None;
    let mut midi: Option<String> = None;
    let mut grid: Option<u32> = None;
    let (mut salience, mut spectral_cue) = (true, cfg!(feature = "spectral"));
    let (mut game, mut octave_check): (Option<String>, bool) = (None, true);

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
        } else if arg == "--salience" || arg == "--spectral-cue" {
            // Accepted for compatibility: both are on by default now.
        } else if let Some(rest) = arg.strip_prefix("--game") {
            let (raw, next) = flag_value(args, i, rest, "--game")?;
            i = next;
            game = Some(raw.to_string());
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

    let stem = stem.ok_or("transcribe needs a <STEM> path\n\nUSAGE:\n    sona transcribe <STEM> --bpm <BPM> [--midi <PATH>] [--grid <16|24> [--no-salience] [--no-spectral-cue]]")?;
    let bpm = bpm.ok_or("transcribe needs --bpm <BPM>")?;
    let refinement = args.iter().any(|a| matches!(a.as_str(), "--salience" | "--spectral-cue" | "--no-salience" | "--no-spectral-cue"));
    if refinement && grid.is_none() {
        return Err("--salience / --spectral-cue (and their --no- forms) refine grid mode; add --grid 16 or --grid 24".into());
    }
    if game.is_some() && grid.is_none() {
        return Err("--game snaps GAME's notes to the grid; add --grid 16 or --grid 24".into());
    }
    if !octave_check && game.is_none() {
        return Err("--no-octave-check only applies with --game".into());
    }
    Ok(TranscribeArgs { stem, bpm, midi, grid, salience, spectral_cue, game, octave_check })
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
