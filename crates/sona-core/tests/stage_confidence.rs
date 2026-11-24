//! The learned cell stage's note confidence (ADR-0033): it should point a user at the notes that are
//! expensive to fix in OpenUtau. A note hiding several moras is long, and a note the model was torn about
//! is where pieces get cut, so a long or unsure note must read as less confident than a short, sure one.
//! The model's weights aren't needed: the confidence reads only its logits and the notes.

use sona_core::cellstage::note_confidence;

const SPACING: f64 = 0.125;
const ANCHOR: f64 = 0.03;

/// Logits for one cell whose top class has probability `p` and the other five share the rest.
fn cell(p: f64) -> [f32; 6] {
    let top = (5.0 * p / (1.0 - p)).ln() as f32;
    [top, 0.0, 0.0, 0.0, 0.0, 0.0]
}

fn logits(cells: &[[f32; 6]]) -> Vec<f32> {
    cells.iter().flatten().copied().collect()
}

/// A note over cells `c0..c1` (cell 0 starts at `k0`).
fn note(k0: i64, c0: i64, c1: i64) -> (f64, f64, i32) {
    (ANCHOR + (k0 + c0) as f64 * SPACING, ANCHOR + (k0 + c1) as f64 * SPACING, 64)
}

#[test]
fn a_longer_note_is_less_confident_than_a_shorter_one_the_model_is_as_sure_of() {
    let k0 = -2;
    let sure = logits(&[cell(0.97); 63]);
    let lengths = [1, 2, 4, 8, 16, 32];
    let notes: Vec<_> = lengths.iter().scan(0, |at, &len| {
        let n = note(k0, *at, *at + len);
        *at += len;
        Some(n)
    }).collect();
    let conf = note_confidence(&sure, k0, SPACING, ANCHOR, &notes);
    for (w, l) in conf.windows(2).zip(lengths.windows(2)) {
        assert!(w[1] < w[0], "a {}-cell note scored {} but a {}-cell one {}", l[1], w[1], l[0], w[0]);
    }
}

#[test]
fn a_note_the_model_was_unsure_of_is_less_confident_than_one_it_was_sure_of() {
    let k0 = 0;
    let mut cells = vec![cell(0.99); 4];
    cells.extend([cell(0.45); 4]);
    let conf = note_confidence(&logits(&cells), k0, SPACING, ANCHOR, &[note(k0, 0, 4), note(k0, 4, 8)]);
    assert!(conf[1] < conf[0], "unsure cells scored {} against sure cells {}", conf[1], conf[0]);
}

#[test]
fn a_short_sure_note_reads_as_confident_and_a_long_unsure_one_falls_under_the_editors_default_flag() {
    // The editor outlines notes below 0.5 by default (web/index.html, Flag threshold).
    let k0 = 0;
    let mut cells = vec![cell(0.999); 1];
    cells.extend([cell(0.5); 16]);
    let conf = note_confidence(&logits(&cells), k0, SPACING, ANCHOR, &[note(k0, 0, 1), note(k0, 1, 17)]);
    assert!(conf[0] > 0.8, "a one-cell note every cell is sure of scored {}", conf[0]);
    assert!(conf[1] < 0.5, "a four-beat note at even odds scored {}", conf[1]);
}

#[test]
fn a_notes_confidence_reads_only_its_own_cells() {
    let k0 = 3;
    let mut cells = vec![cell(0.9); 12];
    let notes = [note(k0, 2, 5), note(k0, 5, 9)];
    let before = note_confidence(&logits(&cells), k0, SPACING, ANCHOR, &notes);
    for c in (0..2).chain(9..12) {
        cells[c] = cell(0.2);
    }
    let after = note_confidence(&logits(&cells), k0, SPACING, ANCHOR, &notes);
    assert_eq!(before, after, "cells outside every note moved the notes' confidence");
    cells[3] = cell(0.2);
    let moved = note_confidence(&logits(&cells), k0, SPACING, ANCHOR, &notes);
    assert!(moved[0] < before[0] && moved[1] == before[1], "an unsure cell inside the first note: {before:?} -> {moved:?}");
}

#[test]
fn every_note_gets_one_confidence_in_range_whatever_the_logits() {
    let mut x: u64 = 0x13c0_4f1d;
    let mut unit = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x >> 11) as f64 / (1u64 << 53) as f64
    };
    for case in 0..2000 {
        let k0 = (unit() * 20.0) as i64 - 10;
        let n = (unit() * 60.0) as usize;
        let scale = if unit() < 0.1 { 1e4 } else { 8.0 };
        let l: Vec<f32> = (0..n * 6).map(|_| ((unit() - 0.5) * scale) as f32).collect();
        let mut notes = Vec::new();
        let mut c = 0i64;
        while (c as usize) < n {
            let len = 1 + (unit() * 10.0) as i64;
            let end = (c + len).min(n as i64);
            if unit() < 0.7 {
                notes.push(note(k0, c, end));
            }
            c = end + i64::from(unit() < 0.3);
        }
        let conf = note_confidence(&l, k0, SPACING, ANCHOR, &notes);
        assert_eq!(conf.len(), notes.len(), "case {case}");
        assert!(conf.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)), "case {case}: {conf:?}");
    }
}

/// What the app shows for the stage's notes, behind `--features game` (CI's `game::` filter).
#[cfg(feature = "game")]
mod game {
    use super::*;
    use sona_core::game::{stage_annotation_notes, stage_segments, StageNotes};
    use sona_core::{ConfidenceScorer, EnergyFrame};

    #[test]
    fn the_app_gets_the_stages_confidence_and_everything_else_as_before() {
        let notes = vec![note(0, 0, 2), (ANCHOR + 2.0 * SPACING, ANCHOR + 3.0 * SPACING, 200), note(0, 3, 7), note(0, 9, 10)];
        let stage = StageNotes { notes: notes.clone(), confidence: vec![0.9, 0.1, 0.35, 0.7] };
        let envelope: Vec<EnergyFrame> = (0..200).map(|i| EnergyFrame { time_s: i as f64 * 0.01, rms: if i % 37 < 20 { 0.4 } else { 0.02 } }).collect();
        let scorer = ConfidenceScorer::default();
        let shown = stage_annotation_notes(&stage, &scorer, &envelope);
        let before = scorer.score_all(&stage_segments(&notes), &envelope);
        assert_eq!(shown.len(), 3, "the note with no MIDI pitch is dropped, the rest kept");
        for ((s, b), want) in shown.iter().zip(&before).zip([0.9, 0.35, 0.7]) {
            assert_eq!((s.onset_s, s.offset_s, s.midi_pitch, &s.sub_scores), (b.onset_s, b.offset_s, b.midi_pitch, &b.sub_scores));
            assert_eq!(s.confidence, want, "{s:?} carries another note's confidence");
        }
    }
}
