//! `sona` — command-line front-end to the perception core.
//!
//! Scaffold (ticket 01): parses no real work yet, just prints usage. The
//! `emit annotation JSON` path lands in ticket 07; argument parsing (likely
//! `clap`) is introduced there rather than pre-committed here.

use std::process::ExitCode;

const USAGE: &str = "\
sona — vocal-to-MIDI perception core (CLI)

USAGE:
    sona <COMMAND> [OPTIONS]

COMMANDS:
    transcribe <STEM> --bpm <BPM>    Transcribe a vocal stem to annotation JSON
                                     (not yet implemented — ticket 07)

OPTIONS:
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
        // No command, or an explicit help request: print usage. Usage on an
        // empty invocation is the ticket's "CLI runs and prints usage" bar.
        None | Some("-h") | Some("--help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("sona: unknown command '{other}'\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}
