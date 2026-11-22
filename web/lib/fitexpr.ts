import { NEUTRAL_POINTS, type Expression, type PitchPoint, type RenderNote, type Vibrato } from "./expression";

export interface ContourFrame {
  time_s: number;
  frequency_hz: number;
}

export interface FitOptions {
  /** A frame within this many cents of the target counts as settled. */
  settleCents: number;
  /** How long the pitch must stay settled, seconds. */
  settleHoldS: number;
  /** Longest glide searched before the onset, seconds, and as a share of the previous note. */
  maxLeadS: number;
  maxLeadShare: number;
  /** A note's tail further than this from its tone before a rest gets an end fall. */
  minTailBendCents: number;
  minRateHz: number;
  maxRateHz: number;
  /** A vibrato needs this autocorrelation peak and this depth. */
  minPeriodicity: number;
  minDepthCents: number;
  /** A note needs this much steady time after its glide for a vibrato. */
  minVibratoS: number;
}

export const DEFAULT_FIT: FitOptions = {
  settleCents: 35,
  settleHoldS: 0.04,
  maxLeadS: 0.2,
  maxLeadShare: 0.6,
  minTailBendCents: 60,
  minRateHz: 3.5,
  maxRateHz: 9,
  minPeriodicity: 0.45,
  minDepthCents: 15,
  minVibratoS: 0.3,
};

const OCTAVE_SLIP_OR_BLEED_CENTS = 700;
const TOUCH_S = 1e-6;

const toMidi = (hz: number) => 69 + 12 * Math.log2(hz / 440);

interface Sample {
  t: number;
  c: number;
}

/** The song's tuning offset in cents: circular mean of the voiced frames' distance from their notes. */
export function tuningCents(notes: RenderNote[], contour: ContourFrame[]): number {
  let x = 0;
  let y = 0;
  for (const s of notesSamples(notes, contour, 0)) {
    const a = (2 * Math.PI * s.c) / 100;
    x += Math.cos(a);
    y += Math.sin(a);
  }
  return x === 0 && y === 0 ? 0 : (Math.atan2(y, x) / (2 * Math.PI)) * 100;
}

function voicedCents(frame: ContourFrame, tone: number, tuning: number): number | null {
  if (!(frame.frequency_hz > 0)) return null;
  const c = (toMidi(frame.frequency_hz) - tone) * 100 - tuning;
  return Math.abs(c) <= OCTAVE_SLIP_OR_BLEED_CENTS ? c : null;
}

function* notesSamples(notes: RenderNote[], contour: ContourFrame[], tuning: number): Generator<Sample & { tone: number }> {
  let j = 0;
  for (const n of [...notes].sort((a, b) => a.onset_s - b.onset_s)) {
    while (j < contour.length && contour[j].time_s < n.onset_s) j++;
    for (let k = j; k < contour.length && contour[k].time_s < n.offset_s; k++) {
      const c = voicedCents(contour[k], n.tone, tuning);
      if (c !== null) yield { t: contour[k].time_s, c, tone: n.tone };
    }
  }
}

function voicedCentsBetween(contour: ContourFrame[], t0: number, t1: number, tone: number, tuning: number): Sample[] {
  let lo = 0;
  let hi = contour.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (contour[mid].time_s < t0) lo = mid + 1;
    else hi = mid;
  }
  const out: Sample[] = [];
  for (let k = lo; k < contour.length && contour[k].time_s < t1; k++) {
    const c = voicedCents(contour[k], tone, tuning);
    if (c !== null) out.push({ t: contour[k].time_s, c });
  }
  return out;
}

function settleTime(s: Sample[], from: number, opt: FitOptions): number | null {
  let start: number | null = null;
  for (const p of s) {
    if (p.t < from) continue;
    if (Math.abs(p.c) <= opt.settleCents) {
      start ??= p.t;
      if (p.t - start >= opt.settleHoldS) return start;
    } else {
      start = null;
    }
  }
  return null;
}

function fitPortamento(n: RenderNote, prev: RenderNote | undefined, contour: ContourFrame[], tuning: number, opt: FitOptions): { points: PitchPoint[]; snapFirst: boolean; settled: number } | null {
  const dur = n.offset_s - n.onset_s;
  const touching = !!prev && Math.abs(prev.offset_s - n.onset_s) < TOUCH_S && prev.tone !== n.tone;
  const lead = touching && prev ? Math.min(opt.maxLeadS, opt.maxLeadShare * (prev.offset_s - prev.onset_s)) : 0.03;
  const s = voicedCentsBetween(contour, n.onset_s - lead, n.onset_s + Math.min(0.35, 0.6 * dur), n.tone, tuning);
  if (s.length < 3) return null;
  const settled = settleTime(s, n.onset_s - lead, opt) ?? n.onset_s + 0.4 * dur;
  const endMs = Math.round((settled - n.onset_s) * 1000);
  if (touching && prev) {
    const prevC = (prev.tone - n.tone) * 100;
    let startT = n.onset_s - lead;
    for (const p of s) {
      if (p.t >= settled) break;
      if (Math.abs(p.c - prevC) <= opt.settleCents) startT = p.t;
    }
    const startMs = Math.round((Math.min(startT, settled - 0.02) - n.onset_s) * 1000);
    return { points: [{ x: startMs, y: 0, shape: "io" }, { x: Math.max(startMs + 10, endMs), y: 0, shape: "io" }], snapFirst: true, settled };
  }
  const first = s.find((p) => p.t >= n.onset_s);
  if (!first || Math.abs(first.c) < 50 || endMs <= 10) return { points: NEUTRAL_POINTS, snapFirst: true, settled };
  const y0 = Math.round(Math.max(-120, Math.min(120, first.c / 10)));
  return { points: [{ x: 0, y: y0, shape: "io" }, { x: endMs, y: 0, shape: "io" }], snapFirst: false, settled };
}

function movingAverage(s: Sample[], halfWidthS: number): number[] {
  const out: number[] = [];
  let lo = 0;
  let hi = 0;
  let sum = 0;
  for (const p of s) {
    while (hi < s.length && s[hi].t <= p.t + halfWidthS) sum += s[hi++].c;
    while (s[lo].t < p.t - halfWidthS) sum -= s[lo++].c;
    out.push(sum / (hi - lo));
  }
  return out;
}

/** The movingAverage detrend is truncated at a segment's edges, inflating the envelope there. */
function interiorPercentile(values: number[], edge: number, q: number): number {
  const interior = values.length > 3 * edge ? values.slice(edge, values.length - edge) : values;
  const sorted = [...interior].sort((a, b) => a - b);
  return sorted[Math.floor(q * (sorted.length - 1))];
}

function fitVibrato(n: RenderNote, from: number, contour: ContourFrame[], tuning: number, opt: FitOptions): Vibrato | null {
  const dur = n.offset_s - n.onset_s;
  const end = n.offset_s - 0.02;
  if (end - from < opt.minVibratoS) return null;
  const raw = voicedCentsBetween(contour, from, end, n.tone, tuning);
  if (raw.length < 20) return null;
  const step = (raw[raw.length - 1].t - raw[0].t) / (raw.length - 1);
  if (!(step > 0)) return null;
  const trend = movingAverage(raw, 0.5 / opt.minRateHz);
  const r = raw.map((p, i) => p.c - trend[i]);
  const energy = r.reduce((a, v) => a + v * v, 0);
  if (energy <= 0) return null;
  let bestLag = 0;
  let bestCorr = -1;
  for (let lag = Math.max(1, Math.floor(1 / opt.maxRateHz / step)); lag <= Math.ceil(1 / opt.minRateHz / step) && lag < r.length / 2; lag++) {
    let acc = 0;
    for (let i = 0; i + lag < r.length; i++) acc += r[i] * r[i + lag];
    const corr = (acc / energy) * (r.length / (r.length - lag));
    if (corr > bestCorr) {
      bestCorr = corr;
      bestLag = lag;
    }
  }
  if (!bestLag || bestCorr < opt.minPeriodicity) return null;
  const periodS = bestLag * step;
  const win = Math.max(1, Math.round(periodS / step));
  const env = r.map((_, i) => {
    let a = 0;
    let k = 0;
    for (let j = Math.max(0, i - (win >> 1)); j < Math.min(r.length, i + (win >> 1) + 1); j++, k++) a += r[j] * r[j];
    return Math.sqrt((2 * a) / k);
  });
  const peak = interiorPercentile(env, win, 0.9);
  const depth = Math.min(200, peak);
  if (depth < opt.minDepthCents) return null;
  const startIdx = env.findIndex((e) => e >= 0.5 * peak);
  const fullIdx = env.findIndex((e) => e >= 0.9 * peak);
  const startT = raw[Math.max(0, startIdx)].t;
  const vibLenS = n.offset_s - startT;
  let sinSum = 0;
  let cosSum = 0;
  raw.forEach((p, i) => {
    const ph = (2 * Math.PI * (p.t - startT)) / periodS;
    sinSum += r[i] * Math.sin(ph);
    cosSum += r[i] * Math.cos(ph);
  });
  const shift = (((Math.atan2(cosSum, sinSum) / (2 * Math.PI)) * 100) % 100 + 100) % 100;
  const meanC = raw.slice(Math.max(0, startIdx)).reduce((a, p) => a + p.c, 0) / Math.max(1, raw.length - Math.max(0, startIdx));
  const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
  const fadeIn = clamp(((raw[Math.max(startIdx, fullIdx)].t - startT) / vibLenS) * 100, 0, 60);
  return {
    length: clamp(Math.round((vibLenS / dur) * 100), 1, 100),
    period: Math.round(periodS * 1000),
    depth: Math.round(depth),
    in: Math.round(fadeIn),
    out: 15,
    shift: Math.round(shift),
    drift: Math.round(clamp((meanC / depth) * 100, -100, 100)),
  };
}

function fitTailBend(n: RenderNote, contour: ContourFrame[], tuning: number, opt: FitOptions): PitchPoint[] | null {
  const dur = n.offset_s - n.onset_s;
  const s = voicedCentsBetween(contour, n.onset_s + 0.4 * dur, n.offset_s, n.tone, tuning);
  if (s.length < 4) return null;
  const tail = s.slice(-3);
  const endC = tail.reduce((a, p) => a + p.c, 0) / tail.length;
  if (Math.abs(endC) < opt.minTailBendCents) return null;
  let lastSettled = s[0].t;
  for (const p of s) if (Math.abs(p.c) <= opt.settleCents) lastSettled = p.t;
  const fromMs = Math.round((lastSettled - n.onset_s) * 1000);
  const toMs = Math.round(dur * 1000);
  if (toMs - fromMs < 15) return null;
  return [
    { x: fromMs, y: 0, shape: "io" },
    { x: toMs, y: Math.round(Math.max(-120, Math.min(120, endC / 10))), shape: "io" },
  ];
}

/** Fit every note's expression. Notes are matched to the result by index. */
export function fitExpressions(notes: RenderNote[], contour: ContourFrame[], opt: FitOptions = DEFAULT_FIT): Expression[] {
  const order = notes.map((_, i) => i).sort((a, b) => notes[a].onset_s - notes[b].onset_s);
  const tuning = tuningCents(notes, contour);
  const out: Expression[] = new Array(notes.length);
  order.forEach((i, k) => {
    const n = notes[i];
    const prev = k > 0 ? notes[order[k - 1]] : undefined;
    const port = fitPortamento(n, prev, contour, tuning, opt);
    const vibFrom = port ? Math.max(port.settled, n.onset_s) : n.onset_s + 0.15 * (n.offset_s - n.onset_s);
    const next = k + 1 < order.length ? notes[order[k + 1]] : undefined;
    const beforeRest = !next || next.onset_s - n.offset_s > TOUCH_S;
    const tailBend = beforeRest ? fitTailBend(n, contour, tuning, opt) : null;
    const head = port ? port.points : NEUTRAL_POINTS;
    const points = tailBend && head[head.length - 1].x < tailBend[0].x ? [...head, ...tailBend] : head;
    out[i] = {
      points,
      snapFirst: port ? port.snapFirst : true,
      vibrato: fitVibrato(n, vibFrom, contour, tuning, opt),
    };
  });
  return out;
}

/** Mean |rendered − contour| in cents over the voiced frames inside notes. */
export function curveErrorCents(notes: RenderNote[], contour: ContourFrame[], rendered: (times: number[]) => Float64Array): number {
  const tuning = tuningCents(notes, contour);
  const samples = [...notesSamples(notes, contour, tuning)];
  if (!samples.length) return 0;
  const pitch = rendered(samples.map((s) => s.t));
  let err = 0;
  samples.forEach((s, i) => {
    err += Math.abs(pitch[i] * 100 - (s.tone * 100 + s.c));
  });
  return err / samples.length;
}
