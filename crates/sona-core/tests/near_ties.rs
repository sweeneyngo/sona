//! `sona_core::near_ties`: an edge GAME rounded from nearly halfway moves to the neighbouring cell when its
//! second listen puts the edge there, and nowhere else.

use sona_core::cellstage::StageNote;
use sona_core::near_ties::{settle_near_ties, NearTieRule};

const CELL: f64 = 0.1;

fn raw(on_cells: f64, off_cells: f64, midi: f32) -> StageNote {
    StageNote { onset_s: on_cells * CELL, offset_s: off_cells * CELL, midi, voiced: true }
}

fn note(start: i64, end: i64, pitch: i32) -> (f64, f64, i32) {
    (start as f64 * CELL, end as f64 * CELL, pitch)
}

fn cells(notes: &[(f64, f64, i32)]) -> Vec<(i64, i64, i32)> {
    notes.iter().map(|&(a, b, p)| ((a / CELL).round() as i64, (b / CELL).round() as i64, p)).collect()
}

fn settle(notes: &[(f64, f64, i32)], lead: &[StageNote], second: &[StageNote]) -> Vec<(i64, i64, i32)> {
    cells(&settle_near_ties(notes, lead, second, CELL, 0.0, NearTieRule::default()))
}

/// Two notes meeting where GAME heard the change 0.4 cell after cell 3, so it rounded to 3.
fn lead_leaning_late() -> Vec<StageNote> {
    vec![raw(0.0, 3.4, 60.0), raw(3.4, 6.0, 62.0)]
}

#[test]
fn a_boundary_rounded_from_nearly_halfway_moves_where_the_second_listen_hears_it() {
    let second = [raw(0.0, 4.1, 60.0), raw(4.1, 6.0, 62.0)];
    let got = settle(&[note(0, 3, 60), note(3, 6, 62)], &lead_leaning_late(), &second);
    assert_eq!(got, vec![(0, 4, 60), (4, 6, 62)], "both notes keep meeting, one cell later");
}

#[test]
fn a_second_listen_that_agrees_with_the_rounding_leaves_the_boundary() {
    let second = [raw(0.0, 2.9, 60.0), raw(2.9, 6.0, 62.0)];
    assert_eq!(settle(&[note(0, 3, 60), note(3, 6, 62)], &lead_leaning_late(), &second), vec![(0, 3, 60), (3, 6, 62)]);
}

#[test]
fn an_edge_that_rounded_cleanly_stays_even_when_the_second_listen_differs() {
    let lead = [raw(0.0, 3.2, 60.0), raw(3.2, 6.0, 62.0)];
    let second = [raw(0.0, 4.1, 60.0), raw(4.1, 6.0, 62.0)];
    assert_eq!(settle(&[note(0, 3, 60), note(3, 6, 62)], &lead, &second), vec![(0, 3, 60), (3, 6, 62)]);
    let looser = NearTieRule { min_lean: 0.15 };
    assert_eq!(cells(&settle_near_ties(&[note(0, 3, 60), note(3, 6, 62)], &lead, &second, CELL, 0.0, looser)), vec![(0, 4, 60), (4, 6, 62)]);
}

#[test]
fn the_edge_only_moves_toward_the_side_the_raw_time_leans() {
    // GAME leans late, but the second listen has the edge a cell early: no evidence for either move.
    let second = [raw(0.0, 1.9, 60.0), raw(1.9, 6.0, 62.0)];
    assert_eq!(settle(&[note(0, 3, 60), note(3, 6, 62)], &lead_leaning_late(), &second), vec![(0, 3, 60), (3, 6, 62)]);
}

#[test]
fn a_release_edge_moves_on_its_own() {
    // A note ending before a rest: its end rounded from 3.6 to 4, the second listen ends at 3.
    let lead = [raw(0.0, 3.6, 60.0), raw(6.0, 8.0, 62.0)];
    let second = [raw(0.0, 3.0, 60.0), raw(6.0, 8.0, 62.0)];
    assert_eq!(settle(&[note(0, 4, 60), note(6, 8, 62)], &lead, &second), vec![(0, 3, 60), (6, 8, 62)]);
}

#[test]
fn a_move_that_would_leave_a_note_without_a_cell_is_undone() {
    // A one-cell note whose start leans late onto its own end cell.
    let lead = [raw(0.4, 1.0, 60.0)];
    let second = [raw(1.0, 2.0, 60.0)];
    assert_eq!(settle(&[note(0, 1, 60)], &lead, &second), vec![(0, 1, 60)]);
}

#[test]
fn two_moves_onto_each_others_cells_are_both_dropped() {
    // A note ends on cell 3 leaning late, the next starts on cell 4 leaning early, and the second listen
    // has edges on both cells: moving both would overlap the notes.
    let lead = [raw(0.0, 3.4, 60.0), raw(3.6, 6.0, 64.0)];
    let second = [raw(0.0, 3.0, 60.0), raw(4.0, 6.0, 64.0)];
    let notes = [note(0, 3, 60), note(4, 6, 64)];
    assert_eq!(settle(&notes, &lead, &second), cells(&notes));
}

#[test]
fn without_a_second_listen_or_lead_notes_nothing_moves() {
    let notes = [note(0, 3, 60), note(3, 6, 62)];
    assert_eq!(settle(&notes, &lead_leaning_late(), &[]), cells(&notes));
    assert_eq!(settle(&notes, &[], &[raw(0.0, 4.0, 60.0)]), cells(&notes));
}

#[test]
fn the_line_stays_one_voice_on_the_grid_and_every_edge_moves_at_most_a_cell() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..500 {
        let n = 1 + (next() % 12) as i64;
        let (mut notes, mut lead, mut second) = (Vec::new(), Vec::new(), Vec::new());
        let mut t = (next() % 3) as i64;
        for _ in 0..n {
            let len = 1 + (next() % 4) as i64;
            notes.push(note(t, t + len, 60 + (next() % 5) as i32));
            t += len + (next() % 2) as i64;
        }
        for &(a, b, p) in &notes {
            let (a, b) = ((a / CELL).round(), (b / CELL).round());
            let j = |v: u64| (v % 1000) as f64 / 1000.0 - 0.5;
            lead.push(raw(a + j(next()), b + j(next()), p as f32));
            second.push(raw(a + 2.0 * j(next()), b + 2.0 * j(next()), p as f32));
        }
        let before = cells(&notes);
        let after = settle(&notes, &lead, &second);
        assert_eq!(after.len(), before.len(), "every note is kept");
        for (x, y) in before.iter().zip(&after) {
            assert!(y.1 > y.0, "a note keeps a cell: {after:?}");
            assert!((y.0 - x.0).abs() <= 1 && (y.1 - x.1).abs() <= 1 && y.2 == x.2, "{before:?} -> {after:?}");
        }
        for w in after.windows(2) {
            assert!(w[1].0 >= w[0].1, "one voice: {before:?} -> {after:?}");
        }
        for (&(a, b, _), &(c, d, _)) in settle_near_ties(&notes, &lead, &second, CELL, 0.0, NearTieRule::default()).iter().zip(&after) {
            assert!((a - c as f64 * CELL).abs() < 1e-9 && (b - d as f64 * CELL).abs() < 1e-9, "on the grid");
        }
    }
}
