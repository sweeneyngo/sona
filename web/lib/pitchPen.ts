import { unvoicedAt, type F0Frame } from "./sungPitch";

const centsToHz = (c: number) => 440 * 2 ** ((c - 6900) / 1200);

function firstAtOrAfter(contour: F0Frame[], t: number): number {
  let lo = 0;
  let hi = contour.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (contour[mid].time_s < t) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Frame indices from `a` to `b` seconds; a click between two frames takes the nearer one. */
function framesBetween(contour: F0Frame[], a: number, b: number): [number, number] {
  const lo = firstAtOrAfter(contour, a);
  let hi = lo;
  while (hi < contour.length && contour[hi].time_s <= b) hi++;
  if (hi > lo || !contour.length) return [lo, hi];
  const nearest = lo > 0 && (lo === contour.length || a - contour[lo - 1].time_s < contour[lo].time_s - a) ? lo - 1 : lo;
  return [nearest, nearest + 1];
}

/** Draw a straight segment in cents; returns how many frames changed. */
export function penStroke(contour: F0Frame[], t0: number, c0: number, t1: number, c1: number): number {
  const [a, b] = t0 <= t1 ? [[t0, c0], [t1, c1]] : [[t1, c1], [t0, c0]];
  const [lo, hi] = framesBetween(contour, a[0], b[0]);
  for (let i = lo; i < hi; i++) {
    const w = b[0] > a[0] ? Math.max(0, Math.min(1, (contour[i].time_s - a[0]) / (b[0] - a[0]))) : 0;
    contour[i] = { time_s: contour[i].time_s, frequency_hz: centsToHz(a[1] + (b[1] - a[1]) * w), voicing: 1 };
  }
  return hi - lo;
}

/** Mark frames unvoiced so the notes sing there; returns how many frames changed. */
export function penErase(contour: F0Frame[], t0: number, t1: number): number {
  const [lo, hi] = framesBetween(contour, Math.min(t0, t1), Math.max(t0, t1));
  for (let i = lo; i < hi; i++) contour[i] = unvoicedAt(contour[i].time_s);
  return hi - lo;
}
