//! `sona_core::release`: a note ends where the singer's energy falls away for good, and nowhere else.

use sona_core::release::{trim_releases, ReleaseRule};

const CELL: f64 = 0.1;
const HOP: f64 = 0.005;

/// An energy envelope (time, RMS) from one level per cell, starting at 0 s, a frame every 5 ms.
fn envelope(levels: &[f64]) -> Vec<(f64, f64)> {
    let frames = (levels.len() as f64 * CELL / HOP).round() as usize;
    (0..frames).map(|i| (i as f64 * HOP, levels[((i as f64 * HOP) / CELL + 1e-9).floor() as usize])).collect()
}

fn cells(notes: &[(f64, f64, i32)]) -> Vec<(i64, i64, i32)> {
    notes.iter().map(|&(a, b, p)| ((a / CELL).round() as i64, (b / CELL).round() as i64, p)).collect()
}

fn note(start: i64, end: i64, pitch: i32) -> (f64, f64, i32) {
    (start as f64 * CELL, end as f64 * CELL, pitch)
}

fn trim(notes: &[(f64, f64, i32)], levels: &[f64]) -> Vec<(i64, i64, i32)> {
    cells(&trim_releases(notes, &envelope(levels), CELL, ReleaseRule::default()))
}

#[test]
fn a_note_held_through_a_breath_ends_where_the_breath_starts() {
    // Four sung cells, a two-cell breath GAME held the note through, then the next note.
    let levels = [1.0, 0.9, 1.0, 0.8, 0.1, 0.15, 1.0, 1.0];
    let got = trim(&[note(0, 6, 60), note(6, 8, 62)], &levels);
    assert_eq!(got, vec![(0, 4, 60), (6, 8, 62)]);
}

#[test]
fn the_next_moras_consonant_in_the_last_cell_does_not_hide_the_release() {
    let levels = [1.0, 1.0, 1.0, 0.1, 0.6, 1.0];
    assert_eq!(trim(&[note(0, 5, 60)], &levels), vec![(0, 3, 60)]);
}

#[test]
fn a_loud_cell_after_the_dip_means_the_note_goes_on() {
    // The singer dips (a consonant inside the note) and comes back: not a release.
    let levels = [1.0, 1.0, 0.1, 0.1, 0.9, 1.0];
    assert_eq!(trim(&[note(0, 5, 60)], &levels), vec![(0, 5, 60)]);
}

#[test]
fn quiet_is_measured_against_the_notes_loudest_cell() {
    // An accented start, a soft body, then a breath: the breath is quiet next to the accent.
    let levels = [1.0, 0.4, 0.4, 0.4, 0.2, 0.2];
    assert_eq!(trim(&[note(0, 6, 60)], &levels), vec![(0, 4, 60)]);
}

#[test]
fn a_one_cell_dip_at_the_end_is_left_alone() {
    let levels = [1.0, 1.0, 1.0, 0.05, 1.0];
    assert_eq!(trim(&[note(0, 4, 60)], &levels), vec![(0, 4, 60)]);
    let one_cell = ReleaseRule { min_cells: 1, ..ReleaseRule::default() };
    assert_eq!(cells(&trim_releases(&[note(0, 4, 60)], &envelope(&levels), CELL, one_cell)), vec![(0, 3, 60)]);
}

#[test]
fn a_fade_that_never_drops_below_the_quiet_share_is_left_alone() {
    let levels = [1.0, 0.8, 0.6, 0.45, 0.35];
    assert_eq!(trim(&[note(0, 5, 60)], &levels), vec![(0, 5, 60)]);
}

#[test]
fn short_notes_silence_and_an_empty_envelope_change_nothing() {
    let notes = [note(0, 1, 60), note(1, 4, 62)];
    assert_eq!(trim(&notes, &[1.0, 0.0, 0.0, 0.0]), cells(&notes));
    assert_eq!(cells(&trim_releases(&notes, &[], CELL, ReleaseRule::default())), cells(&notes));
}

#[test]
fn notes_only_get_shorter_stay_on_the_grid_and_keep_a_cell() {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..300 {
        let levels: Vec<f64> = (0..40).map(|_| (next() % 100) as f64 / 100.0).collect();
        let mut notes = Vec::new();
        let mut c = 0i64;
        while c < 38 {
            let len = 1 + (next() % 8) as i64;
            let end = (c + len).min(40);
            notes.push(note(c, end, 50 + (next() % 20) as i32));
            c = end + (next() % 3) as i64;
        }
        let rule = ReleaseRule { quiet: (next() % 100) as f64 / 100.0, tail: 1.01, min_cells: 1 + (next() % 3) as usize };
        let out = trim_releases(&notes, &envelope(&levels), CELL, rule);
        assert_eq!(out.len(), notes.len());
        for (a, b) in notes.iter().zip(&out) {
            assert_eq!((a.0, a.2), (b.0, b.2), "onset and pitch never move");
            let cut = ((a.1 - b.1) / CELL).round();
            assert!((a.1 - b.1 - cut * CELL).abs() < 1e-9, "the end stays on the grid: {a:?} → {b:?}");
            assert!(cut == 0.0 || cut >= rule.min_cells as f64, "a cut drops at least min_cells: {a:?} → {b:?}");
            assert!(b.1 - b.0 >= CELL - 1e-9, "at least one cell is left: {b:?}");
        }
    }
}
