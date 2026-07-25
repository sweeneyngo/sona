// Musical-key detection + scale snapping for the review surface. Like the grid
// (grid.ts), this is *interpretation*, not perception (ADR-0004): the core emits
// raw note pitches; the UI infers a key from them and uses it only as a review
// aid — to flag out-of-key notes and, optionally, snap corrections to the scale.
// The key is never fed back into the core's pitch detection, so a genuine
// out-of-key note (a blue note, a bend, or a real error) stays visible instead of
// being silently "corrected" away from the confidence flag that should catch it.
// Kept DOM-free so the algorithm is unit-testable.

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

// Krumhansl–Kessler key profiles: the perceived stability of each scale degree,
// relative to the tonic, for a major and a (natural) minor key. Detection
// correlates a piece's pitch-class histogram against all 24 rotations of these.
const MAJOR_PROFILE = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88];
const MINOR_PROFILE = [6.33, 2.68, 3.52, 5.38, 2.6, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17];

// Scale degrees (semitones from the tonic) for the diatonic major and natural
// minor scales — used for in-key membership and scale snapping.
const MAJOR_SCALE = [0, 2, 4, 5, 7, 9, 11];
const MINOR_SCALE = [0, 2, 3, 5, 7, 8, 10];

export type Mode = "major" | "minor";

export interface KeyEstimate {
  /** Tonic pitch class, 0–11 (0 = C). */
  tonic: number;
  mode: Mode;
  /** Best profile correlation in −1..1 — how strongly the notes fit this key. */
  correlation: number;
  /** The key's pitch classes (0–11), ascending — its diatonic scale. */
  scale: number[];
}

/** The bits of a note key detection reads: pitch and (for weighting) duration. */
export interface KeyableNote {
  midi_pitch: number;
  onset_s: number;
  offset_s: number;
}

/** Positive pitch class (0–11) of a MIDI pitch. */
function pitchClass(midi: number): number {
  return ((Math.round(midi) % 12) + 12) % 12;
}

/** Pearson correlation of two equal-length vectors (0 when either is constant). */
function pearson(a: number[], b: number[]): number {
  const n = a.length;
  const ma = a.reduce((s, x) => s + x, 0) / n;
  const mb = b.reduce((s, x) => s + x, 0) / n;
  let num = 0;
  let da = 0;
  let db = 0;
  for (let i = 0; i < n; i++) {
    const xa = a[i] - ma;
    const xb = b[i] - mb;
    num += xa * xb;
    da += xa * xa;
    db += xb * xb;
  }
  const den = Math.sqrt(da * db);
  return den === 0 ? 0 : num / den;
}

/**
 * Detect the most likely key of `notes` by Krumhansl–Schmuckler: a pitch-class
 * histogram weighted by note duration (longer notes anchor the key more),
 * correlated against all 12 major and 12 minor key profiles; the best fit wins.
 * Returns `null` for no notes. Ties break toward the earlier (lower-tonic, then
 * major) candidate, which only matters for degenerate inputs.
 */
export function detectKey(notes: KeyableNote[]): KeyEstimate | null {
  if (notes.length === 0) return null;

  const hist = new Array(12).fill(0);
  for (const n of notes) {
    // Weight by duration, with a floor so a zero-length note still counts.
    hist[pitchClass(n.midi_pitch)] += Math.max(n.offset_s - n.onset_s, 1e-3);
  }

  let best: KeyEstimate | null = null;
  for (let tonic = 0; tonic < 12; tonic++) {
    for (const mode of ["major", "minor"] as const) {
      const profile = mode === "major" ? MAJOR_PROFILE : MINOR_PROFILE;
      // Profile aligned to absolute pitch class: index p gets the weight of scale
      // degree (p − tonic).
      const aligned = hist.map((_, p) => profile[((p - tonic) % 12 + 12) % 12]);
      const correlation = pearson(hist, aligned);
      if (!best || correlation > best.correlation) {
        best = { tonic, mode, correlation, scale: scaleOf(tonic, mode) };
      }
    }
  }
  return best;
}

/** The diatonic pitch classes (ascending) of the key with this `tonic`/`mode`. */
export function scaleOf(tonic: number, mode: Mode): number[] {
  const degrees = mode === "major" ? MAJOR_SCALE : MINOR_SCALE;
  return degrees.map((d) => (d + tonic) % 12).sort((a, b) => a - b);
}

/** Human name of a key, e.g. `{tonic: 9, mode: "minor"}` → "A minor". */
export function keyName(key: KeyEstimate): string {
  return `${NOTE_NAMES[key.tonic]} ${key.mode}`;
}

/** Whether a MIDI pitch's pitch class belongs to the key's scale. */
export function isInKey(midi: number, key: KeyEstimate): boolean {
  return key.scale.includes(pitchClass(midi));
}

/**
 * Snap a MIDI pitch to the nearest pitch in the key's scale (semitone distance),
 * preferring the lower neighbour on a tie. An in-key pitch is returned unchanged.
 */
export function snapToKey(midi: number, key: KeyEstimate): number {
  const rounded = Math.round(midi);
  if (isInKey(rounded, key)) return rounded;
  for (let d = 1; d <= 6; d++) {
    if (key.scale.includes(pitchClass(rounded - d))) return rounded - d;
    if (key.scale.includes(pitchClass(rounded + d))) return rounded + d;
  }
  return rounded; // unreachable for a 7-note scale, but keep it total
}
