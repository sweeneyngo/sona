/**
 * What the notes are: the working notes read against the frame (ADR-0029) and
 * the review settings. The roll draws, the synth plays and every export writes
 * these effective notes, so their timing and pitch are decided here and nowhere
 * else (ADR-0003, ADR-0004).
 */

import type { WorkingNote } from "./annotation";
import type { RenderNote } from "./expression";
import { cellSeconds, type Frame } from "./frame";
import { noteFlags, quantizeNote, resolveOverlaps } from "./grid";
import { isInKey, snapToKey, type KeyEstimate } from "./key";
import type { NoteSpan } from "./pitchHandles";
import type { LyricNote } from "./sequence";

export interface EffectiveNote {
  raw: WorkingNote;
  onset_s: number;
  offset_s: number;
  pitch: number;
  flagged: boolean;
  lowConfidence: boolean;
  escaped: boolean;
  /** The note's pitch is outside the detected key (only when snap-to-key is off). */
  outOfKey: boolean;
}

/** The review settings that change how a note reads, besides the frame. */
export interface Reading {
  flagThreshold: number;
  /** Snap each pitch to the key's scale; off, an out-of-key pitch is only flagged. */
  snapKey: boolean;
  /** The key detected from the working notes, or `null` when there are none. */
  key: KeyEstimate | null;
}

/**
 * Each working note hard-snapped to the frame's grid (or escaped) with its flag
 * state, in the working order. Manual notes never escape: the user placed them
 * deliberately, so they always snap. The line is kept monophonic.
 */
export function effectiveNotes(working: readonly WorkingNote[], frame: Frame, reading: Reading): EffectiveNote[] {
  const s = cellSeconds(frame);
  const { key, snapKey, flagThreshold } = reading;
  const snapped = resolveOverlaps(
    working.map((n) =>
      quantizeNote(
        n,
        n.manual ? { spacing: s, anchor: frame.anchor, escapeDistanceCells: Infinity } : { spacing: s, anchor: frame.anchor },
      ),
    ),
    s,
  );
  return working.map((n, i) => {
    const q = snapped[i];
    const flags = noteFlags(n, q, flagThreshold);
    const pitch = key && snapKey ? snapToKey(n.midi_pitch, key) : n.midi_pitch;
    const outOfKey = key ? !isInKey(pitch, key) : false;
    return { raw: n, onset_s: q.onset_s, offset_s: q.offset_s, pitch, ...flags, outOfKey };
  });
}

/** The grid anchor a fresh annotation starts with: its first onset, or 0 s with no notes. */
export function firstOnset(notes: readonly { onset_s: number }[]): number {
  return notes.length ? Math.min(...notes.map((n) => n.onset_s)) : 0;
}

/** The effective notes as the export seam takes them (`exportSequence`). */
export function exportNotes(effective: readonly EffectiveNote[]): LyricNote[] {
  return effective.map((e) => ({ onset_s: e.onset_s, offset_s: e.offset_s, midi_pitch: e.pitch, lyric: e.raw.lyric, expr: e.raw.expr }));
}

/** The effective notes reshaped for the synth (stem seconds + pitch). */
export function playbackNotes(effective: readonly EffectiveNote[]) {
  return effective.map((e) => ({
    onset_s: e.onset_s,
    offset_s: e.offset_s,
    midi_pitch: e.pitch,
    confidence: e.raw.confidence,
  }));
}

/** The effective notes as the pitch curve renders them. */
export function renderNotes(effective: readonly EffectiveNote[]): RenderNote[] {
  return effective.map((e) => ({ onset_s: e.onset_s, offset_s: e.offset_s, tone: Math.round(e.pitch), expr: e.raw.expr }));
}

/** The working notes in the order they sound. */
export function notesInTimeOrder(effective: readonly EffectiveNote[]): WorkingNote[] {
  return [...effective].sort((a, b) => a.onset_s - b.onset_s).map((e) => e.raw);
}

/** Where `note` sounds, for its pitch handles; `null` when it isn't among the effective notes. */
export function noteSpan(effective: readonly EffectiveNote[], note: WorkingNote): NoteSpan | null {
  const effs = [...effective].sort((a, b) => a.onset_s - b.onset_s);
  const i = effs.findIndex((e) => e.raw === note);
  if (i < 0) return null;
  const e = effs[i];
  const prev = i > 0 && Math.abs(effs[i - 1].offset_s - e.onset_s) < 1e-6 ? Math.round(effs[i - 1].pitch) : null;
  return { onset_s: e.onset_s, offset_s: e.offset_s, tone: Math.round(e.pitch), touchingPrevTone: prev };
}
