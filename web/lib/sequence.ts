/**
 * The one export seam: MIDI, UST and USTX are all written from the
 * {@link Sequence} that {@link exportSequence} builds, so every format puts a
 * note on the same tick. Tick 0 is a bar line of the editor's frame (ADR-0029).
 */

import type { WorkingNote } from "./annotation";
import type { Expression } from "./expression";
import { tickZero, type Frame } from "./frame";

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
  /** Stem time of tick 0: the seconds the notes were moved earlier. Negative when bar 1 starts before the stem. */
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

/**
 * The notes in ticks from the frame's tick 0. Notes are sorted, clamped at
 * tick 0, kept monophonic (a later note trims the one it starts inside), and at
 * least one tick long.
 */
export function exportSequence(notes: LyricNote[], frame: Frame): Sequence {
  const shiftS = tickZero(frame, notes.map((n) => n.onset_s));
  const ticksPerSecond = (frame.bpm / 60) * TICKS_PER_QUARTER;
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
