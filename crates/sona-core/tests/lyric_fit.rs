//! Fitting a pasted phrase's moras to its notes (utau-edit-cost 07): a note the singer ran two
//! same-pitch moras through is split the way a UST writes it, two touching same-pitch notes sung as one
//! mora are joined, and nothing else about the line changes: the same cells stay sung at the same
//! pitches, in one voice.

use sona_core::lyric_fit::{fit_phrase, fit_phrase_moras, fit_span_moras, mora_of, CellNote, Mora, TAIL_CELLS};

fn note(start: i64, end: i64, pitch: i32) -> CellNote {
    CellNote { start, end, pitch }
}

#[test]
fn a_held_note_sung_as_two_moras_splits_with_the_second_at_its_end() {
    let fitted = fit_phrase(&[note(0, 6, 60)], &[mora_of("さ"), mora_of("い")]);
    assert_eq!(fitted, vec![note(0, 6 - TAIL_CELLS, 60), note(6 - TAIL_CELLS, 6, 60)]);
}

#[test]
fn notes_that_already_carry_one_mora_each_are_left_alone() {
    let notes = [note(0, 4, 60), note(4, 8, 62), note(8, 12, 62)];
    assert_eq!(fit_phrase(&notes, &[mora_of("か"), mora_of("な"), mora_of("た")]), notes.to_vec());
}

#[test]
fn a_same_pitch_restart_sung_as_one_mora_is_joined() {
    assert_eq!(fit_phrase(&[note(0, 3, 60), note(3, 6, 60)], &[mora_of("か")]), vec![note(0, 6, 60)]);
}

#[test]
fn a_note_without_a_mora_is_not_joined_across_a_pitch_change() {
    let notes = [note(0, 3, 60), note(3, 6, 62)];
    let fitted = fit_phrase(&notes, &[mora_of("か")]);
    assert_eq!(fitted, notes.to_vec());
}

#[test]
fn without_moras_the_phrase_is_unchanged() {
    let notes = [note(0, 3, 60), note(3, 6, 60), note(6, 7, 64)];
    assert_eq!(fit_phrase(&notes, &[]), notes.to_vec());
}

#[test]
fn an_open_mora_hides_in_a_held_note_before_a_consonant_does() {
    // Three moras on two equal notes: the extra mora goes where a singer would run it in.
    let notes = [note(0, 6, 60), note(6, 12, 60)];
    let open_second = fit_phrase(&notes, &[mora_of("か"), mora_of("い"), mora_of("た")]);
    assert_eq!(open_second, vec![note(0, 4, 60), note(4, 6, 60), note(6, 12, 60)]);
    let open_third = fit_phrase(&notes, &[mora_of("か"), mora_of("た"), mora_of("ん")]);
    assert_eq!(open_third, vec![note(0, 6, 60), note(6, 10, 60), note(10, 12, 60)]);
}

#[test]
fn moras_are_read_from_ust_lyrics() {
    for open in ["あ", "ア", "ぁ", "ん", "ン", "ー", "-", "a", "O", "N", "a あ", " い "] {
        assert_eq!(mora_of(open), Mora::Open, "{open}");
    }
    for consonant in ["か", "カ", "きゃ", "っ", "ka", "a か", "", "息", "ゔ"] {
        assert_eq!(mora_of(consonant), Mora::Consonant, "{consonant}");
    }
}

/// Deterministic xorshift, so a failure reproduces from its case number.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn sung_cells(notes: &[CellNote]) -> Vec<(i64, i32)> {
    notes.iter().flat_map(|n| (n.start..n.end).map(move |c| (c, n.pitch))).collect()
}

#[test]
fn any_fit_keeps_the_same_cells_at_the_same_pitches_in_one_voice() {
    let mut rng = Rng(0x07f1_7a5e);
    for case in 0..3000 {
        let mut notes = Vec::new();
        let mut t = rng.below(4) as i64;
        for _ in 0..1 + rng.below(8) {
            let len = 1 + rng.below(9) as i64;
            let pitch = 60 + rng.below(3) as i32;
            notes.push(note(t, t + len, pitch));
            t += len + if rng.below(5) == 0 { 1 + rng.below(3) as i64 } else { 0 };
        }
        let moras: Vec<Mora> = (0..rng.below(14)).map(|_| if rng.below(2) == 0 { Mora::Open } else { Mora::Consonant }).collect();
        let fitted = fit_phrase(&notes, &moras);
        assert_eq!(sung_cells(&fitted), sung_cells(&notes), "case {case}: {notes:?} {moras:?} -> {fitted:?}");
        assert!(fitted.iter().all(|n| n.end > n.start), "case {case}: empty note in {fitted:?}");
        assert!(fitted.windows(2).all(|w| w[0].end <= w[1].start), "case {case}: overlap in {fitted:?}");
        assert!(fitted.len() <= notes.len().max(moras.len()) + notes.len(), "case {case}");
    }
}

#[test]
fn each_fitted_note_names_the_mora_it_sings() {
    let moras = [mora_of("さ"), mora_of("い"), mora_of("か")];
    let fitted = fit_phrase_moras(&[note(0, 6, 60), note(6, 10, 62)], &moras);
    assert_eq!(fitted, vec![(note(0, 4, 60), Some(0)), (note(4, 6, 60), Some(1)), (note(6, 10, 62), Some(2))]);
    let joined = fit_phrase_moras(&[note(0, 3, 60), note(3, 6, 60)], &[mora_of("か")]);
    assert_eq!(joined, vec![(note(0, 6, 60), Some(0))]);
    let unsung = fit_phrase_moras(&[note(0, 3, 60), note(3, 6, 62)], &[mora_of("か")]);
    assert_eq!(unsung.iter().filter(|(_, m)| m.is_some()).count(), 1);
    assert_eq!(fit_phrase_moras(&[note(0, 3, 60)], &[]), vec![(note(0, 3, 60), None)]);
}

#[test]
fn a_mora_sung_in_a_gap_stays_in_the_gap_when_the_line_spans_it() {
    let notes = [note(0, 4, 60), note(8, 12, 62)];
    let moras = [mora_of("か"), mora_of("た"), mora_of("な")];
    let forced = fit_phrase_moras(&notes, &moras);
    assert_eq!(forced.len(), 3, "without the span a neighbour splits: {forced:?}");
    let spanned = fit_span_moras(&notes, &moras, (0, 12));
    assert_eq!(spanned, vec![(note(0, 4, 60), Some(0)), (note(8, 12, 62), Some(2))]);
}

#[test]
fn any_fit_names_each_mora_at_most_once_in_order() {
    let mut rng = Rng(0x01a7_11e5);
    for case in 0..3000 {
        let mut notes = Vec::new();
        let mut t = rng.below(4) as i64;
        for _ in 0..1 + rng.below(8) {
            let len = 1 + rng.below(9) as i64;
            notes.push(note(t, t + len, 60 + rng.below(3) as i32));
            t += len + if rng.below(4) == 0 { 1 + rng.below(4) as i64 } else { 0 };
        }
        let moras: Vec<Mora> = (0..rng.below(14)).map(|_| if rng.below(2) == 0 { Mora::Open } else { Mora::Consonant }).collect();
        let span = (notes[0].start - rng.below(3) as i64, t + rng.below(3) as i64);
        for (fit, name) in [(fit_phrase_moras(&notes, &moras), "phrase"), (fit_span_moras(&notes, &moras, span), "span")] {
            let named: Vec<usize> = fit.iter().filter_map(|(_, m)| *m).collect();
            assert!(named.windows(2).all(|w| w[0] < w[1]) && named.iter().all(|&m| m < moras.len()), "case {case} {name}: {fit:?}");
            let cells: Vec<CellNote> = fit.iter().map(|(n, _)| *n).collect();
            assert_eq!(sung_cells(&cells), sung_cells(&notes), "case {case} {name}");
            assert!(cells.windows(2).all(|w| w[0].end <= w[1].start), "case {case} {name}: overlap in {cells:?}");
        }
        assert_eq!(fit_phrase(&notes, &moras), fit_phrase_moras(&notes, &moras).into_iter().map(|(n, _)| n).collect::<Vec<_>>(), "case {case}");
    }
}
