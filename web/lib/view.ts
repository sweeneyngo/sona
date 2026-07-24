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

export interface ScaleParams {
  durationS: number;
  minPitch: number;
  maxPitch: number;
  width: number;
  height: number;
  padLeft?: number;
  padTop?: number;
  padBottom?: number;
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
}: ScaleParams): Scale {
  const plotWidth = Math.max(1, width - padLeft);
  const plotHeight = Math.max(1, height - padTop - padBottom);
  const safeDuration = durationS > 0 ? durationS : 1;
  // +1 so the top and bottom pitches each get a full row rather than a zero-
  // height line.
  const pitchSpan = maxPitch - minPitch + 1;
  const rowHeight = plotHeight / pitchSpan;

  return {
    rowHeight,
    minPitch,
    maxPitch,
    timeToX(t) {
      return padLeft + (t / safeDuration) * plotWidth;
    },
    pitchToY(pitch) {
      // Row for `pitch` measured from the top; higher pitch = fewer rows down.
      const rowsFromTop = maxPitch - pitch;
      return padTop + rowsFromTop * rowHeight;
    },
    xToTime(x) {
      return ((x - padLeft) / plotWidth) * safeDuration;
    },
    yToPitch(y) {
      // Floor, so any y inside a row's band maps to that row's pitch.
      return maxPitch - Math.floor((y - padTop) / rowHeight);
    },
  };
}
