//! Settling near ties between GAME's two listens (utau-edit-cost 18). A GAME note edge rounds to the
//! nearest grid cell, and when the raw time sits close to halfway, a few milliseconds of jitter put it a
//! cell early or late. A one-cell legato boundary error costs the user both notes' fixes in OpenUtau. Where
//! GAME's raw time on the lead lies at least [`NearTieRule::min_lean`] of a cell to one side of its
//! rounded edge, and GAME's run on the full vocals puts an edge on the neighbouring cell that side, the
//! edge moves there. The prototype and its dev numbers: `crates/sona-eval/scripts/timing-errors/`.

use crate::cellstage::{snap_notes_indexed, StageNote};
use std::collections::{HashMap, HashSet};

/// When an edge counts as a near tie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearTieRule {
    /// How far (in cells) the raw lead time must lie to one side of its snapped edge.
    pub min_lean: f64,
}

impl Default for NearTieRule {
    fn default() -> Self {
        Self { min_lean: 0.35 }
    }
}

/// `notes` (onset, offset, MIDI) on the grid of `spacing` seconds at `anchor`, with each edge that sits on
/// a near tie of GAME's lead notes `lead` moved one cell toward the raw time when GAME's vocals notes
/// `second` have an edge on that cell. `lead` and `second` are GAME's raw notes, snapped here as
/// [`crate::cellstage::snap_notes`] snaps them; a lead edge's lean is its raw time minus the cell it
/// rounded to, and where a note starts on the cell an earlier note ends on, the start's lean counts.
/// Every edge on a moving cell moves, so a legato boundary moves as one. Two moves that would cross (onto
/// each other's cells) are both dropped, and a cell whose move would leave a note without a cell stays put
/// for every note on it, so the line stays one voice, on the grid, with every note kept.
pub fn settle_near_ties(notes: &[(f64, f64, i32)], lead: &[StageNote], second: &[StageNote], spacing: f64, anchor: f64, rule: NearTieRule) -> Vec<(f64, f64, i32)> {
    let cell = |t: f64| ((t - anchor) / spacing).round_ties_even() as i64;
    let snapped = snap_notes_indexed(lead, spacing, anchor);
    let mut lean: HashMap<i64, f64> = HashMap::new();
    // Ends first, then starts, so a start's lean wins on a shared cell. An edge the one-voice trim moved off
    // its own rounding (more than half a cell from the raw time) is no tie.
    for edges in [snapped.iter().map(|&(_, off, _, i)| (off, lead[i].offset_s)).collect::<Vec<_>>(), snapped.iter().map(|&(on, _, _, i)| (on, lead[i].onset_s)).collect()] {
        for (edge, raw) in edges {
            let k = cell(edge);
            let r = (raw - anchor) / spacing - k as f64;
            if r.abs() <= 0.5 {
                lean.insert(k, r);
            }
        }
    }
    let edges: HashSet<i64> = crate::cellstage::snap_notes(second, spacing, anchor).iter().flat_map(|&(on, off, _)| [cell(on), cell(off)]).collect();
    let wanted: HashMap<i64, i64> = lean
        .iter()
        .filter(|(_, r)| r.abs() >= rule.min_lean)
        .map(|(&k, &r)| (k, if r > 0.0 { k + 1 } else { k - 1 }))
        .filter(|(_, to)| edges.contains(to))
        .collect();
    let mut moves: HashMap<i64, i64> = wanted.iter().filter(|(_, to)| !wanted.contains_key(to)).map(|(&k, &to)| (k, to)).collect();
    let spans: Vec<(i64, i64)> = notes.iter().map(|&(on, off, _)| (cell(on), cell(off))).collect();
    let moved = |moves: &HashMap<i64, i64>, k: i64| moves.get(&k).copied().unwrap_or(k);
    // A cell whose move would leave a note without a cell does not move, for any note on it.
    loop {
        let emptied: Vec<i64> = spans.iter().filter(|&&(a, b)| moved(&moves, b) <= moved(&moves, a)).flat_map(|&(a, b)| [a, b]).filter(|k| moves.contains_key(k)).collect();
        if emptied.is_empty() {
            break;
        }
        for k in emptied {
            moves.remove(&k);
        }
    }
    notes
        .iter()
        .zip(&spans)
        .map(|(&(_, _, pitch), &(a, b))| (anchor + moved(&moves, a) as f64 * spacing, anchor + moved(&moves, b) as f64 * spacing, pitch))
        .collect()
}
