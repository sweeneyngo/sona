import { test } from "vitest";
import assert from "node:assert/strict";

import { midiToFreq, buildSchedule } from "./synth";

test("midiToFreq anchors A4 (69) to 440 Hz", () => {
  assert.equal(midiToFreq(69), 440);
});

test("midiToFreq is one octave per 12 semitones", () => {
  assert.ok(Math.abs(midiToFreq(81) - 880) < 1e-9);
  assert.ok(Math.abs(midiToFreq(57) - 220) < 1e-9);
});

test("midiToFreq handles an equal-tempered semitone", () => {
  // A#4 (70) is 440 * 2^(1/12).
  assert.ok(Math.abs(midiToFreq(70) - 440 * Math.pow(2, 1 / 12)) < 1e-9);
});

test("buildSchedule maps each note to a voice with freq/start/duration", () => {
  const notes = [
    { onset_s: 0.1, offset_s: 0.3, midi_pitch: 69, confidence: 1 },
    { onset_s: 0.5, offset_s: 0.6, midi_pitch: 81, confidence: 0.5 },
  ];
  const voices = buildSchedule(notes);
  assert.equal(voices.length, 2);
  assert.equal(voices[0].freq, 440);
  assert.ok(Math.abs(voices[0].start - 0.1) < 1e-9);
  assert.ok(Math.abs(voices[0].duration - 0.2) < 1e-9);
  assert.ok(Math.abs(voices[1].start - 0.5) < 1e-9);
});

test("buildSchedule plays every note at the same reference gain", () => {
  // Confidence drives the roll heatmap, not playback — each note is heard
  // equally so it can be judged against the stem.
  const notes = [
    { onset_s: 0, offset_s: 1, midi_pitch: 60, confidence: 1 },
    { onset_s: 1, offset_s: 2, midi_pitch: 60, confidence: 0 },
  ];
  const [a, b] = buildSchedule(notes);
  assert.equal(a.gain, b.gain);
  assert.ok(a.gain > 0);
});

test("buildSchedule clamps a zero/negative-length note to a positive duration", () => {
  const [voice] = buildSchedule([
    { onset_s: 0.4, offset_s: 0.4, midi_pitch: 60, confidence: 1 },
  ]);
  assert.ok(voice.duration > 0);
});
