//! The stem cache (ADR-0030): a song's separated stems, kept by the upload's hash and the separators'
//! versions and settings, so transcribing it again (another BPM, grid or engine) skips separation.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What an entry was made from. The SHA-256 of these fields names the entry.
pub struct StemKey {
    pub fields: BTreeMap<String, String>,
    pub hex: String,
}

impl StemKey {
    pub fn new(fields: BTreeMap<String, String>) -> Self {
        let json = serde_json::to_vec(&fields).unwrap_or_default();
        let hex = crate::provenance::fingerprint_bytes("stem-cache", "", &json).sha256;
        StemKey { fields, hex }
    }
}

pub struct StemCache {
    dir: PathBuf,
    keep: usize,
}

const KEY_FILE: &str = "key.json";
const USED_FILE: &str = "used";

fn is_entry_name(name: &str) -> bool {
    name.len() == 64 && name.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Hard-link `from` to `to` (a copy where links aren't possible), replacing `to`.
pub fn link_or_copy(from: &Path, to: &Path) -> std::io::Result<()> {
    let _ = fs::remove_file(to);
    fs::hard_link(from, to).or_else(|_| fs::copy(from, to).map(|_| ()))
}

impl StemCache {
    /// A cache in `dir` that keeps the `keep` entries used most recently; `keep == 0` turns it off.
    pub fn new(dir: PathBuf, keep: usize) -> Self {
        StemCache { dir, keep }
    }

    pub fn enabled(&self) -> bool {
        self.keep > 0
    }

    /// Link the entry's `files` into `dest` and return the job that separated them, or `None` on a miss.
    pub fn restore(&self, key: &StemKey, files: &[&str], dest: &Path) -> Option<String> {
        if !self.enabled() {
            return None;
        }
        let entry = self.dir.join(&key.hex);
        let record: serde_json::Value = serde_json::from_slice(&fs::read(entry.join(KEY_FILE)).ok()?).ok()?;
        if record["key"] != serde_json::json!(key.fields) || !files.iter().all(|f| entry.join(f).is_file()) {
            return None;
        }
        let by = record["separated_by"].as_str()?.to_string();
        for f in files {
            link_or_copy(&entry.join(f), &dest.join(f)).ok()?;
        }
        let _ = fs::write(entry.join(USED_FILE), crate::provenance::now_unix_s().to_string());
        Some(by)
    }

    /// Keep `files` from `src` under `key`, as separated by job `job_id`, then evict down to the limit.
    pub fn store(&self, key: &StemKey, files: &[&str], src: &Path, job_id: &str) -> Result<(), String> {
        if !self.enabled() {
            return Ok(());
        }
        let err = |e: std::io::Error| e.to_string();
        fs::create_dir_all(&self.dir).map_err(err)?;
        let partial = self.dir.join(format!(".partial-{job_id}"));
        let _ = fs::remove_dir_all(&partial);
        fs::create_dir_all(&partial).map_err(err)?;
        let filled = files.iter().try_for_each(|f| link_or_copy(&src.join(f), &partial.join(f))).and_then(|()| {
            let record = serde_json::json!({ "key": key.fields, "separated_by": job_id });
            fs::write(partial.join(KEY_FILE), record.to_string())?;
            fs::write(partial.join(USED_FILE), crate::provenance::now_unix_s().to_string())?;
            let entry = self.dir.join(&key.hex);
            let _ = fs::remove_dir_all(&entry);
            fs::rename(&partial, &entry)
        });
        if let Err(e) = filled {
            let _ = fs::remove_dir_all(&partial);
            return Err(e.to_string());
        }
        self.evict();
        Ok(())
    }

    /// Delete all but the `keep` entries used most recently, and any entry a crash left half-written.
    pub fn evict(&self) {
        let Ok(dir) = fs::read_dir(&self.dir) else { return };
        let mut entries = Vec::new();
        for e in dir.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(".partial-") {
                let _ = fs::remove_dir_all(e.path());
            } else if is_entry_name(&name) {
                let used = fs::metadata(e.path().join(USED_FILE)).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                entries.push((used, name));
            }
        }
        entries.sort_by(|a, b| b.cmp(a));
        for (_, name) in entries.into_iter().skip(self.keep) {
            let _ = fs::remove_dir_all(self.dir.join(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sona-stems-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn key(upload: &str) -> StemKey {
        StemKey::new([("upload_sha256".to_string(), upload.to_string())].into())
    }

    #[test]
    fn stores_restores_and_keeps_the_most_recently_used() {
        let root = dir("cache");
        let job = root.join("job");
        fs::create_dir_all(&job).unwrap();
        let cache = StemCache::new(root.join("stems"), 2);
        for (i, k) in ["a", "b", "c"].iter().enumerate() {
            let _ = fs::remove_file(job.join("stem.wav"));
            fs::write(job.join("stem.wav"), format!("stem {k}")).unwrap();
            cache.store(&key(k), &["stem.wav"], &job, &format!("job{i}")).unwrap();
            if *k == "b" {
                std::thread::sleep(std::time::Duration::from_millis(20));
                let out = root.join("out");
                fs::create_dir_all(&out).unwrap();
                assert_eq!(cache.restore(&key("a"), &["stem.wav"], &out).as_deref(), Some("job0"));
                assert_eq!(fs::read_to_string(out.join("stem.wav")).unwrap(), "stem a");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let out = root.join("again");
        fs::create_dir_all(&out).unwrap();
        assert_eq!(cache.restore(&key("b"), &["stem.wav"], &out), None, "b was used least recently, so it went");
        assert_eq!(cache.restore(&key("a"), &["stem.wav"], &out).as_deref(), Some("job0"));
        assert_eq!(cache.restore(&key("c"), &["stem.wav"], &out).as_deref(), Some("job2"));
        assert_eq!(cache.restore(&key("c"), &["stem.wav", "vocals.wav"], &out), None, "an entry without every file is a miss");
        assert_eq!(StemCache::new(root.join("stems"), 0).restore(&key("c"), &["stem.wav"], &out), None, "keep 0 is off");
        let _ = fs::remove_dir_all(&root);
    }
}
