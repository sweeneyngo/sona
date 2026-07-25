//! Maximum-cardinality bipartite matching — the shared core under the onset and
//! note metrics.
//!
//! Both onset F-measure and note P/R/F1 are defined by `mir_eval` as the *size
//! of the largest set of correspondences* between reference and estimated events
//! that satisfy the tolerance windows, with every event matched at most once
//! (`mir_eval.util.match_events` / `transcription.match_notes`). `mir_eval`
//! computes it with Hopcroft–Karp; we use Kuhn's augmenting-path algorithm,
//! which yields the same *cardinality* (all maximum matchings share it, and the
//! metrics only read the count). The graphs here are tiny — a handful of notes —
//! so the O(V·E) cost is irrelevant.
//!
//! [`matched_prf1`] is the shape both stages actually want: build the feasible
//! graph from a pairwise predicate, match it, and turn the match count into
//! Precision / Recall / F1.

use super::Prf1;

/// Score `reference` against `estimate` as the size of a maximum matching under
/// `feasible`, returned as Precision / Recall / F1.
///
/// `feasible(r, e)` says whether reference event `r` and estimate event `e` may
/// be paired (within the tolerance window). Each event is matched at most once,
/// so crowding several estimates onto one reference still scores a single hit.
/// Empty sides fall out correctly via [`Prf1::from_counts`] (no matches → zeros),
/// matching `mir_eval`'s all-zero return for an empty input.
pub fn matched_prf1<R, E>(
    reference: &[R],
    estimate: &[E],
    feasible: impl Fn(&R, &E) -> bool,
) -> Prf1 {
    let adjacency: Vec<Vec<usize>> = reference
        .iter()
        .map(|r| {
            estimate
                .iter()
                .enumerate()
                .filter(|(_, e)| feasible(r, e))
                .map(|(j, _)| j)
                .collect()
        })
        .collect();
    let matched = maximum_matching(&adjacency).len();
    Prf1::from_counts(matched, reference.len(), estimate.len())
}

/// Given the feasible edges of a bipartite graph, return one maximum-cardinality
/// matching as `(left, right)` index pairs.
///
/// `feasible[left]` lists every right vertex the `left` vertex may pair with.
/// The returned pairs are sorted by left index and are deterministic for a given
/// adjacency (Kuhn scans `feasible[left]` in order), so callers that need a
/// concrete matched set — not just its size — get a stable one.
pub fn maximum_matching(feasible: &[Vec<usize>]) -> Vec<(usize, usize)> {
    let n_left = feasible.len();

    // right_to_left[r] = Some(l) once right vertex r is claimed by left vertex l.
    // Size the table to the largest right index any left vertex can reach.
    let n_right = feasible
        .iter()
        .flatten()
        .copied()
        .max()
        .map_or(0, |r| r + 1);
    let mut right_to_left: Vec<Option<usize>> = vec![None; n_right];

    for left in 0..n_left {
        let mut seen = vec![false; right_to_left.len()];
        augment(left, feasible, &mut seen, &mut right_to_left);
    }

    let mut pairs: Vec<(usize, usize)> = right_to_left
        .iter()
        .enumerate()
        .filter_map(|(r, &l)| l.map(|l| (l, r)))
        .collect();
    pairs.sort_unstable();
    pairs
}

/// Try to match `left` to some right vertex, freeing up an earlier match by
/// following an augmenting path if necessary. Returns whether `left` got matched.
fn augment(
    left: usize,
    feasible: &[Vec<usize>],
    seen: &mut [bool],
    right_to_left: &mut [Option<usize>],
) -> bool {
    for &r in &feasible[left] {
        if seen[r] {
            continue;
        }
        seen[r] = true;
        let free_or_reassignable = match right_to_left[r] {
            None => true,
            Some(other) => augment(other, feasible, seen, right_to_left),
        };
        if free_or_reassignable {
            right_to_left[r] = Some(left);
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_matches_nothing() {
        assert!(maximum_matching(&[]).is_empty());
        assert!(maximum_matching(&[vec![], vec![]]).is_empty());
    }

    #[test]
    fn one_to_one_when_unambiguous() {
        // left 0 -> right 0, left 1 -> right 1.
        let m = maximum_matching(&[vec![0], vec![1]]);
        assert_eq!(m, vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn augments_to_reach_maximum() {
        // left 0 can take {0,1}, left 1 can take {0}. Greedy that gives 0->0
        // would strand left 1; the augmenting path must reassign 0 to left 1
        // and push left 0 onto right 1. Both get matched.
        let m = maximum_matching(&[vec![0, 1], vec![0]]);
        assert_eq!(m.len(), 2);
        let rights: Vec<usize> = m.iter().map(|&(_, r)| r).collect();
        assert!(rights.contains(&0) && rights.contains(&1));
    }

    #[test]
    fn contention_caps_at_available_rights() {
        // Two left vertices both want only right 0 — at most one can match.
        let m = maximum_matching(&[vec![0], vec![0]]);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn matched_prf1_scores_a_predicate_graph() {
        // Two references, two estimates; only equal-value pairs are feasible.
        let reference = [1u8, 2u8];
        let estimate = [2u8, 1u8];
        let p = matched_prf1(&reference, &estimate, |r, e| r == e);
        assert_eq!((p.precision, p.recall, p.f1), (1.0, 1.0, 1.0));
        // An empty estimate side yields all zeros, not a panic.
        let z = matched_prf1(&reference, &[] as &[u8], |r, e| r == e);
        assert_eq!(z.f1, 0.0);
    }
}
