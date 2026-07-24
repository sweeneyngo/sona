import { test } from "vitest";
import assert from "node:assert/strict";

import { writeVarLen, encodeMidiFile } from "./midi";

const ascii = (bytes: Uint8Array, start: number, len: number) =>
  String.fromCharCode(...bytes.slice(start, start + len));

test("writeVarLen encodes small values as a single byte", () => {
  assert.deepEqual(writeVarLen(0), [0x00]);
  assert.deepEqual(writeVarLen(127), [0x7f]);
});

test("writeVarLen encodes the canonical multi-byte examples (SMF spec)", () => {
  // From the Standard MIDI File spec's variable-length quantity table.
  assert.deepEqual(writeVarLen(128), [0x81, 0x00]);
  assert.deepEqual(writeVarLen(0x100000), [0xc0, 0x80, 0x00]);
  assert.deepEqual(writeVarLen(0x0fffffff), [0xff, 0xff, 0xff, 0x7f]);
});

test("encodeMidiFile writes a valid MThd header (format 0, 1 track)", () => {
  const bytes = encodeMidiFile([], { bpm: 120, ppq: 480 });
  assert.equal(ascii(bytes, 0, 4), "MThd");
  // Header length = 6.
  assert.deepEqual([...bytes.slice(4, 8)], [0, 0, 0, 6]);
  // format 0, ntrks 1, division 480.
  assert.deepEqual([...bytes.slice(8, 10)], [0, 0]);
  assert.deepEqual([...bytes.slice(10, 12)], [0, 1]);
  assert.deepEqual([...bytes.slice(12, 14)], [0x01, 0xe0]); // 480
});

test("encodeMidiFile emits a tempo meta matching the BPM", () => {
  const bytes = encodeMidiFile([], { bpm: 120 });
  // 500000 µs per quarter at 120 BPM → FF 51 03 07 A1 20.
  const seq = [0xff, 0x51, 0x03, 0x07, 0xa1, 0x20];
  assert.ok(containsSequence(bytes, seq), "tempo meta present");
});

test("encodeMidiFile emits one note-on and one note-off per note", () => {
  const notes = [
    { onset_s: 0, offset_s: 0.5, midi_pitch: 60 },
    { onset_s: 0.5, offset_s: 1.0, midi_pitch: 64 },
  ];
  const bytes = encodeMidiFile(notes, { bpm: 120, ppq: 480 });
  // Note-on is 0x90 with non-zero velocity; note-off we emit as 0x80.
  let ons = 0;
  let offs = 0;
  for (let i = 0; i < bytes.length; i++) {
    if (bytes[i] === 0x90) ons++;
    if (bytes[i] === 0x80) offs++;
  }
  assert.equal(ons, 2);
  assert.equal(offs, 2);
});

test("encodeMidiFile places note boundaries at the right tick (delta VLQ)", () => {
  // A single note at 120 BPM with ppq=480: onset at tick 0, offset at tick 240
  // (0.25 s → an eighth). 240 needs two VLQ bytes (0x81 0x70).
  const bytes = encodeMidiFile([{ onset_s: 0, offset_s: 0.25, midi_pitch: 69 }], {
    bpm: 120,
    ppq: 480,
  });
  // note-on: delta 0x00, 0x90, pitch 69 (0x45), velocity.
  assert.ok(containsSequence(bytes, [0x00, 0x90, 0x45]), "note-on at tick 0");
  // note-off after a delta of 240 → VLQ 0x81 0x70, then 0x80, pitch 69.
  assert.ok(containsSequence(bytes, [0x81, 0x70, 0x80, 0x45]), "note-off at +240");
});

test("encodeMidiFile ends the track with an End-of-Track meta", () => {
  const bytes = encodeMidiFile([], { bpm: 120 });
  const eot = [0xff, 0x2f, 0x00];
  assert.ok(containsSequence(bytes, eot));
  // ...and it is the final event.
  assert.deepEqual([...bytes.slice(-3)], eot);
});

test("encodeMidiFile sorts unordered notes by onset", () => {
  const bytes = encodeMidiFile(
    [
      { onset_s: 1.0, offset_s: 1.5, midi_pitch: 72 },
      { onset_s: 0.0, offset_s: 0.5, midi_pitch: 60 },
    ],
    { bpm: 120, ppq: 480 },
  );
  // The first note-on in the stream must be the earlier note (pitch 60).
  const on60 = indexOfSequence(bytes, [0x90, 0x3c]);
  const on72 = indexOfSequence(bytes, [0x90, 0x48]);
  assert.ok(on60 > 0 && on72 > 0 && on60 < on72);
});

function containsSequence(bytes: Uint8Array, seq: number[]) {
  return indexOfSequence(bytes, seq) !== -1;
}

function indexOfSequence(bytes: Uint8Array, seq: number[]) {
  outer: for (let i = 0; i + seq.length <= bytes.length; i++) {
    for (let j = 0; j < seq.length; j++) {
      if (bytes[i + j] !== seq[j]) continue outer;
    }
    return i;
  }
  return -1;
}
