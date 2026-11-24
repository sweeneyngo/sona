//! Beat This! (CPJKU; code and weights MIT) as a user-installed tool, an opt-in source of bar 1
//! (ADR-0036). `scripts/setup-separators.sh beat-this` installs `scripts/sona-beat-this.py` into the lead
//! separator's venv as `bin/sona-beat-this`; sona runs it as a separate process on the full mix and reads
//! the downbeat times it writes. Nothing here links an ML runtime.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How to install the tool.
pub const SETUP_FIX: &str = "run scripts/setup-separators.sh beat-this";

/// Where the setup script puts the tool, inside the lead separator's venv.
pub fn in_venv(separator_venv: &Path) -> PathBuf {
    separator_venv.join("bin/sona-beat-this")
}

/// What the tool wrote: the downbeats sona votes with, and what to record about the run.
#[derive(Debug, Clone, PartialEq)]
pub struct Downbeats {
    pub downbeats: Vec<f64>,
    pub version: String,
    pub checkpoint: String,
    pub checkpoint_path: Option<PathBuf>,
}

/// The tool on `audio`, writing its JSON to `out`, pinned to `threads` CPU threads.
pub fn command(program: &Path, threads: u32, audio: &Path, out: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.arg("--threads").arg(threads.to_string()).arg("--out").arg(out).arg(audio);
    for var in ["OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS"] {
        cmd.env(var, threads.to_string());
    }
    cmd
}

/// The tool's JSON at `out`.
pub fn read(out: &Path) -> Result<Downbeats, String> {
    let text = fs::read_to_string(out).map_err(|e| format!("Beat This wrote no result ({}): {e}", out.display()))?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("Beat This's result isn't JSON ({}): {e}", out.display()))?;
    let downbeats = v["downbeats"]
        .as_array()
        .and_then(|a| a.iter().map(serde_json::Value::as_f64).collect::<Option<Vec<f64>>>())
        .ok_or_else(|| format!("Beat This's result has no list of downbeat times ({})", out.display()))?;
    let text_of = |k: &str| v[k].as_str().map_or_else(|| "unknown".to_string(), str::to_string);
    Ok(Downbeats { downbeats, version: text_of("beat_this"), checkpoint: text_of("checkpoint"), checkpoint_path: v["checkpoint_path"].as_str().map(PathBuf::from) })
}

pub fn not_installed(program: &Path, flag: &str) -> String {
    format!("Beat This isn't installed (no {}): {SETUP_FIX}, or name the tool with {flag} PROGRAM", program.display())
}

/// Run the tool to its end, or stop it at `limit` (`sona transcribe`; `sona serve` runs it as a job tool).
pub fn run(program: &Path, threads: u32, audio: &Path, limit: Duration) -> Result<Downbeats, String> {
    if !program.is_file() {
        return Err(not_installed(program, "--beat-this"));
    }
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("sona-beat-this-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = run_in(program, threads, audio, limit, &dir);
    let _ = fs::remove_dir_all(&dir);
    result
}

fn run_in(program: &Path, threads: u32, audio: &Path, limit: Duration, dir: &Path) -> Result<Downbeats, String> {
    let (out, log_path) = (dir.join("downbeats.json"), dir.join("beat-this.log"));
    let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut child = command(program, threads, audio, &out)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| format!("could not start Beat This ({}): {e}", program.display()))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Beat This was stopped after {:.0} s, its time limit. It last printed: {}. If the machine is only slow, give it longer with --tool-timeout SECONDS",
                limit.as_secs_f64(),
                last_lines(&log_path, 3)
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        return Err(format!("Beat This failed ({status}): {}", last_lines(&log_path, 8)));
    }
    read(&out)
}

fn last_lines(path: &Path, n: usize) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text.split(['\r', '\n']).map(str::trim).filter(|l| !l.is_empty()).collect();
    lines[lines.len().saturating_sub(n)..].join(" | ")
}
