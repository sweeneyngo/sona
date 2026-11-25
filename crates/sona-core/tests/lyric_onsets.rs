//! A line's fit reshaped by its moras' onsets (lyrics-first 17's S1, ported in utau-edit-cost 21): a note
//! singing several moras is cut where the later ones begin, a note's start moves to its mora's onset when
//! that is a cell away, and the aligners' onsets agree on one cell per mora, or on none when they disagree.

use sona_core::lyric_fit::{consensus_onsets, fit_lines, fit_lines_with_onsets, lattice_prior, CellNote, LineSpan, Mora, OnsetConsensus, OnsetRule, TAIL_CELLS};

const C: Mora = Mora::Consonant;

fn note(start: i64, end: i64, pitch: i32) -> CellNote {
    CellNote { start, end, pitch }
}

fn line(first: usize, start: f64, end: f64) -> LineSpan {
    LineSpan { first, start, end }
}

fn s1(notes: &[CellNote], moras: &[Mora], lines: &[LineSpan], onsets: &[Option<i64>]) -> Vec<(CellNote, Option<usize>)> {
    fit_lines_with_onsets(notes, moras, lines, onsets, OnsetRule::S1)
}

#[test]
fn a_note_singing_two_moras_is_cut_where_the_second_begins() {
    let one = [note(0, 8, 60)];
    let fitted = s1(&one, &[C, C], &[line(0, 0.0, 8.0)], &[Some(0), Some(3)]);
    assert_eq!(fitted, vec![(note(0, 3, 60), Some(0)), (note(3, 8, 60), Some(1))]);
}

#[test]
fn without_a_usable_onset_the_extra_mora_takes_the_tail() {
    let one = [note(0, 8, 60)];
    let tail = vec![(note(0, 8 - TAIL_CELLS, 60), Some(0)), (note(8 - TAIL_CELLS, 8, 60), Some(1))];
    for second in [None, Some(0), Some(8), Some(-2), Some(11)] {
        assert_eq!(s1(&one, &[C, C], &[line(0, 0.0, 8.0)], &[Some(0), second]), tail, "second onset {second:?}");
    }
    let three = s1(&one, &[C, C, C], &[line(0, 0.0, 8.0)], &[Some(0), Some(7), Some(2)]);
    assert_eq!(three.iter().map(|(n, _)| (n.start, n.end)).collect::<Vec<_>>(), vec![(0, 4), (4, 6), (6, 8)], "onsets that do not increase");
}

#[test]
fn a_start_a_cell_from_its_onset_moves_there_and_a_touching_note_follows() {
    let notes = [note(0, 4, 60), note(4, 8, 62)];
    let lines = [line(0, 0.0, 8.0)];
    let later = s1(&notes, &[C, C], &lines, &[Some(0), Some(5)]);
    assert_eq!(later, vec![(note(0, 5, 60), Some(0)), (note(5, 8, 62), Some(1))]);
    let earlier = s1(&notes, &[C, C], &lines, &[Some(0), Some(3)]);
    assert_eq!(earlier, vec![(note(0, 3, 60), Some(0)), (note(3, 8, 62), Some(1))]);
}

#[test]
fn without_legato_a_later_start_leaves_a_gap() {
    let notes = [note(0, 4, 60), note(4, 8, 62)];
    let staccato = OnsetRule { legato: false, ..OnsetRule::S1 };
    let fitted = fit_lines_with_onsets(&notes, &[C, C], &[line(0, 0.0, 8.0)], &[Some(0), Some(5)], staccato);
    assert_eq!(fitted, vec![(note(0, 4, 60), Some(0)), (note(5, 8, 62), Some(1))]);
}

#[test]
fn a_start_moves_only_within_one_cell_and_never_empties_a_note() {
    let notes = [note(0, 4, 60), note(4, 8, 62)];
    let lines = [line(0, 0.0, 8.0)];
    let unmoved = vec![(notes[0], Some(0)), (notes[1], Some(1))];
    for second in [None, Some(4), Some(2), Some(6)] {
        assert_eq!(s1(&notes, &[C, C], &lines, &[Some(0), second]), unmoved, "second onset {second:?}");
    }
    let short = [note(0, 1, 60), note(1, 2, 62), note(2, 6, 64)];
    let fitted = s1(&short, &[C, C, C], &[line(0, 0.0, 6.0)], &[Some(0), Some(2), Some(1)]);
    assert_eq!(fitted, vec![(short[0], Some(0)), (short[1], Some(1)), (short[2], Some(2))], "the second would be empty, the third would empty the second");
    let first_back = s1(&short, &[C, C, C], &[line(0, 0.0, 6.0)], &[Some(0), Some(0), None]);
    assert_eq!(first_back[1].0, short[1], "a start moved onto the previous note's start would empty it");
}

#[test]
fn a_start_moved_into_another_line_cuts_that_lines_note() {
    let notes = [note(0, 4, 60), note(4, 8, 62)];
    let lines = [line(0, 0.0, 4.0), line(1, 4.0, 8.0)];
    let fitted = s1(&notes, &[C, C], &lines, &[Some(0), Some(3)]);
    assert_eq!(fitted, vec![(note(0, 3, 60), Some(0)), (note(3, 8, 62), Some(1))]);
    let instrumental = [note(0, 1, 50), note(1, 4, 62)];
    let fitted = s1(&instrumental, &[C], &[line(0, 1.0, 4.0)], &[Some(0)]);
    assert_eq!(fitted, vec![(note(0, 4, 62), Some(0))], "a note emptied by the move is dropped");
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

#[test]
fn any_fit_with_onsets_stays_one_voice_and_names_each_mora_once() {
    let mut rng = Rng(0x0021_5e7a);
    for case in 0..3000 {
        let mut notes = Vec::new();
        let mut t = rng.below(4) as i64;
        for _ in 0..1 + rng.below(12) {
            let len = 1 + rng.below(9) as i64;
            notes.push(note(t, t + len, 60 + rng.below(3) as i32));
            t += len + if rng.below(4) == 0 { 1 + rng.below(4) as i64 } else { 0 };
        }
        let moras: Vec<Mora> = (0..1 + rng.below(16)).map(|_| if rng.below(2) == 0 { Mora::Open } else { C }).collect();
        let mut firsts: Vec<usize> = (0..rng.below(5)).map(|_| rng.below(moras.len() as u64) as usize).collect();
        firsts.push(0);
        firsts.sort_unstable();
        firsts.dedup();
        let lines: Vec<LineSpan> = firsts.iter().map(|&f| {
            let a = rng.below(t as u64 + 4) as f64 + 0.5 * rng.below(2) as f64 - 2.0;
            line(f, a, a + rng.below(16) as f64)
        }).collect();
        let onsets: Vec<Option<i64>> = (0..moras.len()).map(|_| (rng.below(4) != 0).then(|| rng.below(t as u64 + 2) as i64 - 1)).collect();
        let fitted = s1(&notes, &moras, &lines, &onsets);
        let cells: Vec<CellNote> = fitted.iter().map(|(n, _)| *n).collect();
        assert!(cells.iter().all(|n| n.end > n.start), "case {case}: empty note in {cells:?}");
        assert!(cells.windows(2).all(|w| w[0].end <= w[1].start), "case {case}: overlap in {cells:?}");
        let mut named: Vec<usize> = fitted.iter().filter_map(|(_, m)| *m).collect();
        named.sort_unstable();
        assert!(named.windows(2).all(|w| w[0] < w[1]), "case {case}: a mora named twice: {fitted:?}");
        let plain = fit_lines(&notes, &moras, &lines);
        assert_eq!(fit_lines_with_onsets(&notes, &moras, &lines, &vec![None; moras.len()], OnsetRule::S1), plain, "case {case}: no onset known");
        let off = OnsetRule { split: false, start_tolerance: 0, ..OnsetRule::S1 };
        assert_eq!(fit_lines_with_onsets(&notes, &moras, &lines, &onsets, off), plain, "case {case}: rule off");
    }
}

#[test]
fn the_lattice_prior_is_the_smoothed_share_of_starts_at_each_beat_position() {
    let prior = lattice_prior(&[0, 4, 8, 1, -1], 4);
    assert_eq!(prior, vec![4.0 / 9.0, 2.0 / 9.0, 1.0 / 9.0, 2.0 / 9.0]);
    assert_eq!(lattice_prior(&[], 4), vec![0.25; 4]);
}

const ANCHOR: f64 = 0.37;
const SPACING: f64 = 0.125;

fn at(cell: f64) -> Option<f64> {
    Some(ANCHOR + cell * SPACING)
}

fn agree(members: &[Vec<Option<f64>>], prior: &[f64]) -> Vec<Option<i64>> {
    consensus_onsets(members, ANCHOR, SPACING, prior, OnsetConsensus::S1)
}

#[test]
fn aligners_that_agree_give_the_nearest_cell() {
    let flat = [0.25; 4];
    let members = vec![vec![at(6.1), at(2.8)], vec![at(5.9), at(3.3)], vec![at(6.2), at(3.0)]];
    assert_eq!(agree(&members, &flat), vec![Some(6), Some(3)]);
}

#[test]
fn the_median_ignores_one_aligner_far_off_and_missing_ones() {
    let flat = [0.25; 4];
    let members = vec![vec![at(6.1), None, None], vec![at(8.6), at(2.2), None], vec![at(5.8), at(1.6), None]];
    assert_eq!(agree(&members, &flat), vec![Some(6), Some(2), None]);
}

#[test]
fn aligners_spread_over_more_than_three_cells_give_no_onset() {
    let flat = [0.25; 4];
    let members = vec![vec![at(2.0), at(2.0)], vec![at(4.0), at(4.0)], vec![at(4.9), at(5.2)]];
    assert_eq!(agree(&members, &flat), vec![Some(4), None]);
}

#[test]
fn the_less_the_aligners_agree_the_more_the_beat_decides() {
    // Cell 5 is on the beat (position 1 of 4 here); the members' median is 4.4.
    let prior = [0.1, 0.5, 0.2, 0.2];
    let tight = vec![vec![at(4.4)], vec![at(4.4)], vec![at(4.45)]];
    assert_eq!(agree(&tight, &prior), vec![Some(4)]);
    let loose = vec![vec![at(3.0)], vec![at(4.4)], vec![at(5.8)]];
    assert_eq!(agree(&loose, &prior), vec![Some(5)]);
}
