import { test } from "vitest";
import assert from "node:assert/strict";

import {
  midiToNoteName,
  confidenceColor,
  weakestSubScore,
  pitchRange,
  makeScale,
  zoomToShowSeconds,
  defaultPitchTop,
  pitchTopRange,
  pitchOutlierFlags,
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

const baseScale = {
  durationS: 2,
  minPitch: 60,
  maxPitch: 72,
  width: 800,
  height: 400,
  padLeft: 40,
  padTop: 10,
  padBottom: 20,
};

test("zoomX stretches the timeline about the left gutter", () => {
  const s = makeScale({ ...baseScale, zoomX: 2 });
  // t=0 still pinned to the gutter; the plot (760px) now spans 2× that.
  assert.ok(Math.abs(s.timeToX(0) - 40) < 1e-9);
  assert.ok(Math.abs(s.timeToX(2) - (40 + 2 * 760)) < 1e-9);
});

test("scrollX shifts the timeline left by that many pixels", () => {
  const s = makeScale({ ...baseScale, zoomX: 2, scrollX: 100 });
  assert.ok(Math.abs(s.timeToX(0) - (40 - 100)) < 1e-9);
  // A point that was at x under no scroll moves left by exactly scrollX.
  const noScroll = makeScale({ ...baseScale, zoomX: 2 });
  assert.ok(Math.abs(s.timeToX(1) - (noScroll.timeToX(1) - 100)) < 1e-9);
});

test("xToTime inverts timeToX under zoom + scroll", () => {
  const s = makeScale({ ...baseScale, zoomX: 3.5, scrollX: 220 });
  for (const t of [0, 0.4, 1.0, 1.9]) {
    assert.ok(Math.abs(s.xToTime(s.timeToX(t)) - t) < 1e-9);
  }
});

test("zoomToShowSeconds picks the zoom that fits ~target seconds in view", () => {
  assert.equal(zoomToShowSeconds(240, 10, 60), 24); // 240s / 10s visible
  assert.equal(zoomToShowSeconds(5, 10, 60), 1); // short song → can't zoom out past 1
  assert.equal(zoomToShowSeconds(6000, 10, 60), 60); // clamped to max zoom
});

test("defaultPitchTop centres the note span in a 2-octave window", () => {
  // span 24 rows; centre of 60..64 is 62, window top = 62 + 12 = 74.
  assert.equal(defaultPitchTop(60, 64, 24), 74);
  // window [74-23, 74] = [51, 74] contains both notes.
  assert.ok(74 - 23 <= 60 && 74 >= 64);
  assert.ok(defaultPitchTop(0, 0, 24) >= 23); // never below the span floor
});

test("pitchTopRange lets a 2-octave window scroll over a wide span", () => {
  const r = pitchTopRange(50, 90, 24, 2); // padded to 48..92, span 41 > 24
  assert.equal(r.min, 71); // window bottom at 48 → top 48+23
  assert.equal(r.max, 92); // window top at 92
  assert.ok(r.min < r.max);
});

test("pitchTopRange pins a window when the span already fits", () => {
  const r = pitchTopRange(60, 64, 24); // 5-semitone span < 24 → fixed
  assert.equal(r.min, r.max);
  assert.equal(r.min, defaultPitchTop(60, 64, 24));
});

test("pitchOutlierFlags marks a wrong-octave note (C1 under a C4 vocal)", () => {
  const flags = pitchOutlierFlags([60, 62, 64, 63, 61, 24]);
  assert.deepEqual(flags, [false, false, false, false, false, true]);
});

test("pitchOutlierFlags keeps a normal ~2-octave vocal span intact", () => {
  const pitches = [55, 57, 60, 62, 64, 67, 69, 72];
  assert.deepEqual(
    pitchOutlierFlags(pitches),
    pitches.map(() => false),
  );
});

test("pitchOutlierFlags is empty for no notes", () => {
  assert.deepEqual(pitchOutlierFlags([]), []);
});

test("zoomX defaults to 1 / scrollX to 0 (fit-to-width, unchanged)", () => {
  const zoomed = makeScale({ ...baseScale });
  const plain = makeScale({ ...baseScale, zoomX: 1, scrollX: 0 });
  assert.equal(zoomed.timeToX(2), plain.timeToX(2));
  assert.ok(Math.abs(zoomed.timeToX(2) - 800) < 1e-9);
});
