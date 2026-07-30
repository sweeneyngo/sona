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

use sona_core::{PYin, Producer, Transcriber};

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
}

/// The `transcribe` subcommand: parse args, decode the stem, run the pipeline,
/// emit JSON (and optional MIDI). Returns a human-readable message on any error.
fn run_transcribe(args: &[String]) -> Result<(), String> {
    let parsed = parse_transcribe_args(args)?;

    let (samples, sample_rate) = load_wav_mono(&parsed.stem)?;
    let annotation = Transcriber::default().transcribe(
        &PYin::default(),
        &samples,
        sample_rate,
        parsed.bpm,
        Producer::Pyin,
    );

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

/// Hand-rolled parse of `<STEM> --bpm <BPM> [--midi <PATH>]`. Accepts both
/// `--flag value` and `--flag=value` forms; the stem is the lone positional.
fn parse_transcribe_args(args: &[String]) -> Result<TranscribeArgs, String> {
    let mut stem: Option<String> = None;
    let mut bpm: Option<f64> = None;
    let mut midi: Option<String> = None;

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

    let stem = stem.ok_or("transcribe needs a <STEM> path\n\nUSAGE:\n    sona transcribe <STEM> --bpm <BPM> [--midi <PATH>]")?;
    let bpm = bpm.ok_or("transcribe needs --bpm <BPM>")?;
    Ok(TranscribeArgs { stem, bpm, midi })
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
