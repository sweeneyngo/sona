/**
 * Which notes the roll outlines for review (ADR-0034). A learned cell stage result flags each song's least
 * confident share of notes, because its confidence ranks the costly fixes but sits mostly above any fixed
 * cutoff (ADR-0033). Every other result keeps the threshold, whose scores mean something else.
 */

import type { Annotation, Note } from "./annotation";
import type { Reading } from "./interpret";
import type { SessionState } from "./session";

/** The share of a stage result's notes outlined until the user picks another. */
export const DEFAULT_FLAG_SHARE = 0.2;

/** Whether the learned cell stage made these notes: its provenance says so, or (older files) its notes carry the doubt. */
export function isStageResult(a: Pick<Annotation, "provenance" | "notes">): boolean {
  return a.provenance?.settings.engine === "game+cells" || a.notes.some((n) => n.stage_doubt);
}

/** The flag part of the reading: the share for a stage result, the threshold alone otherwise. */
export function flagReading(a: Pick<Annotation, "provenance" | "notes">, s: Pick<SessionState, "flagThreshold" | "flagShare">): Pick<Reading, "flagThreshold" | "flagShare"> {
  return isStageResult(a) ? { flagThreshold: s.flagThreshold, flagShare: s.flagShare } : { flagThreshold: s.flagThreshold };
}

interface Rankable {
  onset_s: number;
  offset_s: number;
  confidence: number;
  manual?: boolean;
}

/**
 * For each note, whether it is among the `share` of notes the stage is least sure of: lowest confidence
 * first, the longer of two equal ones first. A note the user has edited keeps its place but is not
 * outlined, so fixing one never outlines another.
 */
export function doubtful(notes: readonly Rankable[], share: number): boolean[] {
  const k = Math.round(Math.min(1, Math.max(0, share)) * notes.length);
  const order = notes
    .map((_, i) => i)
    .sort((a, b) => notes[a].confidence - notes[b].confidence || notes[b].offset_s - notes[b].onset_s - (notes[a].offset_s - notes[a].onset_s) || notes[a].onset_s - notes[b].onset_s);
  const out = notes.map(() => false);
  for (const i of order.slice(0, k)) out[i] = !notes[i].manual;
  return out;
}

/** Why the stage doubts a note, in a few words; `null` for a note it didn't make. */
export function doubtReason(note: Pick<Note, "confidence" | "stage_doubt">): string | null {
  const d = note.stage_doubt;
  if (!d) return null;
  return d.length >= d.unsure ? "long note, may hide several syllables" : "the model was unsure here";
}
