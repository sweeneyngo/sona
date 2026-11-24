import { test } from "vitest";
import assert from "node:assert/strict";

import { writeVarLen, encodeMidiFile } from "./midi";
import { readMidi } from "./exportCases";
import type { Sequence } from "./sequence";

const ascii = (bytes: Uint8Array, start: number, len: number) =>
  String.fromCharCode(...bytes.slice(start, start + len));

const seq = (notes: [number, number, number, string?][]): Sequence => ({
  notes: notes.map(([position, duration, tone, lyric]) => ({ position, duration, tone, lyric: lyric ?? "あ" })),
  shiftS: 0,
});

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

test("encodeMidiFile writes a valid MThd header (format 0, 1 track, 480 ppq)", () => {
  const bytes = encodeMidiFile(seq([]), { bpm: 120 });
  assert.equal(ascii(bytes, 0, 4), "MThd");
  assert.deepEqual([...bytes.slice(4, 8)], [0, 0, 0, 6]);
  assert.deepEqual([...bytes.slice(8, 10)], [0, 0]);
  assert.deepEqual([...bytes.slice(10, 12)], [0, 1]);
  assert.deepEqual([...bytes.slice(12, 14)], [0x01, 0xe0]);
});

test("encodeMidiFile emits a tempo matching the BPM and a 4/4 time signature", () => {
  const read = readMidi(encodeMidiFile(seq([]), { bpm: 120 }));
  assert.equal(read.usPerQuarter, 500000);
  assert.deepEqual(read.timeSignature, [4, 2, 24, 8]);
});

test("encodeMidiFile places every note at its sequence tick, with its lyric", () => {
  const read = readMidi(
    encodeMidiFile(
      seq([
        [960, 240, 64, "さ"],
        [1200, 240, 66, "くら"],
        [1680, 200, 67, "+~"],
      ]),
      { bpm: 133 },
    ),
  );
  assert.deepEqual(read.notes, [
    [960, 240, 64, "さ"],
    [1200, 240, 66, "くら"],
    [1680, 200, 67, "+~"],
  ]);
});

test("encodeMidiFile ends a note before the next one starts on the same tick", () => {
  const bytes = encodeMidiFile(seq([[0, 240, 60], [240, 240, 60]]), { bpm: 120 });
  const off = indexOfSequence(bytes, [0x80, 60, 0]);
  const secondOn = bytes.findIndex((b, i) => b === 0x90 && i > indexOfSequence(bytes, [0x90, 60]) + 1);
  assert.ok(off > 0 && secondOn > off);
  assert.deepEqual(readMidi(bytes).notes.map((n) => n.slice(0, 2)), [[0, 240], [240, 240]]);
});

test("encodeMidiFile ends the track with an End-of-Track meta", () => {
  const bytes = encodeMidiFile(seq([]), { bpm: 120 });
  assert.deepEqual([...bytes.slice(-3)], [0xff, 0x2f, 0x00]);
});

function indexOfSequence(bytes: Uint8Array, seq: number[]) {
  outer: for (let i = 0; i + seq.length <= bytes.length; i++) {
    for (let j = 0; j < seq.length; j++) {
      if (bytes[i + j] !== seq[j]) continue outer;
    }
    return i;
  }
  return -1;
}
