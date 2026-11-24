//! **Edit cost** — how hard a transcription is to fix into the UST **in OpenUtau**, weighted by the
//! user's difficulty tiers (`.scratch/utau-edit-cost/spec.md`). Lower is better.
//!
//! Unlike [`super::correction_cost`], which diffs raw seconds, this scorer works on **grid cells**: a note
//! becomes `[start, end)` in whole cells of the UST's grid (`round((t − anchor) / spacing)`, anchor = UST
//! tick 0), pitch rounded to an integer. Notes shorter than half a cell vanish. Each side is made
//! monophonic (an overlapped note is cut at the next note's start) and only the UST's span counts: from
//! its first note's start to its last note's end. Output notes with no cell in that span are ignored.
//! Rounding to the UST's cells forgives a grid phase error under half a cell; a larger one belongs to
//! the frame metrics (bar offset, ticket 02).
//!
//! ## Classes
//!
//! `shared(o, r)` is the number of cells an output note `o` and a UST note `r` share. Two relations,
//! each pointing to at most one note because both sides are monophonic:
//! - `o` **covers** `r` when `2·shared(o, r) > len(r)` (a strict majority of `r`'s cells);
//! - `o` is a **piece of** `r` when `2·shared(o, r) > len(o)` (a strict majority of `o`'s cells).
//!
//! Every covers / piece-of relation is an edge between `o` and `r`. Two monophonic lines overlap
//! acyclically, so the edges form a forest. Each connected component is one of:
//! - a lone UST note: **missing** (has to be drawn);
//! - a lone output note: **extra** (has to be deleted);
//! - one UST note and one output note, a **pair**, which can carry, independently:
//!   - **pitch**: the pitches differ (any interval, octave included);
//!   - **length**: the lengths in cells differ;
//!   - **shift**: the starts differ by exactly one cell;
//!   - **moved**: the starts differ by two cells or more (outside the user's tiers; see [`WEIGHTS`]);
//! - anything larger (`a` UST notes, `b` output notes, `a + b − 1` edges): a UST note with `d` edges is
//!   **chopped** into `d − 1` extra pieces, and an output note with `d` edges is **joined** over `d − 1`
//!   hidden UST notes. Over the component that is `a + b − 2` restructurings in all. Pitch, length and
//!   shift are not scored inside such a component: rebuilding the notes includes them.
//!
//! So a pure one-cell shift costs one shift, not a shift and a length; a note one cell early that also
//! ends on time costs a shift and a length. In a legato line, one boundary a cell late costs a length
//! on the first note and a shift and a length on the second.
//!
//! ## Measures
//!
//! A measure is 4/4: `cells_per_measure` cells (the grid division: 16 for sixteenths), measure 0
//! starting at the anchor. A measure is **scored** when any note of either side (in the span) has a cell
//! in it, and **clean** when every such note belongs to an error-free pair.

use super::EditNote;

/// The kinds of edit, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Extra,
    Pitch,
    Length,
    Shift,
    Moved,
    Missing,
    Chopped,
    Joined,
}

/// The cost of one edit of each class: the user's OpenUtau tiers (2026-11-24). `Moved` (a start two or
/// more cells off) is not in the tiers; it is priced like a one-cell shift, since a drag is one action
/// whatever its distance, and counted apart so its weight can change.
pub const WEIGHTS: [(ErrorClass, u32); 8] = [
    (ErrorClass::Extra, 1),
    (ErrorClass::Pitch, 1),
    (ErrorClass::Length, 1),
    (ErrorClass::Shift, 1),
    (ErrorClass::Moved, 1),
    (ErrorClass::Missing, 2),
    (ErrorClass::Chopped, 4),
    (ErrorClass::Joined, 4),
];

impl ErrorClass {
    /// Every class, in report order.
    pub const ALL: [ErrorClass; 8] = [
        ErrorClass::Extra,
        ErrorClass::Pitch,
        ErrorClass::Length,
        ErrorClass::Shift,
        ErrorClass::Moved,
        ErrorClass::Missing,
        ErrorClass::Chopped,
        ErrorClass::Joined,
    ];

    /// This class's cost per edit, from [`WEIGHTS`].
    pub fn weight(self) -> u32 {
        WEIGHTS.iter().find(|(c, _)| *c == self).map_or(0, |(_, w)| *w)
    }

    pub fn name(self) -> &'static str {
        match self {
            ErrorClass::Extra => "extra",
            ErrorClass::Pitch => "pitch",
            ErrorClass::Length => "length",
            ErrorClass::Shift => "shift",
            ErrorClass::Moved => "moved",
            ErrorClass::Missing => "missing",
            ErrorClass::Chopped => "chopped",
            ErrorClass::Joined => "joined",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).expect("listed")
    }
}

/// The UST's grid: cell `k` starts at `anchor_s + k · spacing_s`.
#[derive(Debug, Clone, Copy)]
pub struct CellGrid {
    pub spacing_s: f64,
    /// UST tick 0 on the audio's timeline: the start of measure 0.
    pub anchor_s: f64,
    /// Cells in one 4/4 measure (the grid division).
    pub cells_per_measure: u32,
}

/// One song's edit cost, or a pool of songs.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EditCost {
    /// Edits per class, indexed like [`ErrorClass::ALL`].
    pub counts: [usize; 8],
    pub measures: usize,
    pub clean_measures: usize,
    /// Length of the UST's span, in seconds.
    pub span_s: f64,
}

impl EditCost {
    pub fn count(&self, class: ErrorClass) -> usize {
        self.counts[class.index()]
    }

    /// The weighted sum over classes.
    pub fn cost(&self) -> u64 {
        ErrorClass::ALL.iter().map(|c| self.count(*c) as u64 * u64::from(c.weight())).sum()
    }

    pub fn per_minute(&self) -> f64 {
        if self.span_s > 0.0 {
            self.cost() as f64 / (self.span_s / 60.0)
        } else {
            f64::NAN
        }
    }

    pub fn clean_share(&self) -> f64 {
        if self.measures > 0 {
            self.clean_measures as f64 / self.measures as f64
        } else {
            f64::NAN
        }
    }

    pub fn add(&mut self, o: &EditCost) {
        for (a, b) in self.counts.iter_mut().zip(o.counts) {
            *a += b;
        }
        self.measures += o.measures;
        self.clean_measures += o.clean_measures;
        self.span_s += o.span_s;
    }
}

/// A note on the UST's cells; `index` is its position in the notes it came from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cells {
    pub(crate) start: i64,
    pub(crate) end: i64,
    pub(crate) pitch: i64,
    pub(crate) index: usize,
}

impl Cells {
    pub(crate) fn len(&self) -> i64 {
        self.end - self.start
    }
    pub(crate) fn shared(&self, o: &Cells) -> i64 {
        (self.end.min(o.end) - self.start.max(o.start)).max(0)
    }
}

/// `notes` on the grid's cells: sorted, notes under half a cell dropped, overlaps cut at the next start.
pub(crate) fn to_cells(notes: &[EditNote], g: &CellGrid) -> Vec<Cells> {
    let cell = |t: f64| ((t - g.anchor_s) / g.spacing_s).round() as i64;
    let mut v: Vec<Cells> = notes
        .iter()
        .enumerate()
        .map(|(index, n)| Cells { start: cell(n.onset_s), end: cell(n.offset_s), pitch: n.midi_pitch.round() as i64, index })
        .filter(|c| c.end > c.start)
        .collect();
    v.sort_by_key(|c| (c.start, c.end));
    for i in 1..v.len() {
        let next = v[i].start;
        if v[i - 1].end > next {
            v[i - 1].end = next;
        }
    }
    v.retain(|c| c.end > c.start);
    v
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// The edit cost of fixing `output` into `reference` (the UST) on the UST's grid `g`.
pub fn edit_cost(reference: &[EditNote], output: &[EditNote], g: &CellGrid) -> EditCost {
    let refs = to_cells(reference, g);
    let (Some(first), Some(last)) = (refs.first(), refs.last()) else { return EditCost::default() };
    let (span_start, span_end) = (first.start, last.end);
    let outs: Vec<Cells> = to_cells(output, g).into_iter().filter(|o| o.end > span_start && o.start < span_end).collect();
    let (n, m) = (refs.len(), outs.len());

    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (ri, r) in refs.iter().enumerate() {
        for (oi, o) in outs.iter().enumerate() {
            let s = r.shared(o);
            if s > 0 && (2 * s > r.len() || 2 * s > o.len()) {
                edges.push((ri, oi));
            }
        }
    }

    let mut parent: Vec<usize> = (0..n + m).collect();
    let (mut deg_r, mut deg_o) = (vec![0usize; n], vec![0usize; m]);
    for &(ri, oi) in &edges {
        deg_r[ri] += 1;
        deg_o[oi] += 1;
        let (a, b) = (find(&mut parent, ri), find(&mut parent, n + oi));
        parent[a] = b;
    }
    let mut members: std::collections::BTreeMap<usize, (Vec<usize>, Vec<usize>)> = Default::default();
    for ri in 0..n {
        members.entry(find(&mut parent, ri)).or_default().0.push(ri);
    }
    for oi in 0..m {
        members.entry(find(&mut parent, n + oi)).or_default().1.push(oi);
    }

    let ix = ErrorClass::index;
    let mut counts = [0usize; 8];
    let (mut bad_r, mut bad_o) = (vec![true; n], vec![true; m]);
    for (rs, os) in members.values() {
        match (rs.as_slice(), os.as_slice()) {
            ([_], []) => counts[ix(ErrorClass::Missing)] += 1,
            ([], [_]) => counts[ix(ErrorClass::Extra)] += 1,
            ([ri], [oi]) => {
                let (r, o) = (&refs[*ri], &outs[*oi]);
                let shift = (o.start - r.start).abs();
                let errors = [
                    (r.pitch != o.pitch, ErrorClass::Pitch),
                    (r.len() != o.len(), ErrorClass::Length),
                    (shift == 1, ErrorClass::Shift),
                    (shift > 1, ErrorClass::Moved),
                ];
                for (hit, class) in errors {
                    counts[ix(class)] += usize::from(hit);
                }
                let clean = errors.iter().all(|(hit, _)| !hit);
                bad_r[*ri] = !clean;
                bad_o[*oi] = !clean;
            }
            _ => {
                counts[ix(ErrorClass::Chopped)] += rs.iter().map(|&ri| deg_r[ri] - 1).sum::<usize>();
                counts[ix(ErrorClass::Joined)] += os.iter().map(|&oi| deg_o[oi] - 1).sum::<usize>();
            }
        }
    }

    let cpm = i64::from(g.cells_per_measure.max(1));
    let mut measures: std::collections::BTreeMap<i64, bool> = Default::default();
    let notes = refs.iter().zip(bad_r).chain(outs.iter().zip(bad_o));
    for (c, bad) in notes {
        for k in c.start.div_euclid(cpm)..=(c.end - 1).div_euclid(cpm) {
            *measures.entry(k).or_insert(false) |= bad;
        }
    }
    EditCost {
        counts,
        measures: measures.len(),
        clean_measures: measures.values().filter(|bad| !**bad).count(),
        span_s: (span_end - span_start) as f64 * g.spacing_s,
    }
}

/// A 95% percentile interval of `stat` over `iters` resamples of `songs` with replacement (fixed seed,
/// so the interval is reproducible).
pub fn bootstrap(songs: &[EditCost], iters: usize, stat: impl Fn(&EditCost) -> f64) -> (f64, f64) {
    bootstrap_pooled(songs, iters, EditCost::add, stat)
}

/// [`bootstrap`] for any per-song tally that pools with `add`.
pub fn bootstrap_pooled<T: Default>(songs: &[T], iters: usize, add: impl Fn(&mut T, &T), stat: impl Fn(&T) -> f64) -> (f64, f64) {
    if songs.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut draws: Vec<f64> = (0..iters)
        .map(|_| {
            let mut pool = T::default();
            for _ in 0..songs.len() {
                add(&mut pool, &songs[(next() % songs.len() as u64) as usize]);
            }
            stat(&pool)
        })
        .collect();
    draws.sort_by(f64::total_cmp);
    let q = |x: f64| draws[((draws.len() - 1) as f64 * x).round() as usize];
    (q(0.025), q(0.975))
}

/// Song resamples behind the pooled intervals in [`report`].
pub const BOOTSTRAP_ITERS: usize = 10_000;

/// A per-song table plus the pool: weighted cost per minute and clean measures, each pooled over songs
/// with a song-bootstrap 95% interval, and the count and cost of every class.
pub fn report(title: &str, songs: &[(String, EditCost)]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let weights: Vec<String> = ErrorClass::ALL.iter().map(|c| format!("{} {}", c.name(), c.weight())).collect();
    let _ = writeln!(s, "\nedit cost, {title} (weights: {})", weights.join(" · "));
    let _ = write!(s, "  {:<28} {:>5} {:>6} {:>8} {:>13}", "song", "min", "cost", "cost/min", "clean meas");
    for c in ErrorClass::ALL {
        let _ = write!(s, " {:>7}", c.name());
    }
    s.push('\n');
    let mut pool = EditCost::default();
    let row = |s: &mut String, name: &str, e: &EditCost| {
        let _ = write!(
            s,
            "  {:<28} {:>5.2} {:>6} {:>8.1} {:>5.1}% {:>3}/{:<3}",
            name,
            e.span_s / 60.0,
            e.cost(),
            e.per_minute(),
            100.0 * e.clean_share(),
            e.clean_measures,
            e.measures
        );
        for c in ErrorClass::ALL {
            let _ = write!(s, " {:>7}", e.count(c));
        }
        s.push('\n');
    };
    for (name, e) in songs {
        row(&mut s, name, e);
        pool.add(e);
    }
    row(&mut s, &format!("POOLED ({} songs)", songs.len()), &pool);
    let all: Vec<EditCost> = songs.iter().map(|(_, e)| *e).collect();
    let (clo, chi) = bootstrap(&all, BOOTSTRAP_ITERS, EditCost::per_minute);
    let (mlo, mhi) = bootstrap(&all, BOOTSTRAP_ITERS, EditCost::clean_share);
    let _ = writeln!(
        s,
        "  pooled: {:.1} cost/min [95% {:.1}, {:.1}] · clean measures {:.1}% [95% {:.1}, {:.1}] ({BOOTSTRAP_ITERS} song resamples)",
        pool.per_minute(),
        clo,
        chi,
        100.0 * pool.clean_share(),
        100.0 * mlo,
        100.0 * mhi
    );
    let total = pool.cost().max(1) as f64;
    let parts: Vec<String> = ErrorClass::ALL
        .iter()
        .map(|c| {
            let cost = pool.count(*c) as u64 * u64::from(c.weight());
            format!("{} {} ({:.0}%)", c.name(), cost, 100.0 * cost as f64 / total)
        })
        .collect();
    let _ = writeln!(s, "  pooled cost by class: {}", parts.join(" · "));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: CellGrid = CellGrid { spacing_s: 0.125, anchor_s: 0.0, cells_per_measure: 16 };

    fn n(start: i64, end: i64, pitch: i64) -> EditNote {
        EditNote { onset_s: start as f64 * G.spacing_s, offset_s: end as f64 * G.spacing_s, midi_pitch: pitch as f64 }
    }

    fn only(e: &EditCost) -> Vec<(&'static str, usize)> {
        ErrorClass::ALL.iter().filter(|c| e.count(**c) > 0).map(|c| (c.name(), e.count(*c))).collect()
    }

    fn line() -> Vec<EditNote> {
        vec![n(0, 4, 60), n(4, 8, 62), n(12, 16, 64), n(16, 24, 65), n(32, 36, 67)]
    }

    #[test]
    fn identical_lines_cost_nothing_and_every_measure_is_clean() {
        let e = edit_cost(&line(), &line(), &G);
        assert_eq!((e.cost(), e.measures, e.clean_measures), (0, 3, 3));
        assert!((e.span_s - 36.0 * 0.125).abs() < 1e-12);
    }

    #[test]
    fn each_class_alone_costs_exactly_its_weight() {
        let cases: Vec<(ErrorClass, Vec<EditNote>, Vec<EditNote>)> = vec![
            (ErrorClass::Extra, vec![n(0, 4, 60), n(12, 16, 62)], vec![n(0, 4, 60), n(6, 10, 70), n(12, 16, 62)]),
            (ErrorClass::Pitch, vec![n(0, 4, 60)], vec![n(0, 4, 61)]),
            (ErrorClass::Pitch, vec![n(0, 4, 60)], vec![n(0, 4, 72)]),
            (ErrorClass::Length, vec![n(0, 4, 60)], vec![n(0, 6, 60)]),
            (ErrorClass::Length, vec![n(0, 8, 60)], vec![n(0, 3, 60)]),
            (ErrorClass::Shift, vec![n(4, 8, 60)], vec![n(5, 9, 60)]),
            (ErrorClass::Shift, vec![n(4, 8, 60)], vec![n(3, 7, 60)]),
            (ErrorClass::Moved, vec![n(4, 12, 60)], vec![n(7, 15, 60)]),
            (ErrorClass::Missing, vec![n(0, 4, 60), n(8, 12, 62)], vec![n(0, 4, 60)]),
            (ErrorClass::Chopped, vec![n(0, 8, 60)], vec![n(0, 4, 60), n(4, 8, 60)]),
            (ErrorClass::Joined, vec![n(0, 4, 60), n(4, 8, 60)], vec![n(0, 8, 60)]),
        ];
        for (class, reference, output) in cases {
            let e = edit_cost(&reference, &output, &G);
            assert_eq!(only(&e), vec![(class.name(), 1)], "{class:?}");
            assert_eq!(e.cost(), u64::from(class.weight()), "{class:?}");
        }
    }

    #[test]
    fn the_weights_are_the_users_tiers() {
        let w: Vec<u32> = ErrorClass::ALL.iter().map(|c| c.weight()).collect();
        assert_eq!(w, vec![1, 1, 1, 1, 1, 2, 4, 4]);
    }

    #[test]
    fn chopping_costs_per_extra_piece_and_joining_per_hidden_note() {
        let chopped = edit_cost(&[n(0, 12, 60)], &[n(0, 4, 60), n(4, 8, 61), n(8, 12, 60)], &G);
        assert_eq!((only(&chopped), chopped.cost()), (vec![("chopped", 2)], 8));
        let joined = edit_cost(&[n(0, 2, 60), n(2, 4, 60), n(4, 6, 62)], &[n(0, 6, 60)], &G);
        assert_eq!((only(&joined), joined.cost()), (vec![("joined", 2)], 8));
    }

    #[test]
    fn a_long_piece_that_covers_most_of_the_note_still_chops_it() {
        let e = edit_cost(&[n(0, 8, 60)], &[n(0, 6, 60), n(6, 8, 60)], &G);
        assert_eq!(only(&e), vec![("chopped", 1)]);
    }

    #[test]
    fn a_note_joined_with_half_of_the_next_which_is_then_chopped_is_one_of_each() {
        let e = edit_cost(&[n(0, 2, 60), n(2, 10, 62)], &[n(0, 6, 62), n(6, 10, 62)], &G);
        assert_eq!(only(&e), vec![("chopped", 1), ("joined", 1)]);
    }

    #[test]
    fn a_one_cell_overrun_into_the_next_note_is_a_length_not_a_join() {
        let e = edit_cost(&[n(0, 4, 60), n(4, 8, 62)], &[n(0, 5, 60), n(5, 8, 62)], &G);
        assert_eq!(only(&e), vec![("length", 2), ("shift", 1)]);
    }

    #[test]
    fn a_note_only_grazed_by_its_neighbours_is_missing() {
        let e = edit_cost(&[n(0, 4, 60), n(4, 8, 62), n(8, 12, 64)], &[n(0, 5, 60), n(7, 12, 64)], &G);
        assert_eq!(only(&e), vec![("length", 2), ("shift", 1), ("missing", 1)]);
    }

    #[test]
    fn an_output_note_with_a_majority_nowhere_and_covering_nothing_is_extra() {
        let e = edit_cost(&[n(0, 8, 60), n(12, 20, 62)], &[n(0, 6, 60), n(6, 12, 61), n(12, 20, 62)], &G);
        assert_eq!(only(&e), vec![("extra", 1), ("length", 1)]);
    }

    #[test]
    fn a_known_mix_costs_the_sum_of_its_parts() {
        let reference = line();
        let output = vec![
            n(0, 4, 72),
            n(4, 6, 62),
            n(6, 8, 62),
            n(12, 17, 64),
            n(17, 24, 65),
            n(26, 28, 50),
        ];
        let e = edit_cost(&reference, &output, &G);
        assert_eq!(
            only(&e),
            vec![("extra", 1), ("pitch", 1), ("length", 2), ("shift", 1), ("missing", 1), ("chopped", 1)]
        );
        assert_eq!(e.cost(), 1 + 1 + 2 + 1 + 2 + 4);
        assert_eq!((e.measures, e.clean_measures), (3, 0));
    }

    #[test]
    fn a_clean_measure_needs_every_note_touching_it_to_be_right() {
        let reference = vec![n(0, 4, 60), n(16, 20, 62), n(28, 36, 64), n(48, 52, 65)];
        let output = vec![n(0, 4, 60), n(16, 20, 63), n(28, 36, 64), n(48, 52, 65)];
        let e = edit_cost(&reference, &output, &G);
        assert_eq!((e.measures, e.clean_measures), (4, 3));
        let wrong_across_the_bar = vec![n(0, 4, 60), n(16, 20, 62), n(28, 36, 66), n(48, 52, 65)];
        let e = edit_cost(&reference, &wrong_across_the_bar, &G);
        assert_eq!((e.measures, e.clean_measures), (4, 2));
    }

    #[test]
    fn output_outside_the_usts_span_is_ignored_and_overlaps_are_cut() {
        let reference = vec![n(16, 20, 60), n(18, 24, 62)];
        let e = edit_cost(&reference, &[n(0, 8, 70), n(16, 18, 60), n(18, 24, 62), n(40, 48, 70)], &G);
        assert_eq!(e.cost(), 0);
        assert!((e.span_s - 8.0 * 0.125).abs() < 1e-12);
    }

    #[test]
    fn off_grid_times_round_to_the_nearest_cell() {
        let g = CellGrid { anchor_s: 0.3, ..G };
        let at = |c: f64| 0.3 + c * g.spacing_s;
        let reference = [EditNote { onset_s: at(0.0), offset_s: at(4.0), midi_pitch: 60.0 }];
        let output = [EditNote { onset_s: at(0.4), offset_s: at(3.6), midi_pitch: 60.2 }];
        assert_eq!(edit_cost(&reference, &output, &g).cost(), 0);
    }

    #[test]
    fn empty_reference_scores_nothing() {
        assert_eq!(edit_cost(&[], &line(), &G), EditCost::default());
        let missing_all = edit_cost(&line(), &[], &G);
        assert_eq!(only(&missing_all), vec![("missing", 5)]);
    }

    #[test]
    fn the_bootstrap_interval_brackets_the_pooled_value() {
        let song = |cost: usize, minutes: f64| EditCost { counts: [cost, 0, 0, 0, 0, 0, 0, 0], measures: 10, clean_measures: 5, span_s: minutes * 60.0 };
        let songs = [song(10, 1.0), song(30, 1.0), song(20, 2.0), song(5, 0.5)];
        let mut pool = EditCost::default();
        songs.iter().for_each(|s| pool.add(s));
        let (lo, hi) = bootstrap(&songs, 2000, EditCost::per_minute);
        assert!(lo <= pool.per_minute() && pool.per_minute() <= hi, "{lo} {} {hi}", pool.per_minute());
        assert!(lo >= 10.0 && hi <= 30.0);
        let same = [song(10, 1.0); 5];
        assert_eq!(bootstrap(&same, 100, EditCost::per_minute), (10.0, 10.0));
    }
}
