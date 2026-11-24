// Pure Standard MIDI File (SMF) encoder for the review surface. It writes the
// export seam's Sequence (`sequence.ts`), so its ticks are the UST's and the
// USTX's. A format-0, single-track file is all a monophonic vocal line needs.
// No deps — hand-rolled — and DOM-free so it's unit-testable under Vitest.

import { TICKS_PER_QUARTER, type Sequence } from "./sequence";

const NOTE_ON = 0x90;
const NOTE_OFF = 0x80;
const DEFAULT_VELOCITY = 100;

interface Event {
  tick: number;
  rank: number;
  bytes: number[];
}

/**
 * Variable-length quantity: 7 bits per byte, high bit set on all but the last.
 * Returns an array of byte values. This is how SMF encodes delta times.
 */
export function writeVarLen(value: number): number[] {
  let n = Math.max(0, Math.round(value));
  const bytes = [n & 0x7f];
  n = Math.floor(n / 128);
  while (n > 0) {
    bytes.unshift((n & 0x7f) | 0x80);
    n = Math.floor(n / 128);
  }
  return bytes;
}

/**
 * Encode `seq` as a format-0 SMF at `bpm`, `TICKS_PER_QUARTER` ticks per quarter:
 * a tempo, a 4/4 time signature, and per note a lyric (`FF 05`, UTF-8), a note-on
 * and a note-off. At one tick, note-offs come before lyrics and note-ons, so a
 * note ending where the next begins doesn't overlap it.
 */
export function encodeMidiFile(seq: Sequence, { bpm }: { bpm: number }): Uint8Array {
  const utf8 = new TextEncoder();
  const events: Event[] = [];
  for (const n of seq.notes) {
    const pitch = Math.max(0, Math.min(127, n.tone));
    const lyric = [...utf8.encode(n.lyric)];
    events.push({ tick: n.position, rank: 1, bytes: [0xff, 0x05, ...writeVarLen(lyric.length), ...lyric] });
    events.push({ tick: n.position, rank: 2, bytes: [NOTE_ON, pitch, DEFAULT_VELOCITY] });
    events.push({ tick: n.position + n.duration, rank: 0, bytes: [NOTE_OFF, pitch, 0] });
  }
  events.sort((a, b) => a.tick - b.tick || a.rank - b.rank);

  const usPerQuarter = Math.round(60_000_000 / bpm);
  const track: number[] = [
    ...writeVarLen(0),
    0xff,
    0x51,
    0x03,
    (usPerQuarter >> 16) & 0xff,
    (usPerQuarter >> 8) & 0xff,
    usPerQuarter & 0xff,
    ...writeVarLen(0),
    0xff,
    0x58,
    0x04,
    4, // four beats per bar
    2, // of a quarter note (2^2)
    24, // MIDI clocks per metronome click
    8, // 32nd notes per quarter
  ];

  let prevTick = 0;
  for (const ev of events) {
    track.push(...writeVarLen(ev.tick - prevTick), ...ev.bytes);
    prevTick = ev.tick;
  }
  track.push(...writeVarLen(0), 0xff, 0x2f, 0x00);

  const header = [
    ...ascii("MThd"),
    ...u32(6),
    ...u16(0), // format 0
    ...u16(1), // one track
    ...u16(TICKS_PER_QUARTER),
  ];
  const trackChunk = [...ascii("MTrk"), ...u32(track.length), ...track];
  return Uint8Array.from([...header, ...trackChunk]);
}

function ascii(s: string): number[] {
  return [...s].map((c) => c.charCodeAt(0));
}

function u16(n: number): number[] {
  return [(n >> 8) & 0xff, n & 0xff];
}

function u32(n: number): number[] {
  return [(n >> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}
