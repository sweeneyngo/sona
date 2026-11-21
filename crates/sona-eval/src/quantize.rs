//! **Hard-snap quantization** — a faithful Rust port of the review surface's
//! `web/lib/grid.ts` (`gridSpacingSeconds` / `snapTime` / `quantizeNote`), so the
//! harness can score *quantized* output against a tick-quantized UST.
//!
//! **Temporary home, by design** ([ADR-0014]). `sona-core` emits raw perception
//! ([ADR-0004]); quantization is interpretation and lives UI-side. Scoring the
//! deliverable needs it in Rust, so it is duplicated here — in the harness, not
//! the core. Drift between the two copies is caught by the shared case file
//! `fixtures/quantize/cases.json`, which both this module's tests and
//! `web/lib/grid.test.ts` assert against.
//!
//! **Written to be promotable**: pure functions over plain numbers, depending on
//! nothing in `sona-eval` (std only), so moving it into `sona-core` — or exposing
//! it to the UI via WASM, making the core the single source of truth — is a file
//! move plus a re-export.
//!
//! The confidence-aware escape hatch ([ADR-0003]) is ported exactly: a note
//! whose onset lands ≥ `escape_distance_cells` of a cell from the nearest line
//! *and* whose onset clarity is ≥ `onset_confidence_threshold` is left in place
//! and marked `escaped`.
//!
//! [ADR-0003]: ../../../docs/adr/0003-hard-snap-quantization-for-vocaloid.md
//! [ADR-0004]: ../../../docs/adr/0004-core-emits-raw-perception-ui-interprets.md
//! [ADR-0014]: ../../../docs/adr/0014-quantizer-temporary-home-in-sona-eval.md

/// `grid.ts` default: a note must sit ≥ this fraction of a cell off-grid to escape.
pub const DEFAULT_ESCAPE_DISTANCE_CELLS: f64 = 0.25;
/// `grid.ts` default: onset clarity at or above this lets a far-off note escape.
pub const DEFAULT_ONSET_CONFIDENCE_THRESHOLD: f32 = 0.8;

/// Grid + escape-hatch settings (`grid.ts` `QuantizeOptions`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizeOptions {
    /// Seconds per grid cell (see [`grid_spacing_seconds`]).
    pub spacing: f64,
    /// Grid phase: lines sit at `anchor + k·spacing`.
    pub anchor: f64,
    /// Minimum off-grid distance, in cells, for the escape hatch.
    pub escape_distance_cells: f64,
    /// Minimum onset clarity for the escape hatch.
    pub onset_confidence_threshold: f32,
}

impl QuantizeOptions {
    /// `grid.ts` defaults for everything but the grid itself.
    pub fn new(spacing: f64, anchor: f64) -> Self {
        QuantizeOptions {
            spacing,
            anchor,
            escape_distance_cells: DEFAULT_ESCAPE_DISTANCE_CELLS,
            onset_confidence_threshold: DEFAULT_ONSET_CONFIDENCE_THRESHOLD,
        }
    }
}

/// A quantized note (`grid.ts` `QuantizedNote`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizedNote {
    /// Snapped (or, if escaped, original) onset, seconds.
    pub onset_s: f64,
    /// Snapped (or, if escaped, original) offset, seconds.
    pub offset_s: f64,
    /// The escape hatch spared this note from snapping.
    pub escaped: bool,
}

/// Seconds per cell for a note-value `division` (4 = quarter, 16 = sixteenth,
/// 24 = sixteenth-triplet, …) at `bpm`: `240 / (bpm × division)`.
pub fn grid_spacing_seconds(bpm: f64, division: f64) -> f64 {
    240.0 / (bpm * division)
}

/// Nearest grid line to `t`, where lines sit at `anchor + k·spacing`.
pub fn snap_time(t: f64, spacing: f64, anchor: f64) -> f64 {
    anchor + js_round((t - anchor) / spacing) * spacing
}

/// Hard-snap a note's onset and offset, with the confidence-aware escape hatch.
/// `onset_clarity` is the note's onset sub-score; `None` reads as 0 (as
/// `grid.ts` does for a missing sub-score), so such a note never escapes. The
/// offset is pushed out to keep at least a one-cell note.
pub fn quantize_note(
    onset_s: f64,
    offset_s: f64,
    onset_clarity: Option<f32>,
    opts: &QuantizeOptions,
) -> QuantizedNote {
    let snapped_onset = snap_time(onset_s, opts.spacing, opts.anchor);
    let deviation_cells = (onset_s - snapped_onset).abs() / opts.spacing;
    let clarity = onset_clarity.unwrap_or(0.0);

    if deviation_cells >= opts.escape_distance_cells && clarity >= opts.onset_confidence_threshold {
        return QuantizedNote { onset_s, offset_s, escaped: true };
    }

    let mut offset = snap_time(offset_s, opts.spacing, opts.anchor);
    if offset <= snapped_onset + 1e-9 {
        offset = snapped_onset + opts.spacing;
    }
    QuantizedNote { onset_s: snapped_onset, offset_s: offset, escaped: false }
}

/// `grid.ts` `resolveOverlaps`: keep a snapped line monophonic. Walking in onset
/// order, a note that starts inside its predecessor trims the predecessor to its
/// onset; a note on the predecessor's onset line moves to the predecessor's end and
/// keeps at least one cell. No note is dropped; input order is kept.
pub fn resolve_overlaps(notes: &mut [QuantizedNote], spacing: f64) {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|&a, &b| notes[a].onset_s.total_cmp(&notes[b].onset_s).then(a.cmp(&b)));
    for k in 1..order.len() {
        let (p, n) = (order[k - 1], order[k]);
        if notes[n].onset_s >= notes[p].offset_s - 1e-9 {
            continue;
        }
        if notes[n].onset_s > notes[p].onset_s + 1e-9 {
            notes[p].offset_s = notes[n].onset_s;
        } else {
            notes[n].onset_s = notes[p].offset_s;
            if notes[n].offset_s <= notes[n].onset_s + 1e-9 {
                notes[n].offset_s = notes[n].onset_s + spacing;
            }
        }
    }
}

/// JavaScript's `Math.round`: halves round toward +∞ (`-2.5 → -2`), unlike
/// Rust's `f64::round` (away from zero, `-2.5 → -3`). Matters for a note exactly
/// half a cell before the anchor.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// One case in the shared `fixtures/quantize/cases.json` — the drift guard
    /// `web/lib/grid.test.ts` asserts too.
    #[derive(Deserialize)]
    struct Case {
        name: String,
        onset_s: f64,
        offset_s: f64,
        onset_clarity: Option<f32>,
        spacing: f64,
        anchor: f64,
        #[serde(rename = "escapeDistanceCells")]
        escape_distance_cells: Option<f64>,
        #[serde(rename = "onsetConfidenceThreshold")]
        onset_confidence_threshold: Option<f32>,
        expected: Expected,
    }

    #[derive(Deserialize)]
    struct Expected {
        onset_s: f64,
        offset_s: f64,
        escaped: bool,
    }

    fn cases() -> Vec<Case> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/quantize/cases.json");
        let text = std::fs::read_to_string(&path).expect("shared quantize cases");
        serde_json::from_str(&text).expect("parse cases.json")
    }

    #[test]
    fn agrees_with_grid_ts_on_the_shared_cases() {
        let cases = cases();
        assert!(cases.len() >= 10, "expected the shared case set");
        for c in cases {
            let mut opts = QuantizeOptions::new(c.spacing, c.anchor);
            if let Some(d) = c.escape_distance_cells {
                opts.escape_distance_cells = d;
            }
            if let Some(t) = c.onset_confidence_threshold {
                opts.onset_confidence_threshold = t;
            }
            let q = quantize_note(c.onset_s, c.offset_s, c.onset_clarity, &opts);
            assert_eq!(q.escaped, c.expected.escaped, "{}: escaped", c.name);
            assert!((q.onset_s - c.expected.onset_s).abs() < 1e-9, "{}: onset {} vs {}", c.name, q.onset_s, c.expected.onset_s);
            assert!((q.offset_s - c.expected.offset_s).abs() < 1e-9, "{}: offset {} vs {}", c.name, q.offset_s, c.expected.offset_s);
        }
    }

    #[derive(Deserialize)]
    struct Span {
        onset_s: f64,
        offset_s: f64,
    }

    #[derive(Deserialize)]
    struct SequenceCase {
        name: String,
        spacing: f64,
        notes: Vec<Span>,
        expected: Vec<Span>,
    }

    #[test]
    fn resolve_overlaps_agrees_with_grid_ts_on_the_shared_cases() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/quantize/sequence_cases.json");
        let cases: Vec<SequenceCase> = serde_json::from_str(&std::fs::read_to_string(&path).expect("cases")).expect("parse");
        assert!(cases.len() >= 5);
        for c in cases {
            let mut q: Vec<QuantizedNote> =
                c.notes.iter().map(|n| QuantizedNote { onset_s: n.onset_s, offset_s: n.offset_s, escaped: false }).collect();
            resolve_overlaps(&mut q, c.spacing);
            for (i, (g, e)) in q.iter().zip(&c.expected).enumerate() {
                assert!((g.onset_s - e.onset_s).abs() < 1e-9, "{}: onset {i}", c.name);
                assert!((g.offset_s - e.offset_s).abs() < 1e-9, "{}: offset {i}", c.name);
            }
        }
    }

    #[test]
    fn spacing_matches_grid_ts() {
        assert!((grid_spacing_seconds(120.0, 16.0) - 0.125).abs() < 1e-12);
        assert!((grid_spacing_seconds(60.0, 4.0) - 1.0).abs() < 1e-12);
        // A 16th-triplet at 150 BPM: a third of a beat split in two = 66.7 ms.
        assert!((grid_spacing_seconds(150.0, 24.0) - 0.4 / 6.0).abs() < 1e-12);
    }

    #[test]
    fn snap_rounds_halves_like_javascript() {
        // (t - anchor)/spacing = -0.5 → JS Math.round gives -0 (line at anchor),
        // Rust f64::round would give -1.
        assert!((snap_time(0.0, 0.1, 0.05) - 0.05).abs() < 1e-12);
        assert!((snap_time(0.17, 0.1, 0.05) - 0.15).abs() < 1e-12);
    }
}
