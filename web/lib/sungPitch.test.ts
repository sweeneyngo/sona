import { describe, expect, it } from "vitest";
import { renderPitch, type RenderNote } from "./expression";
import { hzToCents, sungPitchCurve, type F0Frame, type PitdCurve } from "./sungPitch";

const centsToHz = (c: number) => 440 * 2 ** ((c - 6900) / 1200);
const notes: RenderNote[] = [
  { onset_s: 1, offset_s: 2, tone: 60 },
  { onset_s: 2, offset_s: 3, tone: 64, expr: { points: [{ x: -60, y: 0, shape: "io" }, { x: 40, y: 0, shape: "io" }], snapFirst: true, vibrato: null } },
];
const glide = (t: number) => 1 / (1 + Math.exp(-(t - 2.01) / 0.015));
const sung = (t: number) => 6020 + 365 * glide(t) + 35 * Math.sin(2 * Math.PI * 5.5 * t);
const frames = (from: number, to: number, f = sung, voicing = 0.9): F0Frame[] => {
  const out: F0Frame[] = [];
  for (let t = from; t < to; t += 0.01) out.push({ time_s: t, frequency_hz: centsToHz(f(t)), voicing });
  return out;
};
const opts = { shiftS: 0.25, ticksPerSecond: 1000 };
const tickToS = (x: number) => x / opts.ticksPerSecond + opts.shiftS;

/** The curve's value at tick `x`, as OpenUtau samples it: linear between points, 0 outside. */
function samplePitd(curve: PitdCurve, x: number): number {
  const { xs, ys } = curve;
  if (!xs.length || x < xs[0] || x > xs[xs.length - 1]) return 0;
  let lo = 0;
  let hi = xs.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (xs[mid] <= x) lo = mid;
    else hi = mid;
  }
  if (xs[lo] === x || hi === lo) return ys[lo];
  return ys[lo] + ((ys[hi] - ys[lo]) * (x - xs[lo])) / (xs[hi] - xs[lo]);
}

describe("sungPitchCurve", () => {
  it("makes the notes plus the curve sing the tracked pitch", () => {
    const curve = sungPitchCurve(frames(0.9, 3.1), notes, opts);
    let worst = 0;
    for (let x = 760; x <= 2745; x += 5) {
      const t = tickToS(x);
      const total = renderPitch(notes, [t])[0] * 100 + samplePitd(curve, x);
      worst = Math.max(worst, Math.abs(total - sung(t)));
    }
    expect(worst).toBeLessThan(4);
  });

  it("writes increasing ticks and whole cents within OpenUtau's range, returning to 0 at the ends", () => {
    const { xs, ys } = sungPitchCurve(frames(0.9, 3.1), notes, opts);
    expect(xs.every((x, i) => i === 0 || x > xs[i - 1])).toBe(true);
    expect(ys.every((y) => Number.isInteger(y) && Math.abs(y) <= 1200)).toBe(true);
    expect([ys[0], ys[ys.length - 1]]).toEqual([0, 0]);
  });

  it("is far smaller than one point every 5 ticks", () => {
    const { xs } = sungPitchCurve(frames(0.9, 3.1), notes, opts);
    expect(xs.length).toBeLessThan(2000 / 5 / 2);
  });

  it("leaves the notes alone where the singer is unvoiced, outside the notes, or an octave away", () => {
    const quiet = frames(0.9, 3.1, sung, 0.1);
    expect(sungPitchCurve(quiet, notes, opts).xs).toEqual([]);
    expect(sungPitchCurve(frames(3.2, 4), notes, opts).xs).toEqual([]);
    const slip = sungPitchCurve(frames(0.9, 3.1, (t) => (t > 1.4 && t < 1.6 ? sung(t) + 1200 : sung(t))), notes, opts);
    expect(samplePitd(slip, Math.round((1.5 - opts.shiftS) * 1000))).toBe(0);
  });
});

describe("hzToCents", () => {
  it("puts A4 at 6900", () => expect(hzToCents(440)).toBe(6900));
});
