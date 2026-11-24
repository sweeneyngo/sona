//! Fitting a pasted lyric phrase to its notes (utau-edit-cost 07). UTAU writes one note per mora, but a
//! singer runs same-pitch moras together (さ>い, げ>ん), so a transcriber hears one note where the UST has
//! two. Given the moras a user pastes onto a run of touching notes, a DP gives every note a block of
//! moras; a note holding several is split, each extra mora taking the note's last [`TAIL_CELLS`] cells
//! (how the UST writes it), and a note holding none is merged into a touching same-pitch note before
//! it. The prototype and its dev numbers: `crates/sona-eval/scripts/same-pitch-boundaries/`.

/// A note on a grid of cells, `start..end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellNote {
    pub start: i64,
    pub end: i64,
    pub pitch: i32,
}

/// How a mora begins. A singer runs an open mora (a bare vowel, ん, or a held ー) into the note before
/// it about half the time; a consonant rarely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mora {
    Consonant,
    Open,
}

/// Cells each extra mora takes at the end of a split note.
pub const TAIL_CELLS: i64 = 2;
const MAX_MORAS_PER_NOTE: usize = 8;
const DURATION_WEIGHT: f64 = 0.5;
const NOTE_WITHOUT_MORA: f64 = 2.0;
const MORA_WITHOUT_NOTE: f64 = 3.0;
const HIDDEN_CONSONANT: f64 = 0.15;
const HIDDEN_OPEN: f64 = 0.55;
/// The cost of a mora sung where the notes have a rest ([`fit_span_moras`]).
pub const MORA_IN_REST: f64 = 0.0;

/// The kind of one UST lyric (kana, katakana, romaji, or VCV `a か`).
pub fn mora_of(lyric: &str) -> Mora {
    let mut l = lyric.trim();
    if l.contains(' ') {
        l = l.split_whitespace().last().unwrap_or("");
    }
    let l: String = l.chars().map(|c| if ('ァ'..='ヶ').contains(&c) { char::from_u32(c as u32 - 0x60).unwrap_or(c) } else { c }).collect();
    let one = |set: &str| l.chars().count() == 1 && l.chars().all(|c| set.contains(c));
    let open = matches!(l.as_str(), "ー" | "-" | "～" | "~" | "ん" | "n" | "N") || one("あいうえおぁぃぅぇぉ") || one("aiueoAIUEO");
    if open {
        Mora::Open
    } else {
        Mora::Consonant
    }
}

/// How many moras each note sings, by a DP over the phrase: each note takes a block of moras (none
/// costs [`NOTE_WITHOUT_MORA`]), a mora may go unsung ([`MORA_WITHOUT_NOTE`]), every mora after a
/// block's first costs `-ln P(hidden | kind)`, and a note's cells per mora should stay near the
/// phrase's.
pub fn mora_counts(lens: &[i64], moras: &[Mora]) -> Vec<usize> {
    mora_blocks(lens, moras).into_iter().map(|(_, k)| k).collect()
}

/// [`mora_counts`] with each note's first mora: `(first, count)`, the note singing `moras[first..first + count]`.
pub fn mora_blocks(lens: &[i64], moras: &[Mora]) -> Vec<(usize, usize)> {
    let stretches: Vec<Stretch> = lens.iter().map(|&len| Stretch { len, note: true }).collect();
    stretch_blocks(&stretches, moras)
}

/// A stretch of a phrase: a note, or a rest that may hold moras the notes missed.
#[derive(Clone, Copy)]
struct Stretch {
    len: i64,
    note: bool,
}

fn stretch_blocks(stretches: &[Stretch], moras: &[Mora]) -> Vec<(usize, usize)> {
    let (n, m) = (stretches.len(), moras.len());
    let attach = |k: Mora| -match k {
        Mora::Consonant => HIDDEN_CONSONANT,
        Mora::Open => HIDDEN_OPEN,
    }
    .ln();
    let cells_per_mora = stretches.iter().filter(|s| s.note).map(|s| s.len).sum::<i64>() as f64 / m.max(1) as f64;
    #[derive(Clone, Copy)]
    enum Step {
        Skip,
        Take(usize),
    }
    let mut cost = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut step = vec![vec![Step::Skip; m + 1]; n + 1];
    cost[0][0] = 0.0;
    for j in 1..=m {
        cost[0][j] = cost[0][j - 1] + MORA_WITHOUT_NOTE;
    }
    for i in 1..=n {
        let Stretch { len, note } = stretches[i - 1];
        for j in 0..=m {
            let mut best = (cost[i - 1][j] + if note { NOTE_WITHOUT_MORA } else { 0.0 }, Step::Take(0));
            if j > 0 && cost[i][j - 1] + MORA_WITHOUT_NOTE < best.0 {
                best = (cost[i][j - 1] + MORA_WITHOUT_NOTE, Step::Skip);
            }
            let mut attached = 0.0;
            for k in 1..=usize::try_from(len).unwrap_or(0).min(MAX_MORAS_PER_NOTE).min(j) {
                let c = if note {
                    if k > 1 {
                        attached += attach(moras[j - k + 1]);
                    }
                    let duration = DURATION_WEIGHT * ((len as f64 / k as f64) / cells_per_mora).ln().abs();
                    cost[i - 1][j - k] + attached + duration
                } else {
                    cost[i - 1][j - k] + MORA_IN_REST * k as f64
                };
                if c < best.0 {
                    best = (c, Step::Take(k));
                }
            }
            cost[i][j] = best.0;
            step[i][j] = best.1;
        }
    }
    let mut blocks = vec![(0, 0); n];
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match step[i][j] {
            Step::Skip => j -= 1,
            Step::Take(k) => {
                blocks[i - 1] = (j - k, k);
                j -= k;
                i -= 1;
            }
        }
    }
    blocks
}

fn cuts(note: &CellNote, k: usize) -> Vec<i64> {
    let (len, k) = (note.end - note.start, k as i64);
    if len < (k - 1) * TAIL_CELLS + 1 {
        let k = k.min(len).max(1);
        return (1..k).map(|j| note.start + ((len * j) as f64 / k as f64 + 0.5).floor() as i64).collect();
    }
    (1..k).map(|j| note.end - (k - j) * TAIL_CELLS).collect()
}

/// `notes`: one phrase, sorted and touching or not; `moras`: what the user pasted onto it. Returns the
/// phrase's notes split and merged to carry one mora each where the DP finds them; with no moras, the
/// notes unchanged.
pub fn fit_phrase(notes: &[CellNote], moras: &[Mora]) -> Vec<CellNote> {
    fit_phrase_moras(notes, moras).into_iter().map(|(n, _)| n).collect()
}

/// [`fit_phrase`], with the mora each fitted note sings: `Some(i)` for `moras[i]`, `None` for a note the
/// DP gives none. A mora the DP leaves unsung, or one beyond the pieces a short note splits into, is on
/// no note.
pub fn fit_phrase_moras(notes: &[CellNote], moras: &[Mora]) -> Vec<(CellNote, Option<usize>)> {
    if moras.is_empty() {
        return notes.iter().map(|n| (*n, None)).collect();
    }
    let lens: Vec<i64> = notes.iter().map(|n| n.end - n.start).collect();
    place_blocks(notes, &mora_blocks(&lens, moras))
}

/// [`fit_phrase_moras`] for a phrase that spans the cells `span.0..span.1` (a pasted line): a rest in
/// it, before, between or after the notes, may hold moras the notes missed, up to one per cell, at
/// [`MORA_IN_REST`] each. Those moras are on no note (a missing note the user draws). Without this,
/// a mora sung where the transcription has a gap is forced into a neighbour, and the split costs more
/// than it saves (lyrics-first 01).
pub fn fit_span_moras(notes: &[CellNote], moras: &[Mora], span: (i64, i64)) -> Vec<(CellNote, Option<usize>)> {
    if moras.is_empty() {
        return notes.iter().map(|n| (*n, None)).collect();
    }
    let mut stretches = Vec::with_capacity(2 * notes.len() + 1);
    let mut end = span.0;
    for n in notes {
        if n.start > end {
            stretches.push(Stretch { len: n.start - end, note: false });
        }
        stretches.push(Stretch { len: n.end - n.start, note: true });
        end = end.max(n.end);
    }
    if span.1 > end {
        stretches.push(Stretch { len: span.1 - end, note: false });
    }
    let blocks: Vec<(usize, usize)> = stretches.iter().zip(stretch_blocks(&stretches, moras)).filter(|(s, _)| s.note).map(|(_, b)| b).collect();
    place_blocks(notes, &blocks)
}

fn place_blocks(notes: &[CellNote], blocks: &[(usize, usize)]) -> Vec<(CellNote, Option<usize>)> {
    let mut out: Vec<(CellNote, Option<usize>)> = Vec::with_capacity(notes.len());
    for (i, (note, &(first, k))) in notes.iter().zip(blocks).enumerate() {
        let continues = i > 0 && notes[i - 1].end == note.start && notes[i - 1].pitch == note.pitch;
        match out.last_mut() {
            Some((last, _)) if k == 0 && continues && last.end == note.start => last.end = note.end,
            _ if k > 1 => {
                let (mut start, mut mora) = (note.start, first);
                for c in cuts(note, k).into_iter().filter(|&c| note.start < c && c < note.end).chain([note.end]) {
                    if c > start {
                        out.push((CellNote { start, end: c, pitch: note.pitch }, Some(mora)));
                        start = c;
                        mora += 1;
                    }
                }
            }
            _ => out.push((*note, (k == 1).then_some(first))),
        }
    }
    out
}

/// A pasted line, or a space-separated piece of one, placed in time (lyrics-first 14): it sings
/// `moras[first..]` up to the next line's `first`, over the cells `start..end` (fractional; from an
/// aligner or a user's drag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSpan {
    pub first: usize,
    pub start: f64,
    pub end: f64,
}

/// Each line's moras fitted by [`fit_span_moras`] onto the notes whose midpoint lies in its span; a note
/// in no span (or in an empty line's) carries no mora. A note belongs to the latest-starting line that
/// starts at or before its midpoint, if that line has not ended. A line that catches no note leaves its
/// moras on no note, and the lines after it are unaffected. Returns the notes, sorted, with the index
/// into `moras` each one sings.
pub fn fit_lines(notes: &[CellNote], moras: &[Mora], lines: &[LineSpan]) -> Vec<(CellNote, Option<usize>)> {
    let mut by_first: Vec<LineSpan> = lines.iter().copied().filter(|l| l.first < moras.len()).collect();
    by_first.sort_by_key(|l| l.first);
    by_first.dedup_by_key(|l| l.first);
    let ranges: Vec<(LineSpan, usize, usize)> = by_first.iter().enumerate().map(|(k, l)| (*l, l.first, by_first.get(k + 1).map_or(moras.len(), |n| n.first))).collect();
    let mut by_start = ranges;
    by_start.sort_by(|a, b| a.0.start.total_cmp(&b.0.start));
    let mut members: Vec<Vec<CellNote>> = vec![Vec::new(); by_start.len()];
    let mut placed: Vec<(CellNote, Option<usize>)> = Vec::with_capacity(notes.len());
    for n in notes {
        let mid = (n.start + n.end) as f64 / 2.0;
        let k = by_start.partition_point(|(l, _, _)| l.start <= mid);
        match k.checked_sub(1) {
            Some(k) if mid < by_start[k].0.end => members[k].push(*n),
            _ => placed.push((*n, None)),
        }
    }
    for ((line, a, b), ns) in by_start.iter().zip(&members) {
        let span = (line.start.round() as i64, line.end.round() as i64);
        placed.extend(fit_span_moras(ns, &moras[*a..*b], span).into_iter().map(|(n, m)| (n, m.map(|m| a + m))));
    }
    placed.sort_by_key(|(n, _)| (n.start, n.end));
    placed
}
