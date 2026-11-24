/**
 * The roll's note edits as pure functions: what a drag, a resize, a
 * double-click or a delete does to a working note. A note the user touches
 * becomes `manual`, so it always snaps (see `effectiveNotes`).
 */

import type { WorkingNote } from "./annotation";
import { cellSeconds, type Frame } from "./frame";
import { snapTime } from "./grid";
import { clampPitch } from "./math";

/** A note as it was when the drag began. */
export type Grabbed = Pick<WorkingNote, "onset_s" | "offset_s" | "midi_pitch">;

export type Moved = Pick<WorkingNote, "onset_s" | "offset_s" | "midi_pitch"> & { manual: true };

export type Resized = Pick<WorkingNote, "offset_s"> & { manual: true };

/** The note dragged by `dt` seconds and `dRows` semitones: same length, never before 0 s. */
export function movedNote(from: Grabbed, dt: number, dRows: number): Moved {
  const onset_s = Math.max(0, from.onset_s + dt);
  return { onset_s, offset_s: onset_s + (from.offset_s - from.onset_s), midi_pitch: clampPitch(from.midi_pitch + dRows), manual: true };
}

/** The note's end dragged by `dt` seconds: at least half a cell long, so the snap rounds it to one cell. */
export function resizedNote(onset_s: number, from: Grabbed, dt: number, frame: Frame): Resized {
  return { offset_s: Math.max(onset_s + cellSeconds(frame) / 2, from.offset_s + dt), manual: true };
}

/** A one-cell note on the grid line nearest `t` (not before 0 s), at `pitch`. */
export function newNote(t: number, pitch: number, frame: Frame): WorkingNote {
  const s = cellSeconds(frame);
  const onset = Math.max(0, snapTime(t, s, frame.anchor));
  return {
    onset_s: onset,
    offset_s: onset + s,
    midi_pitch: clampPitch(pitch),
    confidence: 1,
    sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 1, offset_clarity: 1 },
    manual: true,
  };
}

/** The working set without `note`. */
export function withoutNote(notes: readonly WorkingNote[], note: WorkingNote): WorkingNote[] {
  return notes.filter((n) => n !== note);
}
