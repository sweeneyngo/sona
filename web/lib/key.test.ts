import { test } from "vitest";
import assert from "node:assert/strict";

import { detectKey, keyName, isInKey, snapToKey, scaleOf } from "./key";

// Build notes from (midi, durationSeconds) pairs laid end to end.
function notes(spec: [number, number][]): { midi_pitch: number; onset_s: number; offset_s: number }[] {
  let t = 0;
  return spec.map(([midi_pitch, dur]) => {
    const onset_s = t;
    t += dur;
    return { midi_pitch, onset_s, offset_s: onset_s - 0 + dur };
  });
}

test("detectKey finds C major from a C-major melody", () => {
  // A tonic-heavy C major line: C E G C D E F G A B C (all naturals, C emphasised).
  const k = detectKey(notes([
    [60, 1], [64, 0.5], [67, 0.5], [72, 1], [62, 0.5], [64, 0.5],
    [65, 0.5], [67, 0.5], [69, 0.5], [71, 0.5], [60, 1],
  ]))!;
  assert.equal(keyName(k), "C major");
});

test("detectKey finds A minor from an A-minor melody", () => {
  // A B C D E F G A, tonic-weighted — A natural minor (relative of C major, but
  // the A/E emphasis should pick minor over its relative major).
  const k = detectKey(notes([
    [57, 1.5], [59, 0.5], [60, 0.5], [62, 0.5], [64, 1.5], [65, 0.5],
    [67, 0.5], [57, 1.5], [64, 1],
  ]))!;
  assert.equal(k.tonic, 9);
  assert.equal(k.mode, "minor");
});

test("detectKey is null for no notes", () => {
  assert.equal(detectKey([]), null);
});

test("scaleOf gives the diatonic pitch classes", () => {
  assert.deepEqual(scaleOf(0, "major"), [0, 2, 4, 5, 7, 9, 11]); // C major
  assert.deepEqual(scaleOf(9, "minor"), [0, 2, 4, 5, 7, 9, 11]); // A minor: same set
  assert.deepEqual(scaleOf(7, "major"), [2, 4, 6, 7, 9, 11, 0].sort((a, b) => a - b)); // G major (F#)
});

test("isInKey distinguishes scale tones from accidentals", () => {
  const cmaj = detectKey(notes([[60, 1], [64, 1], [67, 1]]))!;
  assert.equal(isInKey(65, cmaj), true); // F is in C major
  assert.equal(isInKey(66, cmaj), false); // F# is not
  assert.equal(isInKey(72, cmaj), true); // C an octave up
});

test("snapToKey moves an accidental to the nearest scale tone", () => {
  const cmaj = { tonic: 0, mode: "major" as const, correlation: 1, scale: scaleOf(0, "major") };
  // C#4 (61): both C(60) and D(62) are one semitone away — tie goes to the lower.
  assert.equal(snapToKey(61, cmaj), 60);
  // D#4 (63): E(64) is one up, D(62) is one down — tie → lower (D).
  assert.equal(snapToKey(63, cmaj), 62);
  // F#4 (66): G(67) up vs F(65) down — tie → lower (F).
  assert.equal(snapToKey(66, cmaj), 65);
  // An in-key pitch is unchanged.
  assert.equal(snapToKey(67, cmaj), 67);
});

test("snapToKey in a sharp key: accidentals are equidistant, tie goes low", () => {
  const gmaj = { tonic: 7, mode: "major" as const, correlation: 1, scale: scaleOf(7, "major") };
  // Every accidental sits in a whole-step gap, so it is one semitone from *both*
  // neighbours — the tie always breaks to the lower. F natural (65) → E (64),
  // not F#(66), even though F# is the key's characteristic sharp.
  assert.equal(snapToKey(65, gmaj), 64);
  // F#(66) is in key and unchanged.
  assert.equal(snapToKey(66, gmaj), 66);
  // C natural (60) is in G major, unchanged.
  assert.equal(snapToKey(60, gmaj), 60);
});
