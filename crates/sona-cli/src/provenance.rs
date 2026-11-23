//! Provenance for annotations and `sona serve` jobs (ADR-0024): what produced a result, down to the
//! byte (SHA-256 of the input and of every model file) and the build (`git describe`).

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use sona_core::{FileFingerprint, Provenance};

pub fn commit() -> Option<String> {
    Some(env!("SONA_GIT_DESCRIBE")).filter(|s| !s.is_empty()).map(str::to_string)
}

fn hex(digest: impl AsRef<[u8]>) -> String {
    digest.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn fingerprint_file(role: &str, path: &Path) -> Result<FileFingerprint, String> {
    let mut f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(FileFingerprint { role: role.into(), name, bytes, sha256: hex(hasher.finalize()) })
}

pub fn fingerprint_bytes(role: &str, name: &str, data: &[u8]) -> FileFingerprint {
    FileFingerprint { role: role.into(), name: name.into(), bytes: data.len() as u64, sha256: hex(Sha256::digest(data)) }
}

pub fn now_unix_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn provenance(input: FileFingerprint, settings: BTreeMap<String, String>, models: Vec<FileFingerprint>) -> Provenance {
    Provenance { sona_version: sona_core::VERSION.into(), sona_commit: commit(), created_unix_s: now_unix_s(), input, settings, models, job: None }
}

/// What a command prints, or `None` if it fails, is missing, or takes longer than `limit`.
pub fn tool_stdout(cmd: &mut Command, limit: Duration) -> Option<String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if start.elapsed() > limit => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut out = String::new();
    child.stdout.take()?.read_to_string(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_match_sha256sum() {
        let f = fingerprint_bytes("input", "x", b"abc");
        assert_eq!(f.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let path = std::env::temp_dir().join(format!("sona-prov-{}", std::process::id()));
        std::fs::write(&path, vec![7u8; 3 << 20]).unwrap();
        let g = fingerprint_file("input", &path).unwrap();
        assert_eq!((g.bytes, g.sha256), (3 << 20, fingerprint_bytes("input", "x", &vec![7u8; 3 << 20]).sha256));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_tool_that_fails_hangs_or_is_missing_gives_nothing() {
        assert_eq!(tool_stdout(Command::new("sh").args(["-c", "echo a; echo b"]), Duration::from_secs(5)).as_deref(), Some("a\nb\n"));
        assert_eq!(tool_stdout(Command::new("sh").args(["-c", "exit 3"]), Duration::from_secs(5)), None);
        assert_eq!(tool_stdout(Command::new("sh").args(["-c", "sleep 5"]), Duration::from_millis(200)), None);
        assert_eq!(tool_stdout(&mut Command::new("/nonexistent/tool"), Duration::from_secs(1)), None);
    }
}
