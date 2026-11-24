/**
 * The singer's pitch as an OpenUtau `pitd` curve: the deviation, in cents, of
 * the tracked f0 from what the notes and their expressions already sing, so
 * OpenUtau's rendered pitch follows the white line. Only voiced frames inside
 * notes count; a frame more than `maxDevCents` away (an octave slip or a stray
 * consonant) falls back to the notes.
 */

import { renderPitch, type RenderNote } from "./expression";

export interface F0Frame {
  time_s: number;
  frequency_hz: number;
  voicing: number;
}

export function isVoiced(f: F0Frame, minVoicing: number): boolean {
  return f.voicing >= minVoicing && f.frequency_hz > 0;
}

export function unvoicedAt(time_s: number): F0Frame {
  return { time_s, frequency_hz: 0, voicing: 0 };
}

export interface CurveOpts {
  /** Seconds the export moved the notes earlier (`Sequence.shiftS`). */
  shiftS: number;
  ticksPerSecond: number;
  minVoicing?: number;
  maxDevCents?: number;
  /** Frames further apart than this split the curve. */
  maxGapS?: number;
  /** OpenUtau's pen writes a point every 5 ticks. */
  intervalTicks?: number;
  /** Points closer than this to the straight line between their neighbours are dropped. */
  toleranceCents?: number;
}

export interface PitdCurve {
  xs: number[];
  ys: number[];
}

const PITD_LIMIT = 1200;

export function hzToCents(hz: number): number {
  return 1200 * Math.log2(hz / 440) + 6900;
}

function median3(a: number[]): number[] {
  return a.map((v, i) => (i === 0 || i === a.length - 1 ? v : [a[i - 1], v, a[i + 1]].sort((x, y) => x - y)[1]));
}

function douglasPeuckerKeep(xs: number[], ys: number[], tol: number): number[] {
  const keep = new Uint8Array(xs.length);
  keep[0] = keep[xs.length - 1] = 1;
  const stack: [number, number][] = [[0, xs.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop()!;
    let worst = -1;
    let worstD = tol;
    for (let i = a + 1; i < b; i++) {
      const line = ys[a] + ((ys[b] - ys[a]) * (xs[i] - xs[a])) / (xs[b] - xs[a]);
      const d = Math.abs(ys[i] - line);
      if (d > worstD) {
        worst = i;
        worstD = d;
      }
    }
    if (worst >= 0) {
      keep[worst] = 1;
      stack.push([a, worst], [worst, b]);
    }
  }
  return [...keep.keys()].filter((i) => keep[i]);
}

export function sungPitchCurve(contour: F0Frame[], notes: RenderNote[], opts: CurveOpts): PitdCurve {
  const minVoicing = opts.minVoicing ?? 0.4;
  const maxDev = opts.maxDevCents ?? 600;
  const maxGap = opts.maxGapS ?? 0.03;
  const step = opts.intervalTicks ?? 5;
  const tol = opts.toleranceCents ?? 2;
  const spans = [...notes].sort((a, b) => a.onset_s - b.onset_s);
  const inNote = (t: number) => spans.some((n) => t >= n.onset_s && t < n.offset_s);

  const voiced = contour.filter((f) => isVoiced(f, minVoicing) && inNote(f.time_s));
  const runs: F0Frame[][] = [];
  for (const f of voiced) {
    const run = runs[runs.length - 1];
    if (run && f.time_s - run[run.length - 1].time_s <= maxGap) run.push(f);
    else runs.push([f]);
  }

  const xs: number[] = [];
  const ys: number[] = [];
  const toTick = (t: number) => (t - opts.shiftS) * opts.ticksPerSecond;
  for (const run of runs) {
    const times = run.map((f) => f.time_s);
    const base = renderPitch(spans, times);
    const dev = median3(run.map((f) => hzToCents(f.frequency_hz))).map((c, i) => c - base[i] * 100);
    const ok = dev.map((d) => Math.abs(d) <= maxDev);
    let i = 0;
    while (i < run.length) {
      if (!ok[i]) {
        i++;
        continue;
      }
      let j = i;
      while (j + 1 < run.length && ok[j + 1]) j++;
      const first = Math.ceil(toTick(times[i]) / step) * step;
      const last = Math.floor(toTick(times[j]) / step) * step;
      const segX: number[] = [];
      const segY: number[] = [];
      let k = i;
      for (let x = first; x <= last; x += step) {
        if (x < 0) continue;
        while (k < j && toTick(times[k + 1]) < x) k++;
        const x0 = toTick(times[k]);
        const x1 = k < j ? toTick(times[k + 1]) : x0;
        const w = x1 > x0 ? (x - x0) / (x1 - x0) : 0;
        const y = dev[k] + (dev[Math.min(k + 1, j)] - dev[k]) * Math.max(0, Math.min(1, w));
        segX.push(x);
        segY.push(Math.round(Math.max(-PITD_LIMIT, Math.min(PITD_LIMIT, y))));
      }
      if (segX.length) {
        const before = segX[0] - step;
        const after = segX[segX.length - 1] + step;
        const lastX = xs.length ? xs[xs.length - 1] : -Infinity;
        if (before > lastX && before >= 0) {
          xs.push(before);
          ys.push(0);
        }
        for (const idx of douglasPeuckerKeep(segX, segY, tol)) {
          if (segX[idx] <= xs[xs.length - 1]) continue;
          xs.push(segX[idx]);
          ys.push(segY[idx]);
        }
        xs.push(after);
        ys.push(0);
      }
      i = j + 1;
    }
  }
  return { xs, ys };
}
