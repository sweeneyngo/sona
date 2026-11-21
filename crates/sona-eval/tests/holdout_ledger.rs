//! The holdout ledger refuses an unexplained or repeated read of a held-out split,
//! logs every read it allows, and leaves `dev` alone. Runs against a temporary
//! manifest (its own process, so the env var can't race other tests).

use sona_eval::holdout::{authorize, ledger_path};
use sona_eval::ust_corpus::{manifest_path, Manifest};

fn song(id: &str, split: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "expected_title": id, "ust_zip": "x.zip", "original_url": "u", "cover_video_url": "c",
        "upload_date": "2026-01-01", "stratum": "vocaloid-original", "voice": "v", "stratum_evidence": "e",
        "stem": {}, "split": split
    })
}

#[test]
fn reads_need_a_reason_are_logged_once_and_rereads_are_refused() {
    let dir = std::env::temp_dir().join(format!("sona-ledger-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ust-manifest.json");
    let m = serde_json::json!({
        "_doc": "", "channel": "", "selected_on": "", "ust_share_url": "", "ust_terms": "", "excluded": [],
        "songs": [song("a", "dev"), song("b", "holdout2"), song("c", "holdout2")]
    });
    std::fs::write(&path, m.to_string()).unwrap();
    std::env::set_var("SONA_UST_MANIFEST", &path);
    assert_eq!(manifest_path(), path);
    let manifest = Manifest::load(&path).unwrap();

    assert!(authorize(&manifest, "holdout2", "test", None, false).unwrap_err().contains("--reason"));
    assert!(authorize(&manifest, "nope", "test", Some("why"), false).is_err(), "an empty split is not a holdout");
    assert!(!ledger_path().exists(), "a refused read logs nothing");

    authorize(&manifest, "holdout2", "test", Some("confirm X"), false).expect("first read is allowed");
    let log = std::fs::read_to_string(ledger_path()).unwrap();
    assert_eq!(log.lines().count(), 1);
    assert!(log.contains("\"set\":\"b,c\"") && log.contains("confirm X"), "{log}");

    let again = authorize(&manifest, "holdout2", "test", Some("one more look"), false).unwrap_err();
    assert!(again.contains("already read 1 time"), "{again}");
    authorize(&manifest, "holdout2", "test", Some("deliberate re-read"), true).expect("--reread is allowed");
    assert_eq!(std::fs::read_to_string(ledger_path()).unwrap().lines().count(), 2, "and logged");
    std::fs::remove_dir_all(&dir).ok();
}
