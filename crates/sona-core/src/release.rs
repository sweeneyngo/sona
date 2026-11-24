//! Ending a grid note at its release (utau-edit-cost 12). GAME often holds a note through the short rest a
//! singer takes before the next note (a breath, or the next mora's consonant), and the UST writes that
//! rest. In such a note the vocals' energy falls away for the note's last cells. [`trim_releases`] ends
//! the note at the first quiet cell when every cell after it stays low too, if that drops at least
//! [`ReleaseRule::min_cells`] cells. The prototype and its dev numbers:
//! `crates/sona-eval/scripts/single-note-errors/`.

/// When a note's tail counts as a release. Energies are each cell's mean RMS over the note's loudest cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseRule {
    /// The first cell of the release is below this share of the note's loudest cell…
    pub quiet: f64,
    /// …and every later cell below this one (the next mora's consonant may lift the last cell).
    pub tail: f64,
    /// A release shorter than this many cells is left alone.
    pub min_cells: usize,
}

impl Default for ReleaseRule {
    fn default() -> Self {
        Self { quiet: 0.3, tail: 0.8, min_cells: 2 }
    }
}

/// Mean RMS of the frames with a time in `[a, b)`; 0 with none. `energy` is (time, RMS), sorted by time.
fn mean_energy(energy: &[(f64, f64)], a: f64, b: f64) -> f64 {
    let (i, j) = (energy.partition_point(|e| e.0 < a), energy.partition_point(|e| e.0 < b));
    if j > i {
        energy[i..j].iter().map(|e| e.1).sum::<f64>() / (j - i) as f64
    } else {
        0.0
    }
}

/// `notes` (onset, offset, MIDI) on a grid of `spacing` seconds, each ended at its release by `rule`, read
/// from the vocals' energy envelope `energy` (time, RMS). Notes only ever get shorter, keep at least one
/// cell and stay on the grid; a note under two cells is left alone.
pub fn trim_releases(notes: &[(f64, f64, i32)], energy: &[(f64, f64)], spacing: f64, rule: ReleaseRule) -> Vec<(f64, f64, i32)> {
    notes
        .iter()
        .map(|&(on, off, pitch)| {
            let n = ((off - on) / spacing).round_ties_even();
            if n.is_nan() || n < 2.0 {
                return (on, off, pitch);
            }
            let n = n as usize;
            let e: Vec<f64> = (0..n).map(|c| mean_energy(energy, on + c as f64 * spacing, on + (c + 1) as f64 * spacing)).collect();
            let loudest = e.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if loudest.is_nan() || loudest <= 0.0 {
                return (on, off, pitch);
            }
            let mut after = vec![f64::NEG_INFINITY; n + 1];
            for c in (0..n).rev() {
                after[c] = after[c + 1].max(e[c]);
            }
            let last_cut = n - rule.min_cells.clamp(1, n);
            let cut = (1..=last_cut).find(|&k| e[k] < rule.quiet * loudest && after[k + 1] < rule.tail * loudest);
            match cut {
                Some(k) => (on, on + k as f64 * spacing, pitch),
                None => (on, off, pitch),
            }
        })
        .collect()
}
