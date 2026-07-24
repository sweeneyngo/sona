// Tiny shared numeric helpers for the review surface. Its own module so the
// view, synth, and DOM layers import one definition rather than each keeping a
// copy.

/** Clamp to the unit interval; treats NaN as 0. */
export function clamp01(x: number): number {
  if (!(x > 0)) return 0;
  if (x > 1) return 1;
  return x;
}

/** Round to the nearest integer MIDI pitch and clamp to the valid 0–127 range. */
export function clampPitch(p: number): number {
  const n = Math.round(p);
  if (n < 0) return 0;
  if (n > 127) return 127;
  return n;
}
