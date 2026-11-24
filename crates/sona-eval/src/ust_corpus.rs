//! **The UST corpus** — the committed manifest (`corpus/ust-manifest.json`) and
//! loading each song's UST reference from the git-ignored cache.
//!
//! The manifest is the single source of truth for the in-domain gate: which 20
//! songs (the most recent `@pifuyuu` uploads, skip-and-backfill), their stratum,
//! which UST files are the lead and which are other voices, the exact BPM and
//! grid from the UST, the per-song alignment, and the stem's separation
//! provenance. UST content itself is never committed.
//!
//! **Other voices matter for scoring.** Nearly every song has harmony that
//! overlaps the lead (and demucs cannot separate lead from harmony), so the
//! gate also scores **monophonic regions** — stretches where only the lead
//! sounds — using the other-voice USTs to find them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::EvalError;
use crate::ust::{BlockKind, Ust, UstReference};

/// Where separated stems live: `$SONA_UST_STEMS_DIR`, else the crate's
/// git-ignored `data/pifuyuu-stems` (written by `scripts/separate-stems.sh`).
pub fn stems_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("SONA_UST_STEMS_DIR") {
        return PathBuf::from(p);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("data/pifuyuu-stems")
}

/// The committed manifest's default location.
pub fn manifest_path() -> PathBuf {
    if let Some(p) = std::env::var_os("SONA_UST_MANIFEST") {
        return PathBuf::from(p);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/ust-manifest.json")
}

/// The corpus manifest. Field order mirrors the committed JSON so a rewrite
/// produces a minimal diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "_doc")]
    pub doc: String,
    pub channel: String,
    pub selected_on: String,
    pub ust_share_url: String,
    pub ust_terms: String,
    pub songs: Vec<Song>,
    pub excluded: Vec<Exclusion>,
}

/// One included song.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// Stable ASCII slug; also the song's cache directory name.
    pub id: String,
    pub expected_title: String,
    /// The zip's name in the shared UST folder.
    pub ust_zip: String,
    pub ust_zip_sha256: Option<String>,
    /// The lead line: `main*` plus a sung `intro`. Merged into one reference.
    pub ust_lead_files: Option<Vec<String>>,
    /// Every other voice audible in the original (harmony, doubling,
    /// delay/echo, backing vocalise, spoken parts). Not scored; used to find
    /// monophonic regions.
    pub ust_other_voice_files: Option<Vec<String>>,
    /// The ORIGINAL song (not the UTAU cover). YouTube or niconico.
    pub original_url: String,
    pub original_title: Option<String>,
    pub original_duration_s: Option<f64>,
    pub cover_video_url: String,
    /// Channel order; the selection key.
    pub upload_date: String,
    /// `vocaloid-original` (any synthesized vocal — the gate) or `human-sung`.
    pub stratum: String,
    pub voice: String,
    pub stratum_evidence: String,
    /// Exact tempo from the lead UST's `[#SETTING]`.
    pub bpm_from_ust: Option<f64>,
    /// Quantization grid for this song: 16 (sixteenths) or 24 (16th-triplets),
    /// whichever holds the majority of the lead's note starts.
    pub grid_division: Option<u32>,
    /// `stem_time = ust_time + alignment_offset_s` (ticket 05).
    pub alignment_offset_s: Option<f64>,
    /// `aligned`, or `flagged: <reason>` — flagged songs are excluded from the
    /// gate (ticket 05), never silently scored.
    #[serde(default)]
    pub alignment_status: Option<String>,
    pub stem: Stem,
    /// `dev` (iterate and tune on these) or `holdout` (scored once, at the end).
    /// Assigned by upload order before any tuning: odd positions dev, even holdout.
    #[serde(default)]
    pub split: Option<String>,
    /// Whole cells from the UST's beat lines to the music's beat, in `0..cells per beat`
    /// (utau-edit-cost 10; `sona_eval::frame::MusicFrame`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_beat_shift_cells: Option<u32>,
    /// Whole cells from the UST's bar lines (every 4 beats from its tick 0) to the music's bar line,
    /// in `0..cells per bar`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_bar_shift_cells: Option<u32>,
    /// `unverified-estimate` (seeded from `reports/bar-one.md`) or `ear-checked`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_frame_status: Option<String>,
    pub notes: Option<String>,
}

impl Song {
    /// The music's beat and bar against the UST's lines, when the manifest has both shifts.
    pub fn music_frame(&self) -> Option<crate::frame::MusicFrame> {
        Some(crate::frame::MusicFrame {
            beat_shift_cells: self.music_beat_shift_cells?,
            bar_shift_cells: self.music_bar_shift_cells?,
            ear_checked: self.music_frame_status.as_deref() == Some("ear-checked"),
        })
    }
}

/// Stem provenance (ticket 03).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Stem {
    pub model: Option<String>,
    pub version: Option<String>,
    pub path: Option<String>,
}

/// A skipped upload and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exclusion {
    pub title: String,
    pub cover_video_url: String,
    pub reason: String,
}

impl Manifest {
    /// Load the manifest.
    pub fn load(path: &Path) -> Result<Manifest, EvalError> {
        let text = std::fs::read_to_string(path).map_err(|e| EvalError::io(path, e))?;
        serde_json::from_str(&text).map_err(|e| EvalError::parse(path, e.to_string()))
    }

    /// Write it back (2-space pretty JSON, non-ASCII kept as-is, trailing newline).
    pub fn save(&self, path: &Path) -> Result<(), EvalError> {
        let mut text = serde_json::to_string_pretty(self).map_err(|e| EvalError::parse(path, e.to_string()))?;
        text.push('\n');
        std::fs::write(path, text).map_err(|e| EvalError::io(path, e))
    }
}

/// A song's USTs loaded from the cache.
#[derive(Debug, Clone)]
pub struct SongUsts {
    /// The lead files, parsed (in manifest order).
    pub lead: Vec<Ust>,
    /// Every other voice, parsed.
    pub other: Vec<Ust>,
}

impl SongUsts {
    /// Load a song's lead and other-voice USTs from `cache/<id>/`.
    pub fn load(song: &Song, cache: &Path) -> Result<SongUsts, EvalError> {
        let dir = cache.join(&song.id);
        let load_all = |files: &Option<Vec<String>>| -> Result<Vec<Ust>, EvalError> {
            files.iter().flatten().map(|f| Ust::load(&dir.join(f))).collect()
        };
        let lead = load_all(&song.ust_lead_files)?;
        if lead.is_empty() {
            return Err(EvalError::parse(&dir, format!("{}: no lead UST listed in the manifest", song.id)));
        }
        Ok(SongUsts { lead, other: load_all(&song.ust_other_voice_files)? })
    }

    /// Every voice's sung spans (lead and other voices) on the stem timeline,
    /// unmerged — what the vocal stem should contain.
    pub fn all_voice_spans(&self, offset_s: f64) -> Vec<(f64, f64)> {
        self.lead
            .iter()
            .chain(&self.other)
            .flat_map(|u| u.timeline())
            .filter(|p| p.kind == BlockKind::Note)
            .map(|p| (p.start_s + offset_s, p.end_s + offset_s))
            .collect()
    }

    /// The merged lead reference on the stem timeline.
    pub fn reference(&self, offset_s: f64) -> UstReference {
        UstReference::merge(self.lead.iter().map(|u| u.to_reference(offset_s)))
    }

    /// Time spans (stem seconds, merged and sorted) where **more than one voice**
    /// sounds: any other-voice note, or two lead parts overlapping. Outside
    /// these spans the lead is the only voice — a monophonic region.
    pub fn polyphonic_spans(&self, offset_s: f64) -> Vec<(f64, f64)> {
        let mut spans: Vec<(f64, f64)> = Vec::new();
        for u in &self.other {
            for p in u.timeline().iter().filter(|p| p.kind == BlockKind::Note) {
                spans.push((p.start_s + offset_s, p.end_s + offset_s));
            }
        }
        // Lead-on-lead overlaps (split leads / duets).
        let leads: Vec<Vec<(f64, f64)>> = self
            .lead
            .iter()
            .map(|u| u.to_reference(offset_s).notes.iter().map(|n| (n.onset_s, n.offset_s)).collect())
            .collect();
        for (i, a) in leads.iter().enumerate() {
            for b in leads.iter().skip(i + 1) {
                for &(s1, e1) in a {
                    for &(s2, e2) in b {
                        let (s, e) = (s1.max(s2), e1.min(e2));
                        if e > s {
                            spans.push((s, e));
                        }
                    }
                }
            }
        }
        merge_spans(spans)
    }
}

/// Sort and merge overlapping/touching spans.
pub fn merge_spans(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(spans.len());
    for (s, e) in spans {
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Whether `[start, end)` overlaps any span in `spans` (sorted, merged).
pub fn overlaps_any(spans: &[(f64, f64)], start: f64, end: f64) -> bool {
    // Spans are sorted: find the first span ending after `start`.
    let i = spans.partition_point(|&(_, e)| e <= start);
    spans.get(i).is_some_and(|&(s, _)| s < end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_overlapping_and_touching_spans() {
        let merged = merge_spans(vec![(3.0, 4.0), (0.0, 1.0), (0.5, 2.0), (2.0, 2.5)]);
        assert_eq!(merged, vec![(0.0, 2.5), (3.0, 4.0)]);
    }

    #[test]
    fn overlap_query_is_half_open() {
        let spans = [(1.0, 2.0), (5.0, 6.0)];
        assert!(overlaps_any(&spans, 1.5, 1.6));
        assert!(overlaps_any(&spans, 0.5, 1.1));
        assert!(!overlaps_any(&spans, 2.0, 5.0), "touching is not overlapping");
        assert!(!overlaps_any(&spans, 6.0, 7.0));
        assert!(overlaps_any(&spans, 0.0, 10.0));
    }

    #[test]
    fn committed_manifest_parses_and_round_trips() {
        let path = manifest_path();
        let m = Manifest::load(&path).expect("committed manifest");
        // No silent truncation: each split keeps its songs (holdout2 has 10 aligned
        // songs plus the 2 it flagged and backfilled; holdout3 needed no backfill).
        let count = |split: &str, aligned_only: bool| {
            m.songs
                .iter()
                .filter(|s| s.split.as_deref() == Some(split))
                .filter(|s| !aligned_only || s.alignment_status.as_deref().is_some_and(|a| a.starts_with("aligned")))
                .count()
        };
        assert_eq!(
            (count("dev", false), count("holdout", false), count("holdout2", false), count("holdout3", false)),
            (10, 10, 12, 10)
        );
        assert_eq!(count("holdout2", true), 10, "holdout2 scores 10 aligned songs");
        assert_eq!(count("holdout3", true), 10, "holdout3 scores 10 aligned songs");
        // holdout4 is unread: its offsets were set without sona and never checked with sona's
        // onsets, so none of its songs may read as aligned until that check is made at read time.
        assert_eq!((count("holdout4", false), count("holdout4-tempo", false)), (24, 0));
        for s in m.songs.iter().filter(|s| s.split.as_deref().is_some_and(|x| x.starts_with("holdout4"))) {
            assert!(s.alignment_offset_s.is_some(), "{}: no offset", s.id);
            assert!(s.alignment_status.as_deref().is_some_and(|a| a.starts_with("unverified")), "{}: {:?}", s.id, s.alignment_status);
        }
        assert!(
            m.songs.iter().all(|s| matches!(
                s.split.as_deref(),
                Some("dev" | "holdout" | "holdout2" | "holdout3" | "holdout4" | "holdout4-tempo")
            )),
            "unknown split"
        );
        let again: Manifest = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, again);
        let mut pretty = serde_json::to_string_pretty(&m).unwrap();
        pretty.push('\n');
        assert_eq!(pretty, std::fs::read_to_string(&path).unwrap(), "save() rewrites the committed manifest byte for byte");
        for s in &m.songs {
            let shifts = (s.music_beat_shift_cells.is_some(), s.music_bar_shift_cells.is_some(), s.music_frame_status.is_some());
            assert!(matches!(shifts, (false, false, false) | (true, true, true)), "{}: music frame fields come together", s.id);
            if let Some(music) = s.music_frame() {
                assert!(matches!(s.music_frame_status.as_deref(), Some("unverified-estimate" | "ear-checked")), "{}", s.id);
                let cells_per_beat = s.grid_division.unwrap_or(16) / 4;
                assert!(music.is_consistent(cells_per_beat), "{}: the bar line must fall on the beat: {music:?}", s.id);
            }
            assert!(matches!(s.stratum.as_str(), "vocaloid-original" | "human-sung"), "{}: {}", s.id, s.stratum);
            assert!(!s.original_url.contains("youtube.com/@pifuyuu"), "{}: original must not be the cover", s.id);
            assert_ne!(s.original_url, s.cover_video_url, "{}: original must not be the cover", s.id);
        }
    }
}
