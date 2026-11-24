//! **Lyric placement** (lyrics-first 01): how many of the UST's moras a user must retype or shift after
//! a lyric lands on a transcription's notes.
//!
//! Both sides go on the UST's cells exactly as in [`super::edit_cost`] (same rounding, overlaps cut,
//! only output notes in the UST's span). A UST note `r` is **matched** to the output note `o` that
//! shares the most cells with it (ties: the earlier note) when `r` is also the UST note that shares the
//! most cells with `o`, so each note has at most one match. `r` is **placed** when it is matched and `o`
//! carries `r`'s mora, compared after taking a VCV lyric's last word and folding katakana to hiragana.
//! Every other UST note is a mora to retype or shift.
//!
//! A UST **phrase** is a run of touching UST notes. It **lands** when every note in it is placed and no
//! other output note whose midpoint lies in it carries a mora: the pasted lyric fell one mora per note.

use super::edit_cost::{bootstrap_pooled, to_cells, CellGrid, BOOTSTRAP_ITERS};
use super::EditNote;

/// One song's lyric placement, or a pool of songs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LyricPlacement {
    /// UST notes scored (the edit cost's notes).
    pub notes: usize,
    /// UST notes whose matched output note carries their mora.
    pub placed: usize,
    pub phrases: usize,
    pub landed: usize,
    /// Length of the UST's span, in seconds.
    pub span_s: f64,
}

impl LyricPlacement {
    /// UST notes not placed, per minute of the UST's span.
    pub fn wrong_per_minute(&self) -> f64 {
        if self.span_s > 0.0 {
            (self.notes - self.placed) as f64 / (self.span_s / 60.0)
        } else {
            f64::NAN
        }
    }

    pub fn placed_share(&self) -> f64 {
        self.placed as f64 / self.notes as f64
    }

    pub fn landed_share(&self) -> f64 {
        self.landed as f64 / self.phrases as f64
    }

    pub fn add(&mut self, o: &LyricPlacement) {
        self.notes += o.notes;
        self.placed += o.placed;
        self.phrases += o.phrases;
        self.landed += o.landed;
        self.span_s += o.span_s;
    }
}

/// A lyric as one mora to compare: a VCV lyric's last word (`a か` → `か`), katakana folded to hiragana.
pub fn mora_key(lyric: &str) -> String {
    let l = lyric.split_whitespace().last().unwrap_or("");
    l.chars().map(|c| if ('ァ'..='ヶ').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap_or(c) } else { c }).collect()
}

/// The lyric placement of `output` (notes with the mora each carries) against `reference` (the UST's
/// notes and lyrics) on the UST's grid `g`.
pub fn lyric_placement(reference: &[(EditNote, &str)], output: &[(EditNote, Option<&str>)], g: &CellGrid) -> LyricPlacement {
    let ref_notes: Vec<EditNote> = reference.iter().map(|(n, _)| *n).collect();
    let refs = to_cells(&ref_notes, g);
    let (Some(first), Some(last)) = (refs.first(), refs.last()) else { return LyricPlacement::default() };
    let (span_start, span_end) = (first.start, last.end);
    let out_notes: Vec<EditNote> = output.iter().map(|(n, _)| *n).collect();
    let outs: Vec<_> = to_cells(&out_notes, g).into_iter().filter(|o| o.end > span_start && o.start < span_end).collect();
    let ref_mora: Vec<String> = refs.iter().map(|r| mora_key(reference[r.index].1)).collect();
    let out_mora: Vec<Option<String>> = outs.iter().map(|o| output[o.index].1.map(mora_key)).collect();

    let mut best_out: Vec<Option<(i64, usize)>> = vec![None; refs.len()];
    let mut best_ref: Vec<Option<(i64, usize)>> = vec![None; outs.len()];
    for (ri, r) in refs.iter().enumerate() {
        let from = outs.partition_point(|o| o.end <= r.start);
        for (oi, o) in outs.iter().enumerate().skip(from).take_while(|(_, o)| o.start < r.end) {
            let shared = r.shared(o);
            if shared <= 0 {
                continue;
            }
            if best_out[ri].is_none_or(|(s, _)| shared > s) {
                best_out[ri] = Some((shared, oi));
            }
            if best_ref[oi].is_none_or(|(s, _)| shared > s) {
                best_ref[oi] = Some((shared, ri));
            }
        }
    }
    let mut placed = vec![false; refs.len()];
    let mut matched = vec![false; outs.len()];
    for (ri, best) in best_out.iter().enumerate() {
        if let Some((_, oi)) = *best {
            if best_ref[oi].is_some_and(|(_, back)| back == ri) {
                matched[oi] = true;
                placed[ri] = out_mora[oi].as_deref() == Some(ref_mora[ri].as_str());
            }
        }
    }

    let mut phrases: Vec<(usize, usize)> = Vec::new();
    for (ri, r) in refs.iter().enumerate() {
        match phrases.last_mut() {
            Some((_, end)) if refs[*end - 1].end == r.start => *end = ri + 1,
            _ => phrases.push((ri, ri + 1)),
        }
    }
    let landed = phrases
        .iter()
        .filter(|&&(a, b)| {
            let (from, to) = (2 * refs[a].start, 2 * refs[b - 1].end);
            let stray = outs.iter().zip(&out_mora).zip(&matched).any(|((o, m), &hit)| (from..to).contains(&(o.start + o.end)) && m.is_some() && !hit);
            placed[a..b].iter().all(|&p| p) && !stray
        })
        .count();
    LyricPlacement {
        notes: refs.len(),
        placed: placed.iter().filter(|&&p| p).count(),
        phrases: phrases.len(),
        landed,
        span_s: (span_end - span_start) as f64 * g.spacing_s,
    }
}

/// A per-song table plus the pool: wrong moras per minute, placed notes and landed phrases, each pooled
/// over songs with a song-bootstrap 95% interval.
pub fn report(title: &str, songs: &[(String, LyricPlacement)]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "\nlyric placement, {title}");
    let _ = writeln!(s, "  {:<28} {:>5} {:>6} {:>10} {:>14} {:>16}", "song", "min", "wrong", "wrong/min", "placed", "phrases landed");
    let row = |s: &mut String, name: &str, p: &LyricPlacement| {
        let _ = writeln!(
            s,
            "  {:<28} {:>5.2} {:>6} {:>10.1} {:>5.1}% {:>3}/{:<3} {:>5.1}% {:>3}/{:<3}",
            name,
            p.span_s / 60.0,
            p.notes - p.placed,
            p.wrong_per_minute(),
            100.0 * p.placed_share(),
            p.placed,
            p.notes,
            100.0 * p.landed_share(),
            p.landed,
            p.phrases
        );
    };
    let mut pool = LyricPlacement::default();
    for (name, p) in songs {
        row(&mut s, name, p);
        pool.add(p);
    }
    row(&mut s, &format!("POOLED ({} songs)", songs.len()), &pool);
    let all: Vec<LyricPlacement> = songs.iter().map(|(_, p)| *p).collect();
    let ci = |stat: fn(&LyricPlacement) -> f64| bootstrap_pooled(&all, BOOTSTRAP_ITERS, LyricPlacement::add, stat);
    let ((wlo, whi), (plo, phi), (llo, lhi)) = (ci(LyricPlacement::wrong_per_minute), ci(LyricPlacement::placed_share), ci(LyricPlacement::landed_share));
    let _ = writeln!(
        s,
        "  pooled: {:.1} wrong moras/min [95% {:.1}, {:.1}] · placed {:.1}% [95% {:.1}, {:.1}] · phrases landed {:.1}% [95% {:.1}, {:.1}] ({BOOTSTRAP_ITERS} song resamples)",
        pool.wrong_per_minute(),
        wlo,
        whi,
        100.0 * pool.placed_share(),
        100.0 * plo,
        100.0 * phi,
        100.0 * pool.landed_share(),
        100.0 * llo,
        100.0 * lhi
    );
    s
}
