// Pure view helpers for the piano-roll: pitch/colour formatting, sub-score
// reasons, and the time↔pixel / pitch↔pixel scale. No canvas or DOM — kept pure
// so the geometry and heatmap are unit-testable under `node --test`.

import { clamp01 } from "./math.js";

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/** MIDI pitch → scientific pitch name, e.g. 69 → "A4", 61 → "C#4", 0 → "C-1". */
export function midiToNoteName(midi) {
  const name = NOTE_NAMES[((midi % 12) + 12) % 12];
  const octave = Math.floor(midi / 12) - 1;
  return `${name}${octave}`;
}

/**
 * Confidence scalar (0..1) → heatmap colour. Hue sweeps 0° (red, untrustworthy)
 * to 120° (green, solid); this is the per-note colour on the roll.
 */
export function confidenceColor(confidence, { saturation = 70, lightness = 50 } = {}) {
  const c = clamp01(confidence);
  const hue = Math.round(120 * c);
  return `hsl(${hue}, ${saturation}%, ${lightness}%)`;
}

// Human-readable reason per sub-score dimension, phrased as what's *wrong* when
// the score is low — that's what the flag surfaces ("⚠ uncertain offset").
const SUBSCORE_LABELS = {
  pitch_stability: "unstable pitch",
  voicing: "possible bleed",
  onset_clarity: "soft onset",
  offset_clarity: "uncertain offset",
};

/**
 * The shakiest dimension of a note — the weakest-link the scalar was soft-min'd
 * from, and the reason to flag it. Returns `{ key, value, label }`.
 */
export function weakestSubScore(subScores) {
  let weakest = null;
  for (const key of Object.keys(SUBSCORE_LABELS)) {
    const value = subScores[key];
    if (value == null) continue;
    if (weakest === null || value < weakest.value) {
      weakest = { key, value, label: SUBSCORE_LABELS[key] };
    }
  }
  return weakest;
}

/**
 * The MIDI pitch window to draw, padded a few semitones beyond the observed
 * span so notes don't sit flush against the edges. Falls back to a middle
 * octave when there are no notes. Clamped to the MIDI range.
 */
export function pitchRange(notes, { pad = 3, fallback = { min: 55, max: 79 } } = {}) {
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

/**
 * Build the coordinate transforms for the roll. `timeToX` maps seconds across
 * the plot width (starting at `padLeft`, the key gutter); `pitchToY` maps MIDI
 * pitch down the plot height (higher pitch → smaller y). Returns the transforms
 * plus the row height, so callers can draw note bars.
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
}) {
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
    // Inverses, for turning mouse coordinates back into musical positions while
    // editing (ticket 10). `yToPitch` rounds to the nearest semitone row.
    xToTime(x) {
      return ((x - padLeft) / plotWidth) * safeDuration;
    },
    yToPitch(y) {
      // Floor, so any y inside a row's band maps to that row's pitch.
      return maxPitch - Math.floor((y - padTop) / rowHeight);
    },
  };
}
