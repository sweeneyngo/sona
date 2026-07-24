//! **Vocadito loader** (Zenodo 5578807) — parse the dataset's annotations into
//! the common [`Reference`] type.
//!
//! Two annotation kinds, both headerless comma-separated:
//! - **f0**: `time_s, frequency_hz` — frequency `<= 0` marks an unvoiced frame.
//! - **notes**: `onset_s, pitch_hz, duration_s` — two annotator versions
//!   (`notesA1`, `notesA2`); we take A1 as primary and fall back to A2. Pitch is
//!   in **Hz** and rounded to the nearest MIDI note here.
//!
//! The corpus is discovered by **filename pattern**, not a hard-coded directory
//! layout: we walk the root for `vocadito_<id>.wav` and match each track's
//! annotations by name. That keeps the loader working regardless of how the
//! archive nests `Audio/` and `Annotations/`, which the dataset page doesn't
//! pin down. The corpus itself is never committed (ADR-0011); it is fetched into
//! a git-ignored dir by `scripts/fetch-vocadito.sh`.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::EvalError;
use crate::reference::{RefF0, RefNote, Reference};

/// The `SONA_VOCADITO_DIR` override; when unset the corpus is expected under the
/// crate's git-ignored `data/vocadito` (where the fetch script puts it).
pub const CORPUS_ENV: &str = "SONA_VOCADITO_DIR";

/// Resolve where the Vocadito corpus should live: `$SONA_VOCADITO_DIR` if set,
/// else the crate-local `data/vocadito`. Existence is *not* checked here — the
/// loader checks and fails loudly, so callers get one clear error path.
pub fn corpus_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(CORPUS_ENV) {
        return PathBuf::from(dir);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vocadito")
}

/// One discovered track and the annotation files found for it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackPaths {
    /// Track id (the `<id>` in `vocadito_<id>.wav`).
    pub id: String,
    /// The stem audio.
    pub audio: PathBuf,
    /// The f0 annotation CSV, if present.
    pub f0: Option<PathBuf>,
    /// The primary (A1) note annotation CSV, if present.
    pub notes_a1: Option<PathBuf>,
    /// The secondary (A2) note annotation CSV, if present.
    pub notes_a2: Option<PathBuf>,
}

/// The discovered corpus: its root and the tracks under it, sorted by id.
#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    /// The corpus root that was scanned.
    pub root: PathBuf,
    /// Discovered tracks, sorted by numeric-then-lexical id.
    pub tracks: Vec<TrackPaths>,
}

/// Discover the Vocadito corpus under `root`. Fails loudly with
/// [`EvalError::CorpusMissing`] when `root` doesn't exist — the expected
/// first-run state before the fetch script has run.
pub fn discover(root: &Path) -> Result<Corpus, EvalError> {
    if !root.is_dir() {
        return Err(EvalError::CorpusMissing(root.to_path_buf()));
    }

    let mut files = Vec::new();
    collect_files(root, &mut files)?;

    let mut tracks: Vec<TrackPaths> = Vec::new();
    for path in &files {
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // A track is anchored by its audio file `vocadito_<id>.wav`.
        if let Some(id) = name.strip_prefix("vocadito_").and_then(|s| s.strip_suffix(".wav")) {
            let f0 = find_annotation(&files, id, |n| n.ends_with("_f0.csv"));
            let notes_a1 = find_annotation(&files, id, |n| n.contains("notesA1"));
            let notes_a2 = find_annotation(&files, id, |n| n.contains("notesA2"));
            tracks.push(TrackPaths {
                id: id.to_string(),
                audio: path.clone(),
                f0,
                notes_a1,
                notes_a2,
            });
        }
    }

    // A present-but-empty root is as unusable as a missing one — fail loudly
    // rather than silently report zero tracks (which would read as a clean pass).
    if tracks.is_empty() {
        return Err(EvalError::CorpusMissing(root.to_path_buf()));
    }

    tracks.sort_by(|a, b| track_id_key(&a.id).cmp(&track_id_key(&b.id)));
    Ok(Corpus { root: root.to_path_buf(), tracks })
}

/// Load a track's annotations into a [`Reference`]. Notes come from A1 (falling
/// back to A2); f0 from the f0 CSV. Missing annotation files yield empty
/// collections rather than an error, so a partially-annotated track still loads.
pub fn load_reference(track: &TrackPaths) -> Result<Reference, EvalError> {
    let notes = match track.notes_a1.as_ref().or(track.notes_a2.as_ref()) {
        Some(path) => parse_notes_csv(&read(path)?, path)?,
        None => Vec::new(),
    };
    let f0 = match track.f0.as_ref() {
        Some(path) => parse_f0_csv(&read(path)?, path)?,
        None => Vec::new(),
    };
    Ok(Reference { notes, f0 })
}

/// Parse a Vocadito f0 CSV (`time_s, frequency_hz`). A frequency `<= 0` is an
/// unvoiced frame (stored as `0.0`, `voiced = false`). Blank lines are skipped.
pub fn parse_f0_csv(text: &str, path: &Path) -> Result<Vec<RefF0>, EvalError> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cols = line.split(',');
        let time_s = parse_col(&mut cols, path, line_no, "time")?;
        let freq = parse_col(&mut cols, path, line_no, "frequency")?;
        let voiced = freq > 0.0;
        out.push(RefF0 {
            time_s,
            frequency_hz: if voiced { freq as f32 } else { 0.0 },
            voiced,
        });
    }
    Ok(out)
}

/// Parse a Vocadito notes CSV (`onset_s, pitch_hz, duration_s`). Pitch is in Hz
/// and rounded to the nearest MIDI note; the interval is `[onset, onset+dur]`.
/// Blank lines are skipped.
pub fn parse_notes_csv(text: &str, path: &Path) -> Result<Vec<RefNote>, EvalError> {
    let mut out = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cols = line.split(',');
        let onset_s = parse_col(&mut cols, path, line_no, "onset")?;
        let pitch_hz = parse_col(&mut cols, path, line_no, "pitch")?;
        let duration_s = parse_col(&mut cols, path, line_no, "duration")?;
        if pitch_hz <= 0.0 {
            return Err(EvalError::parse(
                path,
                format!("line {}: note pitch must be a positive Hz value", line_no + 1),
            ));
        }
        out.push(RefNote::from_hz(onset_s, onset_s + duration_s, pitch_hz as f32));
    }
    Ok(out)
}

/// Parse the next comma-separated column as an `f64`, with a located error.
fn parse_col<'a>(
    cols: &mut impl Iterator<Item = &'a str>,
    path: &Path,
    line_no: usize,
    field: &str,
) -> Result<f64, EvalError> {
    let raw = cols.next().map(str::trim).unwrap_or("");
    raw.parse::<f64>().map_err(|_| {
        EvalError::parse(
            path,
            format!("line {}: expected a number for {field}, got '{raw}'", line_no + 1),
        )
    })
}

/// Find the annotation file for `id` matching `pred`. Matches files whose name
/// starts with the exact track prefix `vocadito_<id>_`, so track `1` never
/// captures track `10`'s files.
fn find_annotation(
    files: &[PathBuf],
    id: &str,
    pred: impl Fn(&str) -> bool,
) -> Option<PathBuf> {
    let prefix = format!("vocadito_{id}_");
    files
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix) && pred(n))
        })
        .cloned()
}

/// Read a file to a string, tagging I/O errors with the path.
fn read(path: &Path) -> Result<String, EvalError> {
    fs::read_to_string(path).map_err(|e| EvalError::io(path, e))
}

/// Recursively collect regular files under `dir` (std-only, no walkdir dep).
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), EvalError> {
    for entry in fs::read_dir(dir).map_err(|e| EvalError::io(dir, e))? {
        let entry = entry.map_err(|e| EvalError::io(dir, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else {
            out.push(path);
        }
    }
    Ok(())
}

/// Sort key for track ids: numeric ids sort numerically (so `2` precedes `10`),
/// anything else falls back to lexical.
fn track_id_key(id: &str) -> (u64, String) {
    match id.parse::<u64>() {
        Ok(n) => (n, String::new()),
        Err(_) => (u64::MAX, id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> PathBuf {
        PathBuf::from("test.csv")
    }

    #[test]
    fn parses_f0_with_unvoiced_frames() {
        let csv = "0.00,440.0\n0.01,0.0\n0.02,-1.0\n0.03,441.5\n";
        let f0 = parse_f0_csv(csv, &p()).unwrap();
        assert_eq!(f0.len(), 4);
        assert!(f0[0].voiced && (f0[0].frequency_hz - 440.0).abs() < 1e-3);
        // Zero and negative frequencies are unvoiced, normalized to 0.0 Hz.
        assert!(!f0[1].voiced && f0[1].frequency_hz == 0.0);
        assert!(!f0[2].voiced && f0[2].frequency_hz == 0.0);
        assert!(f0[3].voiced);
    }

    #[test]
    fn parses_notes_hz_to_midi_and_offset() {
        // 440 Hz -> A4 (69); duration 0.5 -> offset 0.75. 523.25 Hz -> C5 (72).
        let csv = "0.25,440.0,0.5\n1.0,523.25,0.25\n";
        let notes = parse_notes_csv(csv, &p()).unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].midi_pitch, 69);
        assert!((notes[0].offset_s - 0.75).abs() < 1e-9);
        assert_eq!(notes[1].midi_pitch, 72);
    }

    #[test]
    fn skips_blank_lines() {
        assert_eq!(parse_f0_csv("\n0.0,440.0\n\n", &p()).unwrap().len(), 1);
        assert_eq!(parse_notes_csv("0.0,440.0,0.1\n\n", &p()).unwrap().len(), 1);
    }

    #[test]
    fn malformed_line_is_a_located_error() {
        let err = parse_f0_csv("0.0,440.0\nbogus\n", &p()).unwrap_err();
        assert!(matches!(err, EvalError::Parse { .. }));
        assert!(err.to_string().contains("line 2"));
    }

    #[test]
    fn non_positive_note_pitch_is_rejected() {
        assert!(parse_notes_csv("0.0,0.0,0.1\n", &p()).is_err());
    }

    #[test]
    fn discover_missing_root_fails_loudly() {
        let err = discover(Path::new("/no/such/vocadito/dir")).unwrap_err();
        assert!(matches!(err, EvalError::CorpusMissing(_)));
    }

    #[test]
    fn discover_empty_root_fails_loudly() {
        // A present-but-empty dir must fail too, not silently report zero tracks.
        let dir = std::env::temp_dir().join(format!("sona-eval-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let result = discover(&dir);
        std::fs::remove_dir_all(&dir).ok();
        assert!(matches!(result, Err(EvalError::CorpusMissing(_))));
    }

    #[test]
    fn track_id_key_orders_numerically() {
        let mut ids = vec!["10", "2", "1"];
        ids.sort_by_key(|id| track_id_key(id));
        assert_eq!(ids, vec!["1", "2", "10"]);
    }
}
