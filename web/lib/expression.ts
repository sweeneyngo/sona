/**
 * Units follow OpenUtau (`UNote.cs`, `UVibrato`, `PitchPoint`):
 * - a point's `x` is milliseconds from the note's start;
 * - a point's `y` is tenths of a semitone from the note's tone.
 *
 * `renderPitch` reproduces OpenUtau's `RenderPhrase` pitch build:
 * 1. start from each note's flat tone;
 * 2. inside a note, its vibrato replaces that;
 * 3. add each note's point curve on top, relative to the tone of whichever
 *    note owns that moment (so negative-x points bend the previous note's tail).
 */

export type PointShape = "io" | "l" | "i" | "o";

export interface PitchPoint {
  x: number;
  y: number;
  shape: PointShape;
}

export interface Vibrato {
  /** Percent of the note, measured back from its end. */
  length: number;
  /** Milliseconds. */
  period: number;
  /** Cents. */
  depth: number;
  /** Fade-in, percent of the vibrato's length. */
  in: number;
  /** Fade-out, percent of the vibrato's length. */
  out: number;
  /** Phase, percent of a period. */
  shift: number;
  /** Vertical offset, percent of the depth. */
  drift: number;
}

export interface Expression {
  points: PitchPoint[];
  /** Start the first point at the previous note's pitch when the notes touch. */
  snapFirst: boolean;
  vibrato: Vibrato | null;
}

export const NEUTRAL_POINTS: PitchPoint[] = [
  { x: -40, y: 0, shape: "io" },
  { x: 15, y: 0, shape: "io" },
];

export const NEUTRAL_EXPRESSION: Expression = { points: NEUTRAL_POINTS, snapFirst: true, vibrato: null };

export interface RenderNote {
  onset_s: number;
  offset_s: number;
  /** Integer MIDI tone. */
  tone: number;
  expr?: Expression;
}

const TOUCH_S = 1e-6;

function interpolate(shape: PointShape, x0: number, x1: number, y0: number, y1: number, x: number): number {
  if (x1 - x0 < 1e-9) return y1;
  const u = (x - x0) / (x1 - x0);
  switch (shape) {
    case "io":
      return y0 + ((y1 - y0) * (1 - Math.cos(u * Math.PI))) / 2;
    case "i":
      return y0 + (y1 - y0) * (1 - Math.cos((u * Math.PI) / 2));
    case "o":
      return y0 + (y1 - y0) * Math.sin((u * Math.PI) / 2);
    case "l":
      return y0 + (y1 - y0) * u;
  }
}

/** OpenUtau's `UVibrato.Evaluate`, in cents from the tone; `nPos`/`nPeriod` are fractions of the note. */
export function vibratoCents(v: Vibrato, nPos: number, nPeriod: number): number {
  if (v.length <= 0) return 0;
  const nStart = 1 - v.length / 100;
  const nIn = ((v.length / 100) * v.in) / 100;
  const nOut = ((v.length / 100) * v.out) / 100;
  if (nPos < nStart) return 0;
  const t = (nPos - nStart) / nPeriod + v.shift / 100;
  let y = Math.sin(2 * Math.PI * t) * v.depth + (v.depth / 100) * v.drift;
  if (nPos < nStart + nIn) y *= (nPos - nStart) / nIn;
  else if (nPos > 1 - nOut) y *= (1 - nPos) / nOut;
  return y;
}

/** The first point's y after `snapFirst`, as OpenUtau's `UNote.Validate` sets it. */
function snappedPoints(note: RenderNote, prev: RenderNote | undefined): PitchPoint[] {
  const expr = note.expr ?? NEUTRAL_EXPRESSION;
  const pts = expr.points.length ? expr.points.map((p) => ({ ...p })) : [];
  if (pts.length && expr.snapFirst) {
    pts[0].y = prev && Math.abs(prev.offset_s - note.onset_s) < TOUCH_S ? (prev.tone - note.tone) * 10 : 0;
  }
  return pts;
}

/**
 * The rendered pitch, in MIDI semitones, at each of `times` (seconds). Times
 * outside every note keep the last note's tone, as OpenUtau does.
 */
export function renderPitch(notesIn: RenderNote[], times: number[]): Float64Array {
  const notes = [...notesIn].sort((a, b) => a.onset_s - b.onset_s);
  const out = new Float64Array(times.length);
  const order = times.map((_, i) => i).sort((a, b) => times[a] - times[b]);
  let k = 0;
  let current = notes.length ? notes[0].tone * 100 : 6000;
  for (const i of order) {
    const t = times[i];
    while (k < notes.length && notes[k].offset_s <= t) {
      current = notes[k].tone * 100;
      k++;
    }
    const n = k < notes.length && notes[k].onset_s <= t ? notes[k] : undefined;
    out[i] = n ? n.tone * 100 : current;
  }
  for (const n of notes) {
    const v = n.expr?.vibrato;
    if (!v || v.length <= 0) continue;
    const dur = n.offset_s - n.onset_s;
    const nPeriod = v.period / 1000 / dur;
    times.forEach((t, i) => {
      if (t >= n.onset_s && t < n.offset_s) out[i] = n.tone * 100 + vibratoCents(v, (t - n.onset_s) / dur, nPeriod);
    });
  }
  notes.forEach((n, idx) => {
    const prev = idx > 0 ? notes[idx - 1] : undefined;
    const abs = snappedPoints(n, prev).map((p) => ({ t: n.onset_s + p.x / 1000, c: p.y * 10 + n.tone * 100, shape: p.shape }));
    if (!abs.length) {
      abs.push({ t: n.onset_s, c: n.tone * 100, shape: "io" }, { t: n.offset_s, c: n.tone * 100, shape: "io" });
    }
    if (abs[0].t > n.onset_s) abs.unshift({ t: n.onset_s, c: abs[0].c, shape: "io" });
    if (abs[abs.length - 1].t < n.offset_s) abs.push({ t: n.offset_s, c: abs[abs.length - 1].c, shape: "io" });
    times.forEach((t, i) => {
      if (t < abs[0].t || t >= abs[abs.length - 1].t) return;
      let s = 0;
      while (s < abs.length - 2 && t >= abs[s + 1].t) s++;
      const p0 = abs[s];
      const p1 = abs[s + 1];
      const pitch = interpolate(p0.shape, p0.t, p1.t, p0.c, p1.c, t);
      const base = prev && t < prev.offset_s ? prev.tone * 100 : n.tone * 100;
      out[i] += pitch - base;
    });
  });
  return out.map((c) => c / 100);
}

export const DEFAULT_VIBRATO: Vibrato = { length: 75, period: 175, depth: 25, in: 10, out: 10, shift: 0, drift: 0 };

const SHAPES: readonly PointShape[] = ["io", "l", "i", "o"];
const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

function isPitchPoint(v: unknown): v is PitchPoint {
  if (typeof v !== "object" || v === null) return false;
  const p = v as Record<string, unknown>;
  return isNum(p.x) && isNum(p.y) && SHAPES.includes(p.shape as PointShape);
}

function isVibrato(v: unknown): v is Vibrato {
  if (typeof v !== "object" || v === null) return false;
  const o = v as Record<string, unknown>;
  return (["length", "period", "depth", "in", "out", "shift", "drift"] as const).every((k) => isNum(o[k]));
}

export function isExpression(v: unknown): v is Expression {
  if (typeof v !== "object" || v === null) return false;
  const e = v as Record<string, unknown>;
  return Array.isArray(e.points) && e.points.every(isPitchPoint) && typeof e.snapFirst === "boolean" && (e.vibrato === null || isVibrato(e.vibrato));
}
