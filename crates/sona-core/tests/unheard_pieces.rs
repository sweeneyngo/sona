//! Merging one-cell pieces a second listen doesn't hear (utau-edit-cost 11): a scoop or wobble chopped
//! off a held note joins the note, a short note the second listen also cuts stays, and the line keeps
//! singing the same cells in one voice.

use sona_core::lyric_fit::CellNote;
use sona_core::unheard_pieces::{edge_cells, merge_unheard_pieces, MAX_STEP};

fn note(start: i64, end: i64, pitch: i32) -> CellNote {
    CellNote { start, end, pitch }
}

#[test]
fn a_one_cell_scoop_the_vocals_run_does_not_cut_joins_the_held_note_at_its_pitch() {
    let notes = [note(0, 1, 59), note(1, 8, 60)];
    assert_eq!(merge_unheard_pieces(&notes, &[0, 8]), vec![note(0, 8, 60)]);
}

#[test]
fn a_one_cell_wobble_at_the_end_of_a_held_note_joins_it() {
    let notes = [note(0, 6, 64), note(6, 7, 65), note(9, 12, 60)];
    assert_eq!(merge_unheard_pieces(&notes, &[0, 7, 9, 12]), vec![note(0, 7, 64), note(9, 12, 60)]);
}

#[test]
fn a_short_note_the_vocals_run_also_cuts_stays() {
    let notes = [note(0, 1, 59), note(1, 8, 60)];
    assert_eq!(merge_unheard_pieces(&notes, &[0, 1, 8]), notes.to_vec());
}

#[test]
fn longer_pieces_octaves_same_pitch_and_rests_are_left_alone() {
    let two_cells = [note(0, 2, 59), note(2, 8, 60)];
    let octave = [note(0, 1, 48), note(1, 8, 60)];
    let same_pitch = [note(0, 1, 60), note(1, 8, 60)];
    let rest_between = [note(0, 1, 59), note(2, 8, 60)];
    for notes in [&two_cells[..], &octave, &same_pitch, &rest_between] {
        assert_eq!(merge_unheard_pieces(notes, &[]), notes.to_vec());
    }
    let leap = [note(0, 1, 60 - MAX_STEP), note(1, 8, 60)];
    assert_eq!(merge_unheard_pieces(&leap, &[]), vec![note(0, 8, 60)]);
}

#[test]
fn a_piece_between_two_notes_joins_the_nearer_pitch() {
    let notes = [note(0, 4, 57), note(4, 5, 59), note(5, 9, 60)];
    assert_eq!(merge_unheard_pieces(&notes, &[]), vec![note(0, 4, 57), note(4, 9, 60)]);
    let notes = [note(0, 4, 58), note(4, 5, 59), note(5, 9, 61)];
    assert_eq!(merge_unheard_pieces(&notes, &[]), vec![note(0, 5, 58), note(5, 9, 61)]);
}

#[test]
fn of_two_one_cell_pieces_the_first_joins_the_second() {
    let notes = [note(0, 1, 62), note(1, 2, 64)];
    assert_eq!(merge_unheard_pieces(&notes, &[]), vec![note(0, 2, 64)]);
}

#[test]
fn a_run_of_pieces_takes_the_pitch_of_its_longest() {
    let notes = [note(0, 1, 58), note(1, 2, 59), note(2, 7, 60)];
    assert_eq!(merge_unheard_pieces(&notes, &[]), vec![note(0, 7, 60)]);
}

#[test]
fn edges_are_rounded_to_the_grid_and_an_empty_span_ends_a_cell_later() {
    assert_eq!(edge_cells([(1.02, 2.49), (3.0, 3.01)], 0.5, 0.0), vec![2, 5, 6, 7]);
    assert_eq!(edge_cells([(0.75, 1.25)], 0.5, 0.0), vec![2, 3]);
}

struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

#[test]
fn random_lines_keep_their_cells_and_one_voice_and_only_join_touching_notes() {
    let mut rng = Lcg(11);
    for _ in 0..2000 {
        let mut notes = Vec::new();
        let mut t = 0;
        for _ in 0..rng.below(12) {
            t += rng.below(3) as i64 / 2;
            let len = 1 + rng.below(4) as i64;
            notes.push(note(t, t + len, 55 + rng.below(14) as i32));
            t += len;
        }
        let edges: Vec<i64> = (0..t + 1).filter(|_| rng.below(3) == 0).collect();
        let out = merge_unheard_pieces(&notes, &edges);
        let cells = |ns: &[CellNote]| ns.iter().flat_map(|n| n.start..n.end).collect::<Vec<_>>();
        assert_eq!(cells(&out), cells(&notes), "{notes:?} -> {out:?}");
        assert!(out.windows(2).all(|w| w[0].end <= w[1].start), "{out:?}");
        for o in &out {
            let inside: Vec<&CellNote> = notes.iter().filter(|n| o.start <= n.start && n.end <= o.end).collect();
            assert!(inside.windows(2).all(|w| w[0].end == w[1].start), "{notes:?} -> {out:?}");
            assert!(inside.iter().any(|n| n.pitch == o.pitch), "{notes:?} -> {out:?}");
            if inside.len() > 1 {
                assert!(inside.windows(2).all(|w| !edges.contains(&w[1].start)), "joined across an edge: {notes:?} -> {out:?}");
                assert!(inside.iter().filter(|n| n.end - n.start > 1).count() <= 1, "two long notes joined: {notes:?} -> {out:?}");
            }
        }
    }
}
