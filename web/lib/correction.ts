/**
 * **Correction cost** — how much manual fixing a producer's output needed.
 *
 * sona is judged by ear: the yardstick is "how much did a human have to
 * correct?" (see `CONTEXT.md`). This module turns that into a number by diffing
 * the producer's original notes (the *baseline*) against the user's edited set
 * (the *final*), and counting what changed: notes deleted (spurious/bleed), notes
 * added (misses), and matched notes whose pitch or timing was corrected.
 *
 * The diff is retrospective — no per-edit logging is needed. The review surface
 * already keeps both halves (the loaded annotation's notes and the working set),
 * so the cost is a pure function of the two.
 *
 * ## Two thresholds, not one
 * Matching pairs notes by **onset proximity, ignoring pitch** ({@link
 * MatchParams.matchWindow_s}): a pitch *correction* keeps the note but changes
 * its pitch, so requiring pitch-to-match would split every pitch fix into a
 * delete + an add and lose the breakdown. Once paired, tighter per-field
 * thresholds decide whether onset/offset actually moved. Timing corrections
 * larger than the match window read as a delete + an add — still counted, just
 * not itemised as a move.
 *
 * The onset/offset change thresholds default to the ±50 ms the Rust eval harness
 * uses for note matching (ADR-0011), so the two stay consistent.
 */

/** The minimum shape this module needs from a note (raw seconds, integer MIDI). */
export interface NoteLike {
  onset_s: number;
  offset_s: number;
  midi_pitch: number;
}

/** Thresholds governing how baseline and final notes are paired and compared. */
export interface MatchParams {
  /** Max onset separation, in seconds, for two notes to pair as "the same note". */
  matchWindow_s: number;
  /** Onset shift, in seconds, beyond which a matched note counts as onset-corrected. */
  onsetTol_s: number;
  /** Offset shift, in seconds, beyond which a matched note counts as offset-corrected. */
  offsetTol_s: number;
}

/** Default thresholds; onset/offset tolerances mirror the Rust harness (ADR-0011). */
export const DEFAULT_MATCH_PARAMS: MatchParams = {
  matchWindow_s: 0.15,
  onsetTol_s: 0.05,
  offsetTol_s: 0.05,
};

/** A baseline note paired to the final note it was corrected into. */
export interface NotePair {
  baseline: NoteLike;
  final: NoteLike;
  baselineIndex: number;
  finalIndex: number;
}

/** The result of aligning a baseline note set to a final one. */
export interface Alignment {
  /** Paired baseline↔final notes (the same note, possibly corrected). */
  matched: NotePair[];
  /** Baseline notes with no final match — spurious notes the user removed. */
  deleted: NoteLike[];
  /** Final notes with no baseline match — misses the user added. */
  added: NoteLike[];
}

/** The kind of a single correction. */
export type ChangeKind = "added" | "deleted" | "pitch" | "onset" | "offset";

/**
 * One correction that contributes to the cost. `from` is the baseline note,
 * `to` the final note: an addition has only `to`, a deletion only `from`, and a
 * matched change (pitch/onset/offset) carries both.
 */
export interface Change {
  kind: ChangeKind;
  from?: NoteLike;
  to?: NoteLike;
}

/** Per-category correction counts plus a weighted roll-up. */
export interface CorrectionCost {
  /** Notes the user added (misses). */
  added: number;
  /** Notes the user deleted (false positives / bleed). */
  deleted: number;
  /** Matched notes whose MIDI pitch changed. */
  pitchFixed: number;
  /** Matched notes whose onset moved beyond {@link MatchParams.onsetTol_s}. */
  onsetMoved: number;
  /** Matched notes whose offset moved beyond {@link MatchParams.offsetTol_s}. */
  offsetMoved: number;
  /**
   * Weighted sum of the categories above — a single "how much correction"
   * scalar. Keep the raw counts: the weights are a first guess, to be calibrated
   * later against which corrections users actually make.
   */
  total: number;
}

/** Relative cost of each correction kind. All 1 until calibrated. */
export interface CostWeights {
  added: number;
  deleted: number;
  pitchFixed: number;
  onsetMoved: number;
  offsetMoved: number;
}

/** Equal weights — a deliberate placeholder until real sessions calibrate them. */
export const DEFAULT_COST_WEIGHTS: CostWeights = {
  added: 1,
  deleted: 1,
  pitchFixed: 1,
  onsetMoved: 1,
  offsetMoved: 1,
};

/**
 * Pair baseline notes to final notes one-to-one by nearest onset, feasible only
 * within {@link MatchParams.matchWindow_s}. Greedy over all feasible pairs in
 * ascending onset distance — deterministic, and optimal for the monophonic,
 * sequential notes sona produces (one note sounds at a time).
 */
export function alignNotes(
  baseline: readonly NoteLike[],
  final: readonly NoteLike[],
  params: MatchParams = DEFAULT_MATCH_PARAMS,
): Alignment {
  interface Candidate {
    i: number;
    j: number;
    dist: number;
  }
  const candidates: Candidate[] = [];
  for (let i = 0; i < baseline.length; i++) {
    for (let j = 0; j < final.length; j++) {
      const dist = Math.abs(baseline[i].onset_s - final[j].onset_s);
      if (dist <= params.matchWindow_s) candidates.push({ i, j, dist });
    }
  }
  // Nearest first; break ties by index so the result is stable.
  candidates.sort((a, b) => a.dist - b.dist || a.i - b.i || a.j - b.j);

  const usedBaseline = new Set<number>();
  const usedFinal = new Set<number>();
  const matched: NotePair[] = [];
  for (const c of candidates) {
    if (usedBaseline.has(c.i) || usedFinal.has(c.j)) continue;
    usedBaseline.add(c.i);
    usedFinal.add(c.j);
    matched.push({
      baseline: baseline[c.i],
      final: final[c.j],
      baselineIndex: c.i,
      finalIndex: c.j,
    });
  }
  // Keep matched in baseline order for a readable, stable diff.
  matched.sort((a, b) => a.baselineIndex - b.baselineIndex);

  const deleted = baseline.filter((_, i) => !usedBaseline.has(i));
  const added = final.filter((_, j) => !usedFinal.has(j));
  return { matched, deleted, added };
}

/**
 * Every individual correction turning `baseline` into `final`, as a flat list:
 * one entry per added / deleted note and per changed field of a matched note. A
 * matched note both re-pitched and nudged yields two entries. The list length
 * equals {@link CorrectionCost.total} under default (unit) weights.
 */
export function correctionChanges(
  baseline: readonly NoteLike[],
  final: readonly NoteLike[],
  params: MatchParams = DEFAULT_MATCH_PARAMS,
): Change[] {
  const { matched, deleted, added } = alignNotes(baseline, final, params);
  const changes: Change[] = [];
  for (const note of deleted) changes.push({ kind: "deleted", from: note });
  for (const note of added) changes.push({ kind: "added", to: note });
  for (const { baseline: b, final: f } of matched) {
    if (b.midi_pitch !== f.midi_pitch) changes.push({ kind: "pitch", from: b, to: f });
    if (Math.abs(b.onset_s - f.onset_s) > params.onsetTol_s) changes.push({ kind: "onset", from: b, to: f });
    if (Math.abs(b.offset_s - f.offset_s) > params.offsetTol_s) changes.push({ kind: "offset", from: b, to: f });
  }
  return changes;
}

/**
 * The correction cost of turning `baseline` into `final`: how many notes were
 * added, deleted, or had their pitch/onset/offset corrected, plus a weighted
 * scalar. A single matched note can count in several categories at once (e.g. a
 * note that was both re-pitched and nudged later).
 */
export function correctionCost(
  baseline: readonly NoteLike[],
  final: readonly NoteLike[],
  params: MatchParams = DEFAULT_MATCH_PARAMS,
  weights: CostWeights = DEFAULT_COST_WEIGHTS,
): CorrectionCost {
  const counts = { added: 0, deleted: 0, pitchFixed: 0, onsetMoved: 0, offsetMoved: 0 };
  for (const change of correctionChanges(baseline, final, params)) {
    if (change.kind === "added") counts.added++;
    else if (change.kind === "deleted") counts.deleted++;
    else if (change.kind === "pitch") counts.pitchFixed++;
    else if (change.kind === "onset") counts.onsetMoved++;
    else counts.offsetMoved++;
  }
  const total =
    counts.added * weights.added +
    counts.deleted * weights.deleted +
    counts.pitchFixed * weights.pitchFixed +
    counts.onsetMoved * weights.onsetMoved +
    counts.offsetMoved * weights.offsetMoved;

  return { ...counts, total };
}
