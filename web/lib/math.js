// Tiny shared numeric helpers for the review surface. Its own module so the
// view, synth, and DOM layers import one definition rather than each keeping a
// copy (these are ES modules loaded directly in the browser — no bundler).

/** Clamp to the unit interval; treats NaN as 0. */
export function clamp01(x) {
  if (!(x > 0)) return 0;
  if (x > 1) return 1;
  return x;
}
