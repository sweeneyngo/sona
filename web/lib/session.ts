/**
 * Reading an annotation or a saved session into the editor's state, and
 * writing it back: the working notes, the producer's baseline and the review
 * settings. Undo (ADR-0025) snapshots the same document.
 */

import type { Annotation, BarOneEstimate, Note, WorkingNote } from "./annotation";
import { unpackContour } from "./contourStore";
import { isExpression } from "./expression";
import { parseBarOne } from "./frame";
import { isDivision } from "./grid";
import { firstOnset } from "./interpret";
import { parseProvenance, type Provenance } from "./provenance";

/** The review settings a saved session restores on load. */
export interface SessionState {
  bpm: number;
  division: number;
  gridAnchor: number;
  /** A downbeat the user chose, in stem seconds; absent or null = the first grid line (ADR-0029). */
  barOne: number | null;
  flagThreshold: number;
  snapKey: boolean;
}

/**
 * Validate the shape we depend on and coerce the fields the roll reads. Throws
 * a human-readable error if the file isn't an annotation.
 */
export function normalizeAnnotation(raw: any): Annotation {
  if (!raw || typeof raw !== "object" || !Array.isArray(raw.notes)) {
    throw new Error("expected an object with a `notes` array");
  }
  return {
    version: Number(raw.version) || 0,
    sample_rate: Number(raw.sample_rate) || 0,
    bpm: Number(raw.bpm) || 0,
    duration_s: Number(raw.duration_s) || 0,
    producer: raw.producer ?? "unknown",
    notes: raw.notes.map((n: any) => ({
      onset_s: Number(n.onset_s),
      offset_s: Number(n.offset_s),
      midi_pitch: Number(n.midi_pitch),
      confidence: Number(n.confidence),
      sub_scores: n.sub_scores ?? {},
    })),
    f0_contour: unpackContour(raw.f0_contour),
    energy_envelope: Array.isArray(raw.energy_envelope) ? raw.energy_envelope : [],
    ...withProvenance(raw.provenance),
    ...withBarOne(raw.bar_one),
  };
}

function withBarOne(raw: unknown): { bar_one?: BarOneEstimate } {
  const b = parseBarOne(raw);
  return b ? { bar_one: b } : {};
}

function withProvenance(raw: unknown): { provenance?: Provenance } {
  const p = parseProvenance(raw);
  return p ? { provenance: p } : {};
}

/** The editable set, straight from the parsed notes, so a saved session's `manual` flags and exact raw positions survive the round-trip. */
export function workingNotesFrom(notes: any[]): WorkingNote[] {
  return notes.map((n) => ({
    onset_s: Number(n.onset_s),
    offset_s: Number(n.offset_s),
    midi_pitch: Number(n.midi_pitch),
    confidence: Number(n.confidence),
    sub_scores: n.sub_scores ?? {},
    ...(n.manual ? { manual: true as const } : {}),
    ...(typeof n.lyric === "string" && n.lyric ? { lyric: n.lyric } : {}),
    ...(isExpression(n.expr) ? { expr: n.expr } : {}),
  }));
}

/**
 * The producer's original notes, the baseline the correction cost is measured
 * against: a saved session carries its own `baseline`, a fresh annotation is its
 * own baseline. Fresh objects, so editing the working notes never reaches them.
 * Confidence and sub-scores stay, so a saved session pairs every producer note's
 * scores with whether the user corrected it (older sessions may lack them).
 */
export function baselineFrom(parsed: any): Note[] {
  const baselineSource: any[] = Array.isArray(parsed.baseline) ? parsed.baseline : parsed.notes;
  return baselineSource.map((n) => ({
    onset_s: Number(n.onset_s),
    offset_s: Number(n.offset_s),
    midi_pitch: Number(n.midi_pitch),
    confidence: n.confidence !== undefined ? Number(n.confidence) : 1,
    sub_scores: n.sub_scores ?? {},
  }));
}

/**
 * The settings after loading `parsed`. A saved session's `session` block
 * restores its own; a fresh annotation takes its BPM (120 without one), puts the
 * grid phase on its first onset and bar 1 where the music put it. Anything a
 * file leaves out or gets wrong keeps its `current` value.
 */
export function settingsOnLoad(parsed: any, annotation: Annotation, notes: readonly WorkingNote[], current: SessionState): SessionState {
  const s = parsed.session;
  if (!(s && typeof s === "object")) {
    return {
      ...current,
      bpm: annotation.bpm > 0 ? annotation.bpm : 120,
      gridAnchor: firstOnset(notes),
      barOne: annotation.bar_one?.time_s ?? null,
    };
  }
  const div = Number(s.division);
  return {
    bpm: Number(s.bpm) > 0 ? Number(s.bpm) : current.bpm,
    division: isDivision(div) ? div : current.division,
    gridAnchor: Number.isFinite(Number(s.gridAnchor)) ? Number(s.gridAnchor) : firstOnset(notes),
    barOne: typeof s.barOne === "number" && Number.isFinite(s.barOne) ? s.barOne : null,
    flagThreshold: Number.isFinite(Number(s.flagThreshold)) ? Math.min(1, Math.max(0, Number(s.flagThreshold))) : current.flagThreshold,
    snapKey: s.snapKey === true,
  };
}

/** Whether `parsed` is a saved session rather than a fresh annotation. */
export function isSession(parsed: any): boolean {
  return !!(parsed.session && typeof parsed.session === "object");
}

/**
 * The full editing session: the annotation (f0 contour, energy, per-note
 * confidence, all preserved) with `notes` replaced by the working set (raw
 * seconds + `manual` flags), the producer's original notes as `baseline` (so the
 * correction cost survives a reload), and the review settings.
 */
export function sessionFile(annotation: Annotation, notes: WorkingNote[], baseline: Note[], settings: SessionState): Annotation & { baseline: Note[]; session: SessionState } {
  const { bpm, division, gridAnchor, barOne, flagThreshold, snapKey } = settings;
  return { ...annotation, notes, baseline, session: { bpm, division, gridAnchor, barOne, flagThreshold, snapKey } };
}

/** The document undo snapshots: the working notes and the review settings, as text. */
export function docText(notes: readonly WorkingNote[], settings: SessionState): string {
  const { bpm, division, gridAnchor, barOne, flagThreshold, snapKey } = settings;
  return JSON.stringify({ notes, bpm, division, gridAnchor, barOne, flagThreshold, snapKey });
}

/** A snapshot from {@link docText} back as fresh notes and settings. */
export function parseDoc(text: string): { notes: WorkingNote[]; settings: SessionState } {
  const d = JSON.parse(text);
  return {
    notes: d.notes,
    settings: { bpm: d.bpm, division: d.division, gridAnchor: d.gridAnchor, barOne: d.barOne ?? null, flagThreshold: d.flagThreshold, snapKey: d.snapKey },
  };
}
