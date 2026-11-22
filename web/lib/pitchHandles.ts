/**
 * Direct-manipulation handles for a note's pitch expression (ADR-0019): its
 * portamento points and its vibrato envelope, positioned in note time and
 * cents, so the roll can draw them and turn drags into new expressions.
 * Limits are OpenUtau's (`UVibrato` setters); points keep their order.
 */

import type { Expression, PitchPoint, Vibrato } from "./expression";

export type Handle =
  | { kind: "point"; index: number }
  | { kind: "vibrato-start" }
  | { kind: "vibrato-fade-in" }
  | { kind: "vibrato-fade-out" }
  | { kind: "vibrato-period" };

export interface NoteSpan {
  onset_s: number;
  offset_s: number;
  /** Integer MIDI tone. */
  tone: number;
  /** The previous note's tone when it ends exactly at this onset (snap_first glides from it). */
  touchingPrevTone: number | null;
}

/** A handle's place: seconds on the timeline and pitch in fractional MIDI. */
export interface HandleAt {
  handle: Handle;
  t: number;
  pitch: number;
}

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));

/** The y a point really has: snap_first pins the first point to the previous note's pitch. */
function effectiveY(expr: Expression, i: number, span: NoteSpan): number {
  if (i === 0 && expr.snapFirst) return span.touchingPrevTone === null ? 0 : (span.touchingPrevTone - span.tone) * 10;
  return expr.points[i].y;
}

function vibratoGeometry(v: Vibrato, span: NoteSpan) {
  const dur = span.offset_s - span.onset_s;
  const start = span.offset_s - (dur * v.length) / 100;
  const len = span.offset_s - start;
  return { dur, start, len, fadeInEnd: start + (len * v.in) / 100, fadeOutStart: span.offset_s - (len * v.out) / 100 };
}

export function handlesFor(expr: Expression, span: NoteSpan): HandleAt[] {
  const out: HandleAt[] = expr.points.map((p, index) => ({
    handle: { kind: "point", index },
    t: span.onset_s + p.x / 1000,
    pitch: span.tone + effectiveY(expr, index, span) / 10,
  }));
  const v = expr.vibrato;
  if (v && v.length > 0) {
    const g = vibratoGeometry(v, span);
    const top = span.tone + v.depth / 100;
    out.push(
      { handle: { kind: "vibrato-start" }, t: g.start, pitch: span.tone },
      { handle: { kind: "vibrato-fade-in" }, t: g.fadeInEnd, pitch: top },
      { handle: { kind: "vibrato-fade-out" }, t: g.fadeOutStart, pitch: top },
      { handle: { kind: "vibrato-period" }, t: g.start + v.period / 1000, pitch: span.tone - v.depth / 100 },
    );
  }
  return out;
}

/**
 * The expression after dragging `handle` from its position in `orig` by `dt`
 * seconds and `dCents` cents. Points stay between their neighbours; the first
 * point's height stays snapped while `snapFirst` is on.
 */
export function dragHandle(orig: Expression, span: NoteSpan, handle: Handle, dt: number, dCents: number): Expression {
  const expr: Expression = { ...orig, points: orig.points.map((p) => ({ ...p })), vibrato: orig.vibrato && { ...orig.vibrato } };
  if (handle.kind === "point") {
    const i = handle.index;
    const p = expr.points[i];
    const lo = i > 0 ? orig.points[i - 1].x + 1 : -Infinity;
    const hi = i < orig.points.length - 1 ? orig.points[i + 1].x - 1 : Infinity;
    p.x = Math.round(clamp(orig.points[i].x + dt * 1000, lo, hi));
    if (!(i === 0 && expr.snapFirst)) p.y = Math.round((orig.points[i].y + dCents / 10) * 10) / 10;
    return expr;
  }
  const v = expr.vibrato;
  const o = orig.vibrato;
  if (!v || !o) return expr;
  const g = vibratoGeometry(o, span);
  switch (handle.kind) {
    case "vibrato-start": {
      const start = clamp(g.start + dt, span.onset_s, span.offset_s);
      v.length = Math.round(clamp(((span.offset_s - start) / g.dur) * 100, 0, 100));
      break;
    }
    case "vibrato-fade-in": {
      v.in = Math.round(clamp(((g.fadeInEnd + dt - g.start) / g.len) * 100, 0, 100 - o.out));
      v.depth = Math.round(clamp(o.depth + dCents, 5, 200));
      break;
    }
    case "vibrato-fade-out": {
      v.out = Math.round(clamp(((span.offset_s - (g.fadeOutStart + dt)) / g.len) * 100, 0, 100 - o.in));
      v.depth = Math.round(clamp(o.depth + dCents, 5, 200));
      break;
    }
    case "vibrato-period":
      v.period = Math.round(clamp(o.period + dt * 1000, 5, 500));
      break;
  }
  return expr;
}

/** Insert a point at `xMs` (from the note's start) and height `yTenths`, keeping order. */
export function addPoint(expr: Expression, xMs: number, yTenths: number): { expr: Expression; index: number } {
  const points: PitchPoint[] = expr.points.map((p) => ({ ...p }));
  const x = Math.round(xMs);
  if (points.some((p) => p.x === x)) return { expr, index: points.findIndex((p) => p.x === x) };
  const index = points.findIndex((p) => p.x > x);
  const at = index < 0 ? points.length : index;
  points.splice(at, 0, { x, y: Math.round(yTenths * 10) / 10, shape: "io" });
  return { expr: { ...expr, points }, index: at };
}

/** Remove point `index`; a pitch curve keeps at least two points. */
export function removePoint(expr: Expression, index: number): Expression {
  if (expr.points.length <= 2) return expr;
  return { ...expr, points: expr.points.filter((_, i) => i !== index) };
}
