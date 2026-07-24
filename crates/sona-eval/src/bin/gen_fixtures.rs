//! Regenerate the committed golden fixtures: for each spec, write `<name>.wav`
//! and `<name>.ref.json` into the fixtures dir (or a directory passed as the
//! first argument). Run after changing `synth::fixture_specs`.
//!
//!     cargo run -p sona-eval --bin sona-eval-gen-fixtures

use std::path::PathBuf;
use std::process::ExitCode;

use sona_eval::fixtures::fixtures_dir;
use sona_eval::synth::fixture_specs;
use sona_eval::wav;

fn main() -> ExitCode {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(fixtures_dir);

    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("sona-eval-gen-fixtures: {}: {e}", out_dir.display());
        return ExitCode::FAILURE;
    }

    for spec in fixture_specs() {
        let wav_path = out_dir.join(format!("{}.wav", spec.name));
        let ref_path = out_dir.join(format!("{}.ref.json", spec.name));

        if let Err(e) = wav::write_mono_16bit(&wav_path, &spec.render_samples()) {
            eprintln!("sona-eval-gen-fixtures: {e}");
            return ExitCode::FAILURE;
        }
        let json = match serde_json::to_string_pretty(&spec.reference()) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("sona-eval-gen-fixtures: serializing {}: {e}", spec.name);
                return ExitCode::FAILURE;
            }
        };
        if let Err(e) = std::fs::write(&ref_path, json + "\n") {
            eprintln!("sona-eval-gen-fixtures: {}: {e}", ref_path.display());
            return ExitCode::FAILURE;
        }
        println!("wrote {} and {}", wav_path.display(), ref_path.display());
    }
    ExitCode::SUCCESS
}
