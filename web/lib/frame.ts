/**
 * The **frame** (ADR-0029): the BPM, the grid and where bar 1 falls. The editor
 * draws its cell, beat and bar lines from it, and the export puts tick 0 on one
 * of its bar lines, so a note on a beat line in sona sits on a beat in OpenUtau.
 * 4/4 only: a bar is four beats.
 */

import type { BarOneEstimate } from "./annotation";
import { gridSpacingSeconds, snapTime } from "./grid";

export const BEATS_PER_BAR = 4;

export interface Frame {
  bpm: number;
  /** Cells per whole note: 4 = quarter … 32 = thirty-second, 24 = sixteenth triplet. */
  division: number;
  /** Any grid line, in stem seconds: it fixes where the cells start. */
  anchor: number;
  /**
   * A downbeat the user chose, in stem seconds. `null` is the default, today's
   * rule: bar 1 starts on the first grid line at or after 0 s.
   */
  barOne: number | null;
}

export type LineKind = "cell" | "beat" | "bar";

export interface GridLine {
  t: number;
  kind: LineKind;
}

const ON_LINE_S = 1e-6;

export function cellSeconds(f: Frame): number {
  return gridSpacingSeconds(f.bpm, f.division);
}

export function barSeconds(f: Frame): number {
  return (BEATS_PER_BAR * 60) / f.bpm;
}

function cellsPerBeat(f: Frame): number {
  return Math.max(1, Math.round(f.division / 4));
}

/** Where in `[0, spacing)` the anchor sits. */
export function gridPhase(anchor: number, spacing: number): number {
  if (!(spacing > 0)) return 0;
  return ((anchor % spacing) + spacing) % spacing;
}

/** The downbeat the bar lines count from, on a grid line. */
export function downbeat(f: Frame): number {
  const cell = cellSeconds(f);
  return f.barOne === null ? gridPhase(f.anchor, cell) : snapTime(f.barOne, cell, f.anchor);
}

/**
 * Stem time of the export's tick 0. With the default frame it is the first grid
 * line at or after 0 s. With a chosen bar 1 it is the first bar line at or after
 * 0 s, moved back by whole bars until no note starts before it.
 */
export function tickZero(f: Frame, onsets: readonly number[]): number {
  if (f.barOne === null) return gridPhase(f.anchor, cellSeconds(f));
  const bar = barSeconds(f);
  const d = downbeat(f);
  let t0 = d - Math.floor((d + ON_LINE_S) / bar) * bar;
  const first = onsets.reduce((m, t) => Math.min(m, t), Infinity);
  if (first < t0 - ON_LINE_S) t0 -= Math.ceil((t0 - first - ON_LINE_S) / bar) * bar;
  return t0;
}

/** The grid lines in `[from, to]` (none before 0 s), each a cell, beat or bar line. */
export function gridLines(f: Frame, from: number, to: number): GridLine[] {
  const cell = cellSeconds(f);
  const perBeat = cellsPerBeat(f);
  const perBar = perBeat * BEATS_PER_BAR;
  const kDown = Math.round((downbeat(f) - f.anchor) / cell);
  const lo = Math.max(0, from);
  const out: GridLine[] = [];
  for (let k = Math.ceil((lo - f.anchor) / cell - 1e-9); ; k++) {
    const t = f.anchor + k * cell;
    if (t > to + 1e-9) break;
    if (t < -1e-9) continue;
    const j = k - kDown;
    out.push({ t, kind: mod(j, perBar) === 0 ? "bar" : mod(j, perBeat) === 0 ? "beat" : "cell" });
  }
  return out;
}

/** The bar number OpenUtau shows for a bar line at `t`, counting tick 0 as bar 1. */
export function barNumber(f: Frame, t: number, tick0: number): number {
  return Math.round((t - tick0) / barSeconds(f)) + 1;
}

export type FrameEvent =
  | { type: "tempo"; bpm: number }
  | { type: "division"; division: number }
  /** A ruler drag by `dt` seconds from the frame at its start: the grid moves, and bar 1 moves with it. */
  | { type: "drag"; from: Frame; dt: number }
  /** "This line is beat 1": the grid line nearest `t` becomes a bar line. */
  | { type: "bar-one"; t: number }
  /** Back to Auto: the estimate from the music when the annotation has one, else the default rule. */
  | { type: "auto-bar-one"; estimate?: number | null };

/** The annotation's `bar_one`, or `null` when it has none or it is malformed. */
export function parseBarOne(raw: unknown): BarOneEstimate | null {
  if (typeof raw !== "object" || raw === null) return null;
  const o = raw as Record<string, unknown>;
  const { time_s, confidence, harmonic, source, harmonic_time_s } = o;
  if (typeof time_s !== "number" || !Number.isFinite(time_s) || typeof confidence !== "number" || !(confidence >= 0 && confidence <= 1) || typeof harmonic !== "boolean") return null;
  if (source === undefined && harmonic_time_s === undefined) return { time_s, confidence, harmonic };
  if (source !== "beat-this" || typeof harmonic_time_s !== "number" || !Number.isFinite(harmonic_time_s)) return null;
  return { time_s, confidence, harmonic, source, harmonic_time_s };
}

/** Whether Beat This and the harmony put bar 1 on different beats: the case to check by ear (ADR-0036). */
export function barOneDisagrees(estimate: BarOneEstimate | null): boolean {
  return estimate?.harmonic_time_s !== undefined && Math.abs(estimate.harmonic_time_s - estimate.time_s) > 1e-3;
}

/** What the ♩ popover says bar 1 is. */
export function barOneLabel(f: Frame, estimate: BarOneEstimate | null): string {
  if (f.barOne === null) return "first grid line";
  const at = `${downbeat(f).toFixed(3)} s`;
  if (estimate === null || f.barOne !== estimate.time_s) return at;
  const pct = Math.round(estimate.confidence * 100);
  if (estimate.source === "beat-this") {
    const found = estimate.harmonic ? `${at} · Beat This (${pct}%)` : `${at} · the music's beat (Beat This found no downbeats)`;
    const harmony = estimate.harmonic_time_s;
    return harmony === undefined ? found : barOneDisagrees(estimate) ? `${found} · the harmony says ${harmony.toFixed(3)} s` : `${found} · the harmony agrees`;
  }
  return estimate.harmonic ? `${at} · from the music (${pct}%)` : `${at} · the music's beat (bar unsure, ${pct}%)`;
}

/**
 * The frame after `e`. A chosen bar 1 is a time in the stem, so a new tempo or
 * grid keeps it there (on the grid line nearest it).
 */
export function updateFrame(f: Frame, e: FrameEvent): Frame {
  switch (e.type) {
    case "tempo":
      return e.bpm > 0 && Number.isFinite(e.bpm) ? { ...f, bpm: e.bpm } : f;
    case "division":
      return e.division > 0 ? { ...f, division: e.division } : f;
    case "drag":
      return { ...e.from, anchor: e.from.anchor + e.dt, barOne: downbeat(e.from) + e.dt };
    case "bar-one":
      return { ...f, barOne: snapTime(e.t, cellSeconds(f), f.anchor) };
    case "auto-bar-one":
      return { ...f, barOne: e.estimate ?? null };
  }
}

function mod(a: number, n: number): number {
  return ((a % n) + n) % n;
}
