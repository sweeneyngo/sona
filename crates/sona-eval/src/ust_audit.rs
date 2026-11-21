//! **UST format audit** — does a UST actually carry the offset signal the UST
//! corpus is being built for? (`.scratch/ust-corpus-gate/issues/01`)
//!
//! A UST is contiguous: a legato note's "offset" is just the next note's onset —
//! the next-onset heuristic ADR-0007 tried and ADR-0010 retired. Only a
//! **rest-terminated** note (followed by a rest, a breath, or the end of the
//! track) carries genuine musical-end information. That fraction is the
//! headline number: it bounds how much real offset signal the corpus delivers.
//!
//! Alongside it, per UST: how idealized the timing is (where note starts fall
//! on the 16th / 32nd / triplet grids, and how far off-16th starts sit from the
//! nearest 16th), note-length extremes, and how many notes fall below sona's
//! prune floor (notes sona structurally cannot emit).
//!
//! Pure statistics over a parsed [`Ust`]; the `sona-eval-ust-audit` binary prints them.

use crate::ust::{BlockKind, Placed, Ust, UstEncoding, TICKS_PER_QUARTER};

/// 16th-note grid spacing, in ticks.
pub const SIXTEENTH: u64 = TICKS_PER_QUARTER as u64 / 4;
/// 32nd-note grid spacing, in ticks.
pub const THIRTY_SECOND: u64 = TICKS_PER_QUARTER as u64 / 8;
/// 16th-triplet grid spacing, in ticks (also contains every 8th- and
/// quarter-triplet line).
pub const TRIPLET: u64 = TICKS_PER_QUARTER as u64 / 6;

/// The COnP onset tolerance (mirrors the note metric's ±50 ms). An off-16th
/// start further than this from its nearest 16th line cannot be matched by a
/// 16th-snapped estimate, however perfect the raw detection.
pub const ONSET_TOLERANCE_S: f64 = 0.050;

/// At or above this rest-terminated fraction, most notes carry a real end.
pub const STRONG_OFFSET_SIGNAL: f64 = 0.5;
/// Below this, genuine offset labels are a small minority.
pub const THIN_OFFSET_SIGNAL: f64 = 0.25;

/// Where a set of tick positions falls on the grids. The four classes are
/// exclusive (checked in order 16th → 32nd → triplet → off-grid) and sum to the
/// number of positions; the 32nd and triplet grids only coincide on 16th lines,
/// so "32nd" and "triplet" mean *strictly* finer than a 16th.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridFit {
    /// On a 16th line.
    pub on_16th: usize,
    /// On a 32nd line but not a 16th.
    pub on_32nd: usize,
    /// On a 16th-triplet line but not a 16th.
    pub on_triplet: usize,
    /// On none of the above.
    pub off_grid: usize,
    /// For every position *not* on a 16th line: its distance to the nearest
    /// 16th line, in seconds at the local tempo — how far a 16th hard-snap moves it.
    pub off_16th_residual_s: Vec<f64>,
}

impl GridFit {
    fn add(&mut self, tick: u64, seconds_per_tick: f64) {
        if tick.is_multiple_of(SIXTEENTH) {
            self.on_16th += 1;
            return;
        }
        if tick.is_multiple_of(THIRTY_SECOND) {
            self.on_32nd += 1;
        } else if tick.is_multiple_of(TRIPLET) {
            self.on_triplet += 1;
        } else {
            self.off_grid += 1;
        }
        let r = tick % SIXTEENTH;
        self.off_16th_residual_s.push(r.min(SIXTEENTH - r) as f64 * seconds_per_tick);
    }

    /// Positions classified.
    pub fn total(&self) -> usize {
        self.on_16th + self.on_32nd + self.on_triplet + self.off_grid
    }

    /// Off-16th positions whose snap distance exceeds the onset tolerance — a
    /// 16th-snapped estimate cannot land within ±50 ms of them.
    pub fn beyond_tolerance(&self) -> usize {
        self.off_16th_residual_s.iter().filter(|&&r| r > ONSET_TOLERANCE_S + 1e-9).count()
    }

    fn merge(&mut self, other: &GridFit) {
        self.on_16th += other.on_16th;
        self.on_32nd += other.on_32nd;
        self.on_triplet += other.on_triplet;
        self.off_grid += other.off_grid;
        self.off_16th_residual_s.extend_from_slice(&other.off_16th_residual_s);
    }
}

/// The audit of one UST (or, via [`UstAudit::pool`], of many).
#[derive(Debug, Clone, PartialEq)]
pub struct UstAudit {
    /// Encoding the file was decoded with (`None` for a pooled audit).
    pub encoding: Option<UstEncoding>,
    /// Initial tempo, BPM (`None` for a pooled audit).
    pub tempo: Option<f64>,
    /// Effective mid-song tempo changes — each breaks the single-BPM assumption.
    pub tempo_changes: usize,
    /// Sung notes.
    pub notes: usize,
    /// Rest blocks.
    pub rests: usize,
    /// Breath blocks.
    pub breaths: usize,
    /// Sung notes followed by a rest, a breath, or the end of the track.
    pub rest_terminated: usize,
    /// Total sequence duration, seconds.
    pub duration_s: f64,
    /// Where sung-note starts fall on the grids.
    pub starts: GridFit,
    /// Where the *ends* of rest-terminated notes fall — the only offsets that are
    /// real labels, so their idealization matters for quantized offset scoring.
    pub rest_terminated_ends: GridFit,
    /// Shortest sung note, seconds.
    pub shortest_s: Option<f64>,
    /// Longest sung note, seconds.
    pub longest_s: Option<f64>,
    /// Sung notes shorter than the prune floor.
    pub below_prune: usize,
    /// The prune floor used, seconds.
    pub prune_s: f64,
}

impl UstAudit {
    /// Audit one UST against sona's note prune floor `prune_s` (seconds).
    pub fn of(ust: &Ust, prune_s: f64) -> Self {
        let timeline = ust.timeline();
        let mut audit = UstAudit {
            encoding: Some(ust.encoding),
            tempo: Some(ust.tempo),
            tempo_changes: ust.tempo_changes().len(),
            notes: 0,
            rests: 0,
            breaths: 0,
            rest_terminated: 0,
            duration_s: timeline.last().map_or(0.0, |p| p.end_s),
            starts: GridFit::default(),
            rest_terminated_ends: GridFit::default(),
            shortest_s: None,
            longest_s: None,
            below_prune: 0,
            prune_s,
        };

        for (k, p) in timeline.iter().enumerate() {
            match p.kind {
                BlockKind::Rest => audit.rests += 1,
                BlockKind::Breath => audit.breaths += 1,
                BlockKind::Note => {
                    audit.notes += 1;
                    let spt = seconds_per_tick(p);
                    audit.starts.add(p.start_tick, spt);

                    let dur = p.duration_s();
                    audit.shortest_s = Some(audit.shortest_s.map_or(dur, |s| s.min(dur)));
                    audit.longest_s = Some(audit.longest_s.map_or(dur, |s| s.max(dur)));
                    if dur < prune_s {
                        audit.below_prune += 1;
                    }

                    let next_is_silence = timeline.get(k + 1).is_none_or(|n| n.kind != BlockKind::Note);
                    if next_is_silence {
                        audit.rest_terminated += 1;
                        audit.rest_terminated_ends.add(p.end_tick, spt);
                    }
                }
            }
        }
        audit
    }

    /// Pool several audits (note-weighted): counts sum, residuals concatenate,
    /// extremes take the min/max. Encoding and tempo are per-song, so dropped.
    pub fn pool(audits: &[UstAudit]) -> Self {
        let mut pooled = UstAudit {
            encoding: None,
            tempo: None,
            tempo_changes: 0,
            notes: 0,
            rests: 0,
            breaths: 0,
            rest_terminated: 0,
            duration_s: 0.0,
            starts: GridFit::default(),
            rest_terminated_ends: GridFit::default(),
            shortest_s: None,
            longest_s: None,
            below_prune: 0,
            prune_s: audits.first().map_or(0.0, |a| a.prune_s),
        };
        for a in audits {
            pooled.tempo_changes += a.tempo_changes;
            pooled.notes += a.notes;
            pooled.rests += a.rests;
            pooled.breaths += a.breaths;
            pooled.rest_terminated += a.rest_terminated;
            pooled.duration_s += a.duration_s;
            pooled.starts.merge(&a.starts);
            pooled.rest_terminated_ends.merge(&a.rest_terminated_ends);
            pooled.shortest_s = min_opt(pooled.shortest_s, a.shortest_s);
            pooled.longest_s = max_opt(pooled.longest_s, a.longest_s);
            pooled.below_prune += a.below_prune;
        }
        pooled
    }

    /// The headline: rest-terminated sung notes / sung notes.
    pub fn rest_terminated_fraction(&self) -> f64 {
        ratio(self.rest_terminated, self.notes)
    }

    /// Notes whose UST "offset" is just the next onset.
    pub fn legato(&self) -> usize {
        self.notes - self.rest_terminated
    }

    /// The 16th-note duration at the initial tempo, seconds.
    pub fn sixteenth_s(&self) -> Option<f64> {
        self.tempo.map(|bpm| 60.0 / bpm / 4.0)
    }
}

/// A plain-language reading of a rest-terminated fraction: what it implies for
/// how much genuine offset signal the corpus carries.
pub fn offset_signal_reading(fraction: f64) -> &'static str {
    if fraction >= STRONG_OFFSET_SIGNAL {
        "STRONG offset signal: most sung notes end in a rest or breath, so most UST offsets are \
         genuine musical ends. COnPOff against these USTs measures real offset accuracy — the \
         corpus can gate the offset bottleneck as planned."
    } else if fraction >= THIN_OFFSET_SIGNAL {
        "MODERATE offset signal: a real but minority share of notes carry a genuine end; the rest \
         are legato, where the UST offset is just the next onset. Pooled COnPOff would mostly \
         re-measure onsets, so the in-domain gate (06) must report COnPOff on the rest-terminated \
         subset separately — that subset is the offset measurement."
    } else {
        "THIN offset signal: genuine offset labels are a small minority. The corpus is still a \
         strong onset/pitch gate, but as an *offset* gate it delivers far less than planned — \
         the plan changes shape: score offsets only on rest-terminated notes, and weigh whether \
         that subset is large enough to decide the offset question before building 02–06."
    }
}

/// Median of an ascending-sorted slice (mean of the middle pair for even
/// lengths), or `None` when empty.
pub fn median_sorted(sorted: &[f64]) -> Option<f64> {
    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

/// `num / den`, or 0 when `den` is 0.
pub fn ratio(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}

/// The local seconds-per-tick of a placed block (tempo changes included).
fn seconds_per_tick(p: &Placed) -> f64 {
    let ticks = (p.end_tick - p.start_tick) as f64;
    if ticks > 0.0 {
        p.duration_s() / ticks
    } else {
        0.0
    }
}

fn min_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

fn max_opt(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, y) => x.or(y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ust::fixtures_dir;

    const PRUNE: f64 = 0.1;

    fn audit(name: &str) -> UstAudit {
        UstAudit::of(&Ust::load(&fixtures_dir().join(name)).unwrap(), PRUNE)
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn staccato_is_fully_rest_terminated() {
        let a = audit("staccato.ust");
        assert_eq!((a.notes, a.rests, a.breaths), (5, 6, 0));
        assert_eq!(a.rest_terminated, 5);
        assert!(close(a.rest_terminated_fraction(), 1.0));
        // Every start on a 16th; at 150 BPM a 16th is 100 ms.
        assert_eq!(a.starts.on_16th, 5);
        assert!(close(a.sixteenth_s().unwrap(), 0.1));
        // Ends: さ く る on 16ths; ら (3060) and ち (3180) on 32nds.
        assert_eq!((a.rest_terminated_ends.on_16th, a.rest_terminated_ends.on_32nd), (3, 2));
        // A 32nd off a 16th at 150 BPM is exactly 50 ms — at, not beyond, tolerance.
        assert!(a.rest_terminated_ends.off_16th_residual_s.iter().all(|&r| close(r, 0.05)));
        assert_eq!(a.rest_terminated_ends.beyond_tolerance(), 0);
        // ち is a 32nd = 50 ms: below the 100 ms prune floor.
        assert!(close(a.shortest_s.unwrap(), 0.05));
        assert!(close(a.longest_s.unwrap(), 0.4));
        assert_eq!(a.below_prune, 1);
        assert!(close(a.duration_s, 3.6));
        assert_eq!(a.tempo_changes, 0);
    }

    #[test]
    fn legato_counts_breath_as_terminator() {
        let a = audit("legato.ust");
        assert_eq!((a.notes, a.rests, a.breaths), (7, 2, 1));
        // か ends at a breath, き at a rest; the other five are legato.
        assert_eq!(a.rest_terminated, 2);
        assert_eq!(a.legato(), 5);
        // あ い お き on 16ths; う え on triplets; か humanized off-grid.
        let s = &a.starts;
        assert_eq!((s.on_16th, s.on_32nd, s.on_triplet, s.off_grid), (4, 0, 2, 1));
        // Residuals (120 BPM: 1 tick = 1/960 s): う/え 40 ticks, か 30 ticks.
        let mut r = s.off_16th_residual_s.clone();
        r.sort_by(f64::total_cmp);
        let want = [30.0 / 960.0, 40.0 / 960.0, 40.0 / 960.0];
        assert!(r.iter().zip(want).all(|(g, w)| close(*g, w)), "{r:?}");
        assert_eq!(a.below_prune, 0);
    }

    #[test]
    fn tempo_change_is_counted_and_timed_locally() {
        let a = audit("tempo-change.ust");
        assert_eq!(a.tempo_changes, 1);
        assert_eq!(a.notes, 3);
        // ラ(1) runs into ラ(2) (legato); ラ(2) hits a rest; ラ(3) ends the track.
        assert_eq!(a.rest_terminated, 2);
        // At 64 BPM the 240-tick note lasts 0.46875 s, not 0.234 s.
        assert!(close(a.shortest_s.unwrap(), 0.46875));
    }

    #[test]
    fn pooling_is_note_weighted() {
        let pooled = UstAudit::pool(&[audit("staccato.ust"), audit("legato.ust")]);
        assert_eq!(pooled.notes, 12);
        assert_eq!(pooled.rest_terminated, 7);
        assert!(close(pooled.rest_terminated_fraction(), 7.0 / 12.0));
        assert_eq!(pooled.starts.total(), 12);
        assert!(close(pooled.shortest_s.unwrap(), 0.05));
        assert_eq!(pooled.tempo, None);
    }

    #[test]
    fn grid_classes_are_exclusive() {
        let mut g = GridFit::default();
        for t in [0, 120, 60, 180, 80, 160, 7, 240] {
            g.add(t, 1.0 / 960.0);
        }
        assert_eq!((g.on_16th, g.on_32nd, g.on_triplet, g.off_grid), (3, 2, 2, 1));
        assert_eq!(g.total(), 8);
        assert_eq!(g.off_16th_residual_s.len(), 5);
    }

    #[test]
    fn residual_beyond_tolerance_only_at_slow_tempi() {
        // A 32nd off the 16th grid is 60 ticks: 50 ms at 150 BPM, 62.5 ms at 120.
        let mut slow = GridFit::default();
        slow.add(60, 60.0 / (120.0 * 480.0));
        assert_eq!(slow.beyond_tolerance(), 1);
        let mut fast = GridFit::default();
        fast.add(60, 60.0 / (150.0 * 480.0));
        assert_eq!(fast.beyond_tolerance(), 0);
    }

    #[test]
    fn median_averages_the_middle_pair() {
        assert_eq!(median_sorted(&[]), None);
        assert_eq!(median_sorted(&[1.0, 2.0, 9.0]), Some(2.0));
        assert_eq!(median_sorted(&[1.0, 2.0, 4.0, 9.0]), Some(3.0));
    }

    #[test]
    fn reading_bands() {
        assert!(offset_signal_reading(0.8).starts_with("STRONG"));
        assert!(offset_signal_reading(0.3).starts_with("MODERATE"));
        assert!(offset_signal_reading(0.1).starts_with("THIN"));
    }
}
