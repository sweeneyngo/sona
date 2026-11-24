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
    let (n, m) = (lens.len(), moras.len());
    let attach = |k: Mora| -match k {
        Mora::Consonant => HIDDEN_CONSONANT,
        Mora::Open => HIDDEN_OPEN,
    }
    .ln();
    let cells_per_mora = lens.iter().sum::<i64>() as f64 / m.max(1) as f64;
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
        let len = lens[i - 1];
        for j in 0..=m {
            let mut best = (cost[i - 1][j] + NOTE_WITHOUT_MORA, Step::Take(0));
            if j > 0 && cost[i][j - 1] + MORA_WITHOUT_NOTE < best.0 {
                best = (cost[i][j - 1] + MORA_WITHOUT_NOTE, Step::Skip);
            }
            let mut attached = 0.0;
            for k in 1..=usize::try_from(len).unwrap_or(0).min(MAX_MORAS_PER_NOTE).min(j) {
                if k > 1 {
                    attached += attach(moras[j - k + 1]);
                }
                let duration = DURATION_WEIGHT * ((len as f64 / k as f64) / cells_per_mora).ln().abs();
                let c = cost[i - 1][j - k] + attached + duration;
                if c < best.0 {
                    best = (c, Step::Take(k));
                }
            }
            cost[i][j] = best.0;
            step[i][j] = best.1;
        }
    }
    let mut counts = vec![0; n];
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match step[i][j] {
            Step::Skip => j -= 1,
            Step::Take(k) => {
                counts[i - 1] = k;
                j -= k;
                i -= 1;
            }
        }
    }
    counts
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
    if moras.is_empty() {
        return notes.to_vec();
    }
    let lens: Vec<i64> = notes.iter().map(|n| n.end - n.start).collect();
    let counts = mora_counts(&lens, moras);
    let mut out: Vec<CellNote> = Vec::with_capacity(notes.len());
    for (i, (note, &k)) in notes.iter().zip(&counts).enumerate() {
        let continues = i > 0 && notes[i - 1].end == note.start && notes[i - 1].pitch == note.pitch;
        match out.last_mut() {
            Some(last) if k == 0 && continues && last.end == note.start => last.end = note.end,
            _ if k > 1 => {
                let mut start = note.start;
                for c in cuts(note, k).into_iter().filter(|&c| note.start < c && c < note.end).chain([note.end]) {
                    if c > start {
                        out.push(CellNote { start, end: c, pitch: note.pitch });
                        start = c;
                    }
                }
            }
            _ => out.push(*note),
        }
    }
    out
}
