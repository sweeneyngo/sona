//! Lyric placement (lyrics-first 01): the moras a user must retype or shift once a lyric lands on a
//! transcription's notes, matched to the UST's notes on its cells as the edit cost matches them.

use sona_eval::correction::edit_cost::CellGrid;
use sona_eval::correction::lyric_placement::{lyric_placement, mora_key, report, LyricPlacement};
use sona_eval::correction::EditNote;

const G: CellGrid = CellGrid { spacing_s: 0.125, anchor_s: 0.0, cells_per_measure: 16 };

fn n(start: i64, end: i64) -> EditNote {
    EditNote { onset_s: start as f64 * G.spacing_s, offset_s: end as f64 * G.spacing_s, midi_pitch: 60.0 }
}

fn ust(notes: &[(i64, i64, &'static str)]) -> Vec<(EditNote, &'static str)> {
    notes.iter().map(|&(a, b, m)| (n(a, b), m)).collect()
}

fn sung(notes: &[(i64, i64, Option<&'static str>)]) -> Vec<(EditNote, Option<&'static str>)> {
    notes.iter().map(|&(a, b, m)| (n(a, b), m)).collect()
}

fn counts(p: &LyricPlacement) -> (usize, usize, usize, usize) {
    (p.placed, p.notes, p.landed, p.phrases)
}

fn phrase() -> Vec<(EditNote, &'static str)> {
    ust(&[(0, 4, "あ"), (4, 8, "い"), (8, 12, "う"), (12, 16, "え")])
}

#[test]
fn the_ust_itself_places_every_mora_and_lands_every_phrase() {
    let reference = ust(&[(0, 4, "か"), (4, 8, "な"), (12, 16, "た"), (16, 20, "ん")]);
    let output: Vec<_> = reference.iter().map(|(e, m)| (*e, Some(*m))).collect();
    let p = lyric_placement(&reference, &output, &G);
    assert_eq!(counts(&p), (4, 4, 2, 2));
    assert_eq!(p.wrong_per_minute(), 0.0);
    assert!((p.span_s - 20.0 * 0.125).abs() < 1e-12);
}

#[test]
fn one_extra_note_shifts_the_rest_of_a_pasted_phrase() {
    // The first note chopped in two; pasting in order puts every later mora one note early.
    let output = sung(&[(0, 2, Some("あ")), (2, 4, Some("い")), (4, 8, Some("う")), (8, 12, Some("え")), (12, 16, None)]);
    let p = lyric_placement(&phrase(), &output, &G);
    assert_eq!(counts(&p), (1, 4, 0, 1));
    let minutes = 16.0 * 0.125 / 60.0;
    assert!((p.wrong_per_minute() - 3.0 / minutes).abs() < 1e-9);
}

#[test]
fn notes_one_cell_off_still_carry_their_moras() {
    let output = sung(&[(1, 5, Some("あ")), (5, 8, Some("い")), (8, 13, Some("う")), (13, 16, Some("え"))]);
    assert_eq!(counts(&lyric_placement(&phrase(), &output, &G)), (4, 4, 1, 1));
}

#[test]
fn a_joined_note_places_only_one_of_its_moras_even_when_both_are_the_same() {
    let reference = ust(&[(0, 4, "ら"), (4, 8, "ら")]);
    let p = lyric_placement(&reference, &sung(&[(0, 8, Some("ら"))]), &G);
    assert_eq!(counts(&p), (1, 2, 0, 1));
}

#[test]
fn a_note_without_a_mora_or_with_the_wrong_one_is_not_placed() {
    let output = sung(&[(0, 4, Some("あ")), (4, 8, None), (8, 12, Some("お")), (12, 16, Some("え"))]);
    assert_eq!(counts(&lyric_placement(&phrase(), &output, &G)), (2, 4, 0, 1));
}

#[test]
fn a_stray_mora_inside_a_phrase_keeps_it_from_landing() {
    let reference = ust(&[(0, 4, "あ"), (4, 8, "い")]);
    let output = sung(&[(0, 4, Some("あ")), (4, 7, Some("い")), (7, 8, Some("う"))]);
    assert_eq!(counts(&lyric_placement(&reference, &output, &G)), (2, 2, 0, 1));
    let silent = sung(&[(0, 4, Some("あ")), (4, 7, Some("い")), (7, 8, None)]);
    assert_eq!(counts(&lyric_placement(&reference, &silent, &G)), (2, 2, 1, 1));
}

#[test]
fn phrases_are_runs_of_touching_ust_notes() {
    let reference = ust(&[(0, 4, "あ"), (4, 8, "い"), (10, 14, "う"), (14, 18, "え"), (24, 28, "お")]);
    let output = sung(&[(0, 4, Some("あ")), (4, 8, Some("い")), (10, 14, Some("う")), (14, 18, Some("か")), (24, 28, Some("お"))]);
    assert_eq!(counts(&lyric_placement(&reference, &output, &G)), (4, 5, 2, 3));
}

#[test]
fn katakana_and_vcv_lyrics_name_the_same_mora() {
    for (a, b) in [("か", "カ"), ("か", "a か"), ("きゃ", "キャ"), ("ん", "- ン")] {
        assert_eq!(mora_key(a), mora_key(b), "{a} {b}");
    }
    assert_ne!(mora_key("か"), mora_key("が"));
    let reference = ust(&[(0, 4, "a か")]);
    assert_eq!(counts(&lyric_placement(&reference, &sung(&[(0, 4, Some("カ"))]), &G)), (1, 1, 1, 1));
}

#[test]
fn output_outside_the_usts_span_is_ignored() {
    let reference = ust(&[(8, 12, "あ")]);
    let output = sung(&[(0, 4, Some("あ")), (8, 12, Some("あ")), (20, 24, Some("あ"))]);
    assert_eq!(counts(&lyric_placement(&reference, &output, &G)), (1, 1, 1, 1));
    assert_eq!(lyric_placement(&[], &output, &G), LyricPlacement::default());
}

#[test]
fn songs_pool_and_report_with_an_interval() {
    let good = lyric_placement(&phrase(), &phrase().iter().map(|(e, m)| (*e, Some(*m))).collect::<Vec<_>>(), &G);
    let bad = lyric_placement(&phrase(), &sung(&[(0, 16, None)]), &G);
    let mut pool = good;
    pool.add(&bad);
    assert_eq!(counts(&pool), (4, 8, 1, 2));
    let text = report("test", &[("good".into(), good), ("bad".into(), bad)]);
    assert!(text.contains("POOLED (2 songs)") && text.contains("wrong moras/min [95%"), "{text}");
}
