// Pure grid + hard-snap quantization maths for the review surface.
// Snapping lives UI-side (ADR-0004): the core emits raw seconds, and the UI
// interprets them against a user-chosen BPM/grid/phase. Kept DOM-free so the
// snapping and the confidence-aware escape hatch are unit-testable.

import type { Note, SubScores } from "./annotation";

/** The bits of a note this module reads: timing plus (optional) onset clarity. */
export interface QuantizableNote extends Pick<Note, "onset_s" | "offset_s"> {
  sub_scores?: Partial<SubScores>;
}

export interface QuantizeOptions {
  spacing: number;
  anchor?: number;
  escapeDistanceCells?: number;
  onsetConfidenceThreshold?: number;
}

export interface QuantizedNote {
  onset_s: number;
  offset_s: number;
  escaped: boolean;
}

export interface NoteFlags {
  flagged: boolean;
  lowConfidence: boolean;
  escaped: boolean;
}

/**
 * The grids the app offers, by note-value `division` (24 = 16th triplets): the
 * editor's grid menu lists them all, a `sona serve` job decodes on those marked
 * `job` (`sona transcribe --grid`), and a saved session reopens on any of them.
 */
export const GRIDS = [
  { division: 4, label: "1/4", job: false },
  { division: 8, label: "1/8", job: false },
  { division: 16, label: "1/16", job: true },
  { division: 24, label: "1/16T", job: true },
  { division: 32, label: "1/32", job: false },
] as const;

type Grid = (typeof GRIDS)[number];
export type Division = Grid["division"];
export type JobDivision = Extract<Grid, { job: true }>["division"];

export const DIVISIONS: readonly Division[] = GRIDS.map((g) => g.division);
export const JOB_GRIDS = GRIDS.filter((g): g is Extract<Grid, { job: true }> => g.job);
export const DEFAULT_DIVISION: JobDivision = 16;

export const isDivision = (d: number): d is Division => (DIVISIONS as readonly number[]).includes(d);

/** The job grid a menu value names, else the default. */
export function jobDivision(value: string): JobDivision {
  return JOB_GRIDS.find((g) => String(g.division) === value)?.division ?? DEFAULT_DIVISION;
}

/**
 * Seconds per grid cell for a note-value `division` (4 = quarter, 8 = eighth,
 * 16 = sixteenth, …) at `bpm`. A whole note is 4 beats, so a 1/division cell is
 * `4/division` beats: `240 / (bpm × division)` seconds.
 */
export function gridSpacingSeconds(bpm: number, division: number): number {
  return 240 / (bpm * division);
}

/** Nearest grid line to `t`, where lines sit at `anchor + k·spacing`. */
export function snapTime(t: number, spacing: number, anchor: number): number {
  return anchor + Math.round((t - anchor) / spacing) * spacing;
}

/**
 * Hard-snap a note's onset and offset to the grid, with a confidence-aware
 * escape hatch (ADR-0003): a note landing *far* from the grid (≥
 * `escapeDistanceCells` of a cell from the nearest line) while its onset was
 * *confidently* detected (`onset_clarity` ≥ `onsetConfidenceThreshold`) is left
 * in place and marked `escaped` — that's the tell of a real triplet/32nd run,
 * not tracking jitter. Everything else snaps, with offset pushed out to keep at
 * least a one-cell note.
 *
 * Returns `{ onset_s, offset_s, escaped }`; never mutates the input.
 */
export function quantizeNote(
  note: QuantizableNote,
  { spacing, anchor = 0, escapeDistanceCells = 0.25, onsetConfidenceThreshold = 0.8 }: QuantizeOptions,
): QuantizedNote {
  const snappedOnset = snapTime(note.onset_s, spacing, anchor);
  const deviationCells = Math.abs(note.onset_s - snappedOnset) / spacing;
  const onsetClarity = note.sub_scores?.onset_clarity ?? 0;

  if (deviationCells >= escapeDistanceCells && onsetClarity >= onsetConfidenceThreshold) {
    return { onset_s: note.onset_s, offset_s: note.offset_s, escaped: true };
  }

  let offset = snapTime(note.offset_s, spacing, anchor);
  if (offset <= snappedOnset + 1e-9) offset = snappedOnset + spacing;
  return { onset_s: snappedOnset, offset_s: offset, escaped: false };
}

/**
 * Keep a snapped line monophonic (verification/grid-notes, SnappedMonophonic).
 * Walking in onset order:
 * - **Overlap.** The later note starts inside the earlier one, so the earlier
 *   note ends where the later begins.
 * - **Collision.** Both start on the same line, so the later note moves to the
 *   earlier one's end and keeps at least one cell. No note is ever dropped.
 *
 * Returns new timings in the input order. Raw notes are untouched.
 */
export function resolveOverlaps<T extends { onset_s: number; offset_s: number }>(notes: T[], spacing: number): T[] {
  const out = notes.map((n) => ({ ...n }));
  const order = out.map((_, i) => i).sort((a, b) => out[a].onset_s - out[b].onset_s || a - b);
  for (let k = 1; k < order.length; k++) {
    const prev = out[order[k - 1]];
    const next = out[order[k]];
    if (next.onset_s >= prev.offset_s - 1e-9) continue;
    if (next.onset_s > prev.onset_s + 1e-9) {
      prev.offset_s = next.onset_s;
    } else {
      next.onset_s = prev.offset_s;
      if (next.offset_s <= next.onset_s + 1e-9) next.offset_s = next.onset_s + spacing;
    }
  }
  return out;
}

/**
 * Whether a note should be visually flagged for review, and why. A note is
 * flagged when its confidence scalar is below the (UI-slider) `threshold`, or
 * when the escape hatch spared it from snapping. Both facts are surfaced so the
 * UI can say *why*.
 */
export function noteFlags(
  note: { confidence: number },
  quantized: { escaped?: boolean },
  threshold: number,
): NoteFlags {
  const lowConfidence = note.confidence < threshold;
  const escaped = !!quantized.escaped;
  return { flagged: lowConfidence || escaped, lowConfidence, escaped };
}
