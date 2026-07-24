// Pure view helpers for the piano-roll: pitch/colour formatting, sub-score
// reasons, and the time↔pixel / pitch↔pixel scale. No canvas or DOM — kept pure
// so the geometry and heatmap are unit-testable under Vitest.

import { clamp01 } from "./math";
import type { SubScores } from "./annotation";

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/** MIDI pitch → scientific pitch name, e.g. 69 → "A4", 61 → "C#4", 0 → "C-1". */
export function midiToNoteName(midi: number): string {
  const name = NOTE_NAMES[((midi % 12) + 12) % 12];
  const octave = Math.floor(midi / 12) - 1;
  return `${name}${octave}`;
}

/**
 * Confidence scalar (0..1) → heatmap colour. Hue sweeps 0° (red, untrustworthy)
 * to 120° (green, solid); this is the per-note colour on the roll.
 */
export function confidenceColor(
  confidence: number,
  { saturation = 70, lightness = 50 }: { saturation?: number; lightness?: number } = {},
): string {
  const c = clamp01(confidence);
  const hue = Math.round(120 * c);
  return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
}

// Human-readable reason per sub-score dimension, phrased as what's *wrong* when
// the score is low — that's what the flag surfaces ("⚠ uncertain offset").
const SUBSCORE_LABELS: Record<keyof SubScores, string> = {
  pitch_stability: "unstable pitch",
  voicing: "possible bleed",
  onset_clarity: "soft onset",
  offset_clarity: "uncertain offset",
};

export interface WeakestSubScore {
  key: string;
  value: number;
  label: string;
}

/**
 * The shakiest dimension of a note — the weakest-link the scalar was soft-min'd
 * from, and the reason to flag it. Returns `null` if no sub-scores are present.
 */
export function weakestSubScore(subScores: Partial<SubScores>): WeakestSubScore | null {
  let weakest: WeakestSubScore | null = null;
  for (const key of Object.keys(SUBSCORE_LABELS) as (keyof SubScores)[]) {
    const value = subScores[key];
    if (value == null) continue;
    if (weakest === null || value < weakest.value) {
      weakest = { key, value, label: SUBSCORE_LABELS[key] };
    }
  }
  return weakest;
}

export interface PitchWindow {
  min: number;
  max: number;
}

/**
 * The MIDI pitch window to draw, padded a few semitones beyond the observed
 * span so notes don't sit flush against the edges. Falls back to a middle
 * octave when there are no notes. Clamped to the MIDI range.
 */
export function pitchRange(
  notes: { midi_pitch: number }[],
  { pad = 3, fallback = { min: 55, max: 79 } }: { pad?: number; fallback?: PitchWindow } = {},
): PitchWindow {
  if (notes.length === 0) return { ...fallback };
  let min = Infinity;
  let max = -Infinity;
  for (const note of notes) {
    if (note.midi_pitch < min) min = note.midi_pitch;
    if (note.midi_pitch > max) max = note.midi_pitch;
  }
  return {
    min: Math.max(0, min - pad),
    max: Math.min(127, max + pad),
  };
}

/** Median of a numeric array (average of the middle pair for even lengths). */
function median(xs: number[]): number {
  const s = [...xs].sort((a, b) => a - b);
  const mid = s.length >> 1;
  return s.length % 2 ? s[mid] : (s[mid - 1] + s[mid]) / 2;
}

/**
 * Flag pitch outliers — notes whose MIDI pitch sits far from the central
 * tendency, e.g. a wrong-octave C1 from separation bleed under a C4 vocal. Uses
 * a robust median ± max(`madK`×MAD, `floor`) band so a tight cluster isn't
 * over-trimmed and a normal ~2-octave span stays intact. Returns one boolean
 * per input pitch, in order.
 */
export function pitchOutlierFlags(
  pitches: number[],
  { madK = 3, floor = 15 }: { madK?: number; floor?: number } = {},
): boolean[] {
  if (pitches.length === 0) return [];
  const med = median(pitches);
  const mad = median(pitches.map((p) => Math.abs(p - med)));
  const spread = Math.max(madK * mad, floor);
  return pitches.map((p) => Math.abs(p - med) > spread);
}

/**
 * Horizontal zoom that makes roughly `targetSeconds` of the timeline fill the
 * view (a sensible zoomed-in default instead of the whole song). Clamped to
 * `[minZoom, maxZoom]`.
 */
export function zoomToShowSeconds(
  totalDuration: number,
  targetSeconds: number,
  maxZoom: number,
  minZoom = 1,
): number {
  if (targetSeconds <= 0 || totalDuration <= 0) return minZoom;
  return Math.min(maxZoom, Math.max(minZoom, totalDuration / targetSeconds));
}

/**
 * The top-of-window MIDI pitch that centres `[minNote, maxNote]` in a `span`-row
 * window (the visible window is `[top - (span-1), top]`). Clamped so the window
 * stays within the MIDI range.
 */
export function defaultPitchTop(minNote: number, maxNote: number, span: number): number {
  const center = Math.round((minNote + maxNote) / 2);
  return Math.min(127, Math.max(span - 1, center + Math.floor(span / 2)));
}

/**
 * The allowed range of `pitchTop` so a `span`-row window can scroll to cover the
 * padded note span `[minNote-pad, maxNote+pad]`. When that span already fits in
 * one window, both bounds pin to the centred default (nothing to scroll).
 */
export function pitchTopRange(
  minNote: number,
  maxNote: number,
  span: number,
  pad = 2,
): { min: number; max: number } {
  const lo = Math.max(0, minNote - pad);
  const hi = Math.min(127, maxNote + pad);
  const min = Math.max(span - 1, lo + (span - 1)); // window bottom at `lo`
  const max = Math.min(127, hi); // window top at `hi`
  if (min > max) {
    const c = defaultPitchTop(minNote, maxNote, span);
    return { min: c, max: c };
  }
  return { min, max };
}

export interface ScaleParams {
  durationS: number;
  minPitch: number;
  maxPitch: number;
  width: number;
  height: number;
  padLeft?: number;
  padTop?: number;
  padBottom?: number;
  /** Horizontal zoom: 1 = fit-to-width; >1 stretches the timeline. */
  zoomX?: number;
  /** Horizontal scroll offset in pixels (how far the timeline is panned left). */
  scrollX?: number;
}

export interface Scale {
  rowHeight: number;
  minPitch: number;
  maxPitch: number;
  timeToX(t: number): number;
  pitchToY(pitch: number): number;
  xToTime(x: number): number;
  yToPitch(y: number): number;
}

/**
 * Build the coordinate transforms for the roll. `timeToX` maps seconds across
 * the plot width (starting at `padLeft`, the key gutter); `pitchToY` maps MIDI
 * pitch down the plot height (higher pitch → smaller y). `xToTime`/`yToPitch`
 * are the inverses, for turning mouse coordinates back into musical positions
 * while editing.
 */
export function makeScale({
  durationS,
  minPitch,
  maxPitch,
  width,
  height,
  padLeft = 0,
  padTop = 0,
  padBottom = 0,
  zoomX = 1,
  scrollX = 0,
}: ScaleParams): Scale {
  const plotWidth = Math.max(1, width - padLeft);
  const plotHeight = Math.max(1, height - padTop - padBottom);
  const safeDuration = durationS > 0 ? durationS : 1;
  // At zoom 1 the whole timeline fits the plot; higher zoom stretches it, and
  // `scrollX` pans that stretched content left under the fixed gutter.
  const contentWidth = plotWidth * zoomX;
  // +1 so the top and bottom pitches each get a full row rather than a zero-
  // height line.
  const pitchSpan = maxPitch - minPitch + 1;
  const rowHeight = plotHeight / pitchSpan;

  return {
    rowHeight,
    minPitch,
    maxPitch,
    timeToX(t) {
      return padLeft + (t / safeDuration) * contentWidth - scrollX;
    },
    pitchToY(pitch) {
      // Row for `pitch` measured from the top; higher pitch = fewer rows down.
      const rowsFromTop = maxPitch - pitch;
      return padTop + rowsFromTop * rowHeight;
    },
    xToTime(x) {
      return ((x - padLeft + scrollX) / contentWidth) * safeDuration;
    },
    yToPitch(y) {
      // Floor, so any y inside a row's band maps to that row's pitch.
      return maxPitch - Math.floor((y - padTop) / rowHeight);
    },
  };
}
