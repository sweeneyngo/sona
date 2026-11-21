//! The holdout ledger: reading any held-out split of the UST corpus (every split
//! but `dev`: `holdout`, `holdout2`, …) leaves a trace.
//!
//! The holdout exists to confirm a result once, after tuning on `dev`. A second
//! read of the same songs is how a tuning loop quietly starts fitting it, so
//! every read appends to `corpus/holdout-reads.jsonl` (committed) with its reason
//! and commit, and a re-read of an already-read song set is refused unless the
//! caller says `--reread` — which is logged too.

use std::io::Write;
use std::path::PathBuf;

use crate::ust_corpus::{manifest_path, Manifest};

/// The ledger beside the manifest.
pub fn ledger_path() -> PathBuf {
    manifest_path().with_file_name("holdout-reads.jsonl")
}

/// The song ids of a held-out `split`, sorted, joined: the identity of "this holdout".
pub fn holdout_set(manifest: &Manifest, split: &str) -> String {
    let mut ids: Vec<&str> = manifest.songs.iter().filter(|s| s.split.as_deref() == Some(split)).map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    ids.join(",")
}

/// Earlier reads of exactly this holdout set.
pub fn prior_reads(set: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(ledger_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v.get("set").and_then(|s| s.as_str()) == Some(set))
        .collect()
}

/// Allow a read of a held-out `split` (anything but `dev`) and log it, or explain why not.
pub fn authorize(manifest: &Manifest, split: &str, tool: &str, reason: Option<&str>, reread: bool) -> Result<(), String> {
    let set = holdout_set(manifest, split);
    if set.is_empty() {
        return Err(format!("the manifest has no songs in split {split:?}"));
    }
    let Some(reason) = reason.map(str::trim).filter(|r| !r.is_empty()) else {
        return Err("reading the holdout needs --reason \"<what this read confirms>\"; it is logged in corpus/holdout-reads.jsonl".into());
    };
    let prior = prior_reads(&set);
    if !prior.is_empty() && !reread {
        let last = prior.last().and_then(|v| v.get("commit")).and_then(|c| c.as_str()).unwrap_or("?");
        return Err(format!(
            "this holdout set was already read {} time(s), last at {last} (corpus/holdout-reads.jsonl). \
             Anything tuned since is contaminated against it: build a fresh holdout, or pass --reread to read it anyway (logged).",
            prior.len()
        ));
    }
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let entry = serde_json::json!({ "unix_time": when, "commit": commit, "tool": tool, "split": split, "reason": reason, "reread": reread, "set": set });
    let path = ledger_path();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(f, "{entry}").map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("holdout read logged in {} — commit the ledger with the result", path.display());
    Ok(())
}
