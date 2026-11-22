import { describe, expect, it } from "vitest";
import { NEUTRAL_EXPRESSION, renderPitch, vibratoCents, type RenderNote, type Vibrato } from "./expression";
import { curveErrorCents, fitExpressions, type ContourFrame } from "./fitexpr";

const VIB: Vibrato = { length: 60, period: 180, depth: 40, in: 20, out: 10, shift: 0, drift: 0 };
const hz = (midi: number) => 440 * 2 ** ((midi - 69) / 12);
const frames = (t0: number, t1: number, f: (t: number) => number): ContourFrame[] => {
  const out: ContourFrame[] = [];
  for (let t = t0; t < t1; t += 0.005) out.push({ time_s: t, frequency_hz: hz(f(t)) });
  return out;
};

describe("renderPitch (OpenUtau semantics)", () => {
  it("is the flat tone with neutral expressions", () => {
    const notes: RenderNote[] = [
      { onset_s: 0, offset_s: 1, tone: 60 },
      { onset_s: 1.5, offset_s: 2, tone: 64 },
    ];
    expect(Array.from(renderPitch(notes, [0.5, 1.75]))).toEqual([60, 64]);
  });

  it("glides from the previous note when notes touch (snap_first), over the points' span", () => {
    const notes: RenderNote[] = [
      { onset_s: 0, offset_s: 1, tone: 60 },
      { onset_s: 1, offset_s: 2, tone: 64, expr: { points: [{ x: -100, y: 0, shape: "l" }, { x: 100, y: 0, shape: "l" }], snapFirst: true, vibrato: null } },
    ];
    const [before, mid, after] = renderPitch(notes, [0.85, 1.0, 1.15]);
    expect(before).toBeCloseTo(60, 9);
    expect(mid).toBeCloseTo(62, 9);
    expect(after).toBeCloseTo(64, 9);
  });

  it("evaluates vibrato like UVibrato.Evaluate: silent before its start, full depth after the fade", () => {
    expect(vibratoCents(VIB, 0.3, 0.18)).toBe(0);
    const nStart = 1 - VIB.length / 100;
    expect(vibratoCents(VIB, nStart + 0.25 * 0.18 + 0.2, 0.18)).toBeCloseTo(40 * Math.sin(2 * Math.PI * (0.25 + 0.2 / 0.18)), 9);
  });
});

describe("fitExpressions", () => {
  it("recovers a known vibrato's rate and depth from a contour", () => {
    const note: RenderNote = { onset_s: 0, offset_s: 1.2, tone: 67 };
    const truth: RenderNote = { ...note, expr: { ...NEUTRAL_EXPRESSION, vibrato: VIB } };
    const times = Array.from({ length: 240 }, (_, i) => i * 0.005);
    const pitch = renderPitch([truth], times);
    const contour = times.map((t, i) => ({ time_s: t, frequency_hz: hz(pitch[i]) }));
    const [fit] = fitExpressions([note], contour);
    expect(fit.vibrato).not.toBeNull();
    expect(Math.abs(fit.vibrato!.period - 180)).toBeLessThanOrEqual(15);
    expect(Math.abs(fit.vibrato!.depth - 40)).toBeLessThanOrEqual(8);
  });

  it("finds no vibrato on a steady note", () => {
    const [fit] = fitExpressions([{ onset_s: 0, offset_s: 1, tone: 60 }], frames(0, 1, () => 60.03));
    expect(fit.vibrato).toBeNull();
  });

  it("fits a glide into a touching note so the rendered curve follows the singer", () => {
    const notes: RenderNote[] = [
      { onset_s: 0, offset_s: 1, tone: 60 },
      { onset_s: 1, offset_s: 2, tone: 65 },
    ];
    const sung = (t: number) => (t < 0.95 ? 60 : t > 1.12 ? 65 : 60 + (5 * (1 - Math.cos(((t - 0.95) / 0.17) * Math.PI))) / 2);
    const contour = frames(0.05, 1.95, sung);
    const fits = fitExpressions(notes, contour);
    const fitted = notes.map((n, i) => ({ ...n, expr: fits[i] }));
    const flatErr = curveErrorCents(notes, contour, (ts) => renderPitch(notes, ts));
    const fitErr = curveErrorCents(notes, contour, (ts) => renderPitch(fitted, ts));
    expect(fitErr).toBeLessThan(flatErr * 0.5);
    expect(fits[1].points[0].x).toBeLessThan(0);
  });
});

describe("properties (seeded random)", () => {
  let seed = 11;
  const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);

  it("recovers random vibratos: period within 12%, depth within 25%", () => {
    let misses = 0;
    for (let trial = 0; trial < 60; trial++) {
      const vib: Vibrato = { length: 60 + rand() * 35, period: 130 + rand() * 120, depth: 20 + rand() * 70, in: rand() * 30, out: rand() * 20, shift: rand() * 100, drift: 0 };
      const note: RenderNote = { onset_s: 0, offset_s: 1 + rand() * 1.5, tone: 55 + Math.floor(rand() * 20) };
      const times = Array.from({ length: Math.floor(note.offset_s / 0.005) }, (_, i) => i * 0.005);
      const pitch = renderPitch([{ ...note, expr: { ...NEUTRAL_EXPRESSION, vibrato: vib } }], times);
      const [fit] = fitExpressions([note], times.map((t, i) => ({ time_s: t, frequency_hz: hz(pitch[i]) })));
      const v = fit.vibrato;
      if (!v || Math.abs(v.period - vib.period) > 0.12 * vib.period || Math.abs(v.depth - vib.depth) > 0.25 * vib.depth) misses++;
    }
    expect(misses).toBe(0);
  });

  it("renders a neutral expression exactly flat for random note sequences", () => {
    for (let trial = 0; trial < 50; trial++) {
      const notes: RenderNote[] = [];
      let t = 0;
      for (let k = 0; k < 6; k++) {
        const d = 0.05 + rand();
        notes.push({ onset_s: t, offset_s: t + d, tone: 48 + Math.floor(rand() * 30), expr: NEUTRAL_EXPRESSION });
        t += d + (rand() < 0.5 ? 0 : rand() * 0.3);
      }
      const probes = notes.map((n) => n.onset_s + 0.6 * (n.offset_s - n.onset_s));
      const out = renderPitch(notes, probes);
      notes.forEach((n, i) => {
        if (n.offset_s - n.onset_s > 0.2) expect(out[i]).toBeCloseTo(n.tone, 9);
      });
    }
  });
});
