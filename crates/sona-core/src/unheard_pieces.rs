//! Merging one-cell pieces that a second listen does not hear (utau-edit-cost 11). GAME chops a held
//! note at a scoop, a glide or a wobble into a one-cell piece a step or two away from the note, and the
//! user pays a split-note fix for each. Sona's pitch track can't tell such a piece from a real short
//! note; GAME's run on the full vocals can, a little: where it has no note edge at the cut, the piece
//! is more often part of its neighbour. Such a piece joins that neighbour and takes its pitch. The
//! prototype and its dev numbers: `crates/sona-eval/scripts/pitch-wobble/`.

use crate::lyric_fit::CellNote;
use std::collections::{HashMap, HashSet};

/// Steps of an octave or more are octave errors or real leaps, never a wobble.
pub const MAX_STEP: i32 = 11;

/// `notes`: one voice on a grid of cells, sorted. `other_edges`: the cells where a second
/// transcription (GAME on the full vocals, on the same grid) starts or ends a note. A one-cell note
/// touching a neighbour 1..=[`MAX_STEP`] semitones away, with no other edge at the cut between them,
/// joins that neighbour. A note that could join either side joins the smaller step, the later
/// neighbour on a tie; of two touching one-cell notes the earlier one joins. A merged run takes the
/// pitch of its longest note (the later one on a tie).
pub fn merge_unheard_pieces(notes: &[CellNote], other_edges: &[i64]) -> Vec<CellNote> {
    let edges: HashSet<i64> = other_edges.iter().copied().collect();
    let mut joins: HashMap<usize, ((i32, u8), usize)> = HashMap::new();
    for i in 1..notes.len() {
        let (a, b) = (notes[i - 1], notes[i]);
        let step = (b.pitch - a.pitch).abs();
        if a.end != b.start || step == 0 || step > MAX_STEP || edges.contains(&b.start) {
            continue;
        }
        let short_first = a.end - a.start <= b.end - b.start;
        let (short, len) = if short_first { (i - 1, a.end - a.start) } else { (i, b.end - b.start) };
        if len != 1 {
            continue;
        }
        let key = (step, u8::from(!short_first));
        if joins.get(&short).is_none_or(|&(best, _)| key < best) {
            joins.insert(short, (key, i));
        }
    }
    let cuts: HashSet<usize> = joins.values().map(|&(_, cut)| cut).collect();
    let mut out: Vec<CellNote> = Vec::with_capacity(notes.len());
    let mut longest = (0, 0);
    for (i, &note) in notes.iter().enumerate() {
        let len = note.end - note.start;
        match out.last_mut() {
            Some(last) if cuts.contains(&i) => {
                last.end = note.end;
                if len >= longest.0 {
                    longest = (len, note.pitch);
                    last.pitch = note.pitch;
                } else {
                    last.pitch = longest.1;
                }
            }
            _ => {
                out.push(note);
                longest = (len, note.pitch);
            }
        }
    }
    out
}

/// The grid cells where `spans` (onset, offset in seconds) start or end, rounded to the grid at `anchor`
/// as the stage's sources are (an empty span ends one cell after it starts).
pub fn edge_cells(spans: impl IntoIterator<Item = (f64, f64)>, spacing: f64, anchor: f64) -> Vec<i64> {
    let cell = |t: f64| ((t - anchor) / spacing).round_ties_even() as i64;
    spans
        .into_iter()
        .flat_map(|(on, off)| {
            let (a, b) = (cell(on), cell(off));
            [a, if b <= a { a + 1 } else { b }]
        })
        .collect()
}
