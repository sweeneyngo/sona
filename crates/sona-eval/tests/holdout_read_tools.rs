//! The tools a holdout read uses (utau-edit-cost 17) go through the ledger: `sona-eval-frame
//! --estimate-bar-one --split` and the alignment check of a split whose songs are still unverified refuse
//! a read without a reason, log the one they allow, refuse a second, and leave dev alone. Each case runs
//! the real binary against its own temporary manifest and ledger.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn song(id: &str, split: &str, status: Option<&str>) -> serde_json::Value {
    let mut s = serde_json::json!({
        "id": id, "expected_title": id, "ust_zip": "x.zip", "original_url": "u", "cover_video_url": "c",
        "upload_date": "2026-01-01", "stratum": "vocaloid-original", "voice": "v", "stratum_evidence": "e",
        "stem": {}, "split": split, "bpm_from_ust": 120.0, "grid_division": 16, "alignment_offset_s": -4.5
    });
    if let Some(status) = status {
        s["alignment_status"] = status.into();
    }
    s
}

/// A manifest with one dev song and two holdout songs whose alignment is not verified yet, in a fresh folder.
fn corpus(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sona-read-tools-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("stems")).unwrap();
    let m = serde_json::json!({
        "_doc": "", "channel": "", "selected_on": "", "ust_share_url": "", "ust_terms": "", "excluded": [],
        "songs": [
            song("a", "dev", Some("aligned")),
            song("b", "held", Some("unverified (set without sona; drum peak 3.00)")),
            song("c", "held", Some("unverified (set without sona; drum peak 4.00)")),
        ]
    });
    std::fs::write(dir.join("ust-manifest.json"), m.to_string()).unwrap();
    dir
}

fn run(bin: &str, dir: &Path, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .env("SONA_UST_MANIFEST", dir.join("ust-manifest.json"))
        .env("SONA_UST_STEMS_DIR", dir.join("stems"))
        .env("SONA_UST_DIR", dir.join("ust"))
        .env("SONA_UST_AUDIO_DIR", dir.join("audio"))
        .output()
        .unwrap()
}

fn ledger(dir: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join("holdout-reads.jsonl")).unwrap_or_default().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

const FRAME: &str = env!("CARGO_BIN_EXE_sona-eval-frame");
const ALIGN: &str = env!("CARGO_BIN_EXE_sona-eval-ust-align");

#[test]
fn bar_one_on_a_held_out_split_is_one_logged_read() {
    let dir = corpus("frame");
    let refused = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--reason"));
    assert!(ledger(&dir).is_empty(), "a refused read logs nothing");

    let read = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held", "--reason", "confirm bar 1"]);
    assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    let log = ledger(&dir);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0]["set"], "b,c");
    assert_eq!(log[0]["tool"], "sona-eval-frame --estimate-bar-one");
    assert_eq!(log[0]["reread"], false);

    let again = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held", "--reason", "one more look"]);
    assert!(!again.status.success() && String::from_utf8_lossy(&again.stderr).contains("already read"));
    let same_sitting = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held", "--reason", "same sitting", "--reread"]);
    assert!(same_sitting.status.success());
    assert_eq!(ledger(&dir).len(), 2);

    let dev = run(FRAME, &dir, &["--estimate-bar-one"]);
    assert!(dev.status.success());
    assert_eq!(ledger(&dir).len(), 2, "dev is never logged");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn ear_labels_that_put_the_bar_off_the_beat_stop_the_run_before_the_read() {
    let dir = corpus("labels");
    let labels = dir.join("labels.json");
    std::fs::write(&labels, r#"{"songs": {"b": {"meter": "4/4", "beat_shift_cells": 1, "bar_shift_cells": 6}}}"#).unwrap();
    let bad = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held", "--music-frames", labels.to_str().unwrap(), "--reason", "r"]);
    assert!(!bad.status.success());
    std::fs::write(&labels, r#"{"songs": {"b": {"meter": "4/4", "beat_shift_cells": 1}}}"#).unwrap();
    let incomplete = run(FRAME, &dir, &["--estimate-bar-one", "--split", "held", "--music-frames", labels.to_str().unwrap(), "--reason", "r"]);
    assert!(!incomplete.status.success() && String::from_utf8_lossy(&incomplete.stderr).contains("bar_shift_cells"));
    assert!(ledger(&dir).is_empty(), "a bad label file must not spend the holdout");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn checking_an_unverified_split_is_the_logged_first_step_of_its_read() {
    let dir = corpus("align");
    let everything = run(ALIGN, &dir, &[]);
    assert!(everything.status.success(), "{}", String::from_utf8_lossy(&everything.stderr));
    let out = String::from_utf8_lossy(&everything.stdout);
    assert!(out.lines().any(|l| l.starts_with('b') && l.contains("unverified split")), "{out}");
    assert!(ledger(&dir).is_empty(), "without --split the check skips unverified songs and logs nothing");

    let refused = run(ALIGN, &dir, &["--split", "held"]);
    assert!(!refused.status.success() && String::from_utf8_lossy(&refused.stderr).contains("--reason"));
    assert!(ledger(&dir).is_empty());

    let offsets_only = run(ALIGN, &dir, &["--split", "held", "--no-verify"]);
    assert!(offsets_only.status.success());
    assert!(ledger(&dir).is_empty(), "setting offsets without sona is not a read");

    let check = run(ALIGN, &dir, &["--split", "held", "--reason", "holdout read, step 1"]);
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    let log = ledger(&dir);
    assert_eq!(log.len(), 1);
    assert_eq!((log[0]["tool"].as_str(), log[0]["set"].as_str()), (Some("sona-eval-ust-align"), Some("b,c")));
    std::fs::remove_dir_all(&dir).ok();
}
