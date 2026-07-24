import { test } from "vitest";
import assert from "node:assert/strict";

import {
  midiToNoteName,
  confidenceColor,
  weakestSubScore,
  pitchRange,
  makeScale,
} from "./view";

test("midiToNoteName names the anchor and octave boundaries", () => {
  assert.equal(midiToNoteName(69), "A4");
  assert.equal(midiToNoteName(60), "C4");
  assert.equal(midiToNoteName(61), "C#4");
  assert.equal(midiToNoteName(0), "C-1");
});

test("confidenceColor walks red -> green as confidence rises", () => {
  const low = confidenceColor(0);
  const high = confidenceColor(1);
  assert.notEqual(low, high);
  // Hue encodes the heatmap: 0deg (red) at the bottom, 120deg (green) at the top.
  assert.match(low, /hsl\(\s*0/);
  assert.match(high, /hsl\(\s*120/);
});

test("confidenceColor clamps out-of-range inputs", () => {
  assert.equal(confidenceColor(-5), confidenceColor(0));
  assert.equal(confidenceColor(5), confidenceColor(1));
});

test("weakestSubScore picks the lowest dimension and labels it", () => {
  const sub = {
    pitch_stability: 0.9,
    voicing: 0.8,
    onset_clarity: 0.95,
    offset_clarity: 0.4,
  };
  const weakest = weakestSubScore(sub);
  assert.ok(weakest);
  assert.equal(weakest.key, "offset_clarity");
  assert.equal(weakest.value, 0.4);
  assert.match(weakest.label, /offset/i);
});

test("pitchRange pads the observed span and stays within MIDI bounds", () => {
  const { min, max } = pitchRange([
    { midi_pitch: 60 },
    { midi_pitch: 64 },
  ]);
  assert.ok(min < 60, "pads below the lowest note");
  assert.ok(max > 64, "pads above the highest note");
  assert.ok(min >= 0 && max <= 127);
});

test("pitchRange falls back to a sensible window when there are no notes", () => {
  const { min, max } = pitchRange([]);
  assert.ok(max > min);
  assert.ok(min >= 0 && max <= 127);
});

test("makeScale maps time to x and pitch to y within the plot rect", () => {
  const scale = makeScale({
    durationS: 2,
    minPitch: 60,
    maxPitch: 72,
    width: 800,
    height: 400,
    padLeft: 40,
    padTop: 10,
    padBottom: 20,
  });
  // t=0 sits at the left gutter; t=duration at the right edge.
  assert.ok(Math.abs(scale.timeToX(0) - 40) < 1e-9);
  assert.ok(Math.abs(scale.timeToX(2) - 800) < 1e-9);
  // Higher pitch is higher on screen (smaller y).
  assert.ok(scale.pitchToY(72) < scale.pitchToY(60));
  // The plot area sits inside the vertical padding.
  assert.ok(scale.pitchToY(72) >= 10);
  assert.ok(scale.pitchToY(60) <= 400 - 20);
});

test("makeScale inverses round-trip time and pitch", () => {
  const scale = makeScale({
    durationS: 2,
    minPitch: 60,
    maxPitch: 72,
    width: 800,
    height: 400,
    padLeft: 40,
    padTop: 10,
    padBottom: 20,
  });
  assert.ok(Math.abs(scale.xToTime(scale.timeToX(1.3)) - 1.3) < 1e-9);
  // pitchToY targets a row's top; sampling the row centre recovers the pitch.
  const y = scale.pitchToY(65) + scale.rowHeight / 2;
  assert.equal(scale.yToPitch(y), 65);
});
