/**
 * A singing-synth project puts tick 0 on a bar line, so the notes are moved back
 * by the grid **phase** to land on the editor's grid.
 */

import type { WorkingNote } from "./annotation";
import type { Expression } from "./expression";

export const TICKS_PER_QUARTER = 480;
export const DEFAULT_LYRIC = "あ";

export type LyricNote = Pick<WorkingNote, "onset_s" | "offset_s" | "midi_pitch" | "lyric" | "expr">;

export interface SeqNote {
  /** Start, in ticks from the project's tick 0. */
  position: number;
  /** Length in ticks (≥ 1). */
  duration: number;
  /** MIDI note number, 0–127. */
  tone: number;
  lyric: string;
  expr?: Expression;
}

export interface Sequence {
  notes: SeqNote[];
  /** Seconds the notes were moved earlier to sit on the editor's grid. */
  shiftS: number;
}

/** The exported notes back on the stem's timeline, in seconds, as a singing synth will play them. */
export function sequenceInSeconds(seq: Sequence, bpm: number, tpq = TICKS_PER_QUARTER): { onset_s: number; offset_s: number; tone: number; expr?: Expression }[] {
  const ticksPerSecond = (bpm / 60) * tpq;
  return seq.notes.map((n) => ({
    onset_s: n.position / ticksPerSecond + seq.shiftS,
    offset_s: (n.position + n.duration) / ticksPerSecond + seq.shiftS,
    tone: n.tone,
    expr: n.expr,
  }));
}

/** The grid phase: where in a cell the anchor sits, in `[0, spacing)`. */
export function gridPhase(anchor: number, spacing: number): number {
  if (!(spacing > 0)) return 0;
  return ((anchor % spacing) + spacing) % spacing;
}

/**
 * Convert seconds to ticks on the shifted timeline. Notes are sorted, clamped
 * at tick 0, kept monophonic (a later note trims the one it starts inside),
 * and at least one tick long.
 */
export function toSequence(notes: LyricNote[], opts: { bpm: number; spacing: number; anchor: number; tpq?: number }): Sequence {
  const tpq = opts.tpq ?? TICKS_PER_QUARTER;
  const shiftS = gridPhase(opts.anchor, opts.spacing);
  const ticksPerSecond = (opts.bpm / 60) * tpq;
  const toTicks = (s: number) => Math.max(0, Math.round((s - shiftS) * ticksPerSecond));
  const sorted = [...notes].sort((a, b) => a.onset_s - b.onset_s);
  const out: SeqNote[] = [];
  for (const n of sorted) {
    let position = toTicks(n.onset_s);
    const prev = out[out.length - 1];
    if (prev && position < prev.position + prev.duration) {
      if (position <= prev.position) position = prev.position + prev.duration;
      else prev.duration = position - prev.position;
    }
    const end = Math.max(position + 1, toTicks(n.offset_s));
    const lyric = (n.lyric ?? "").trim() || DEFAULT_LYRIC;
    out.push({ position, duration: end - position, tone: Math.max(0, Math.min(127, Math.round(n.midi_pitch))), lyric, ...(n.expr ? { expr: n.expr } : {}) });
  }
  return { notes: out, shiftS };
}
