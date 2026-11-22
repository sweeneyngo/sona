import type { F0Frame } from "./annotation";

/** `[time_s, frequency_hz, voicing]`, rounded to what the roll and the exports can tell apart. */
export type PackedFrame = [number, number, number];

export function packContour(contour: F0Frame[]): PackedFrame[] {
  return contour.map((f) => [Math.round(f.time_s * 1e4) / 1e4, Math.round(f.frequency_hz * 100) / 100, Math.round(f.voicing * 1000) / 1000]);
}

/** Frames in either form: the annotation's objects, or packed triples from a lite autosave. */
export function unpackContour(raw: unknown): F0Frame[] {
  if (!Array.isArray(raw)) return [];
  return raw.map((f) => (Array.isArray(f) ? { time_s: Number(f[0]), frequency_hz: Number(f[1]), voicing: Number(f[2]) } : (f as F0Frame)));
}
