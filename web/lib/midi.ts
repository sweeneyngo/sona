// Pure Standard MIDI File (SMF) encoder for the review surface (ticket 10).
// The UI exports the *authoritative* MIDI from the current snapped + corrected
// state (ADR-0004), so this owns the seconds→ticks conversion and byte layout.
// A format-0, single-track file is all a monophonic vocal line needs. No deps —
// hand-rolled — and DOM-free so it's unit-testable under Vitest.

import { clampPitch } from "./math";
import type { Note } from "./annotation";

const NOTE_ON = 0x90;
const NOTE_OFF = 0x80;
const DEFAULT_VELOCITY = 100;

/** A note the encoder needs: pitch + timing in seconds (a slice of {@link Note}). */
export type MidiNote = Pick<Note, "onset_s" | "offset_s" | "midi_pitch">;

interface Event {
  tick: number;
  kind: "on" | "off";
  pitch: number;
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
 * Encode notes (each `{ onset_s, offset_s, midi_pitch }`, seconds) as a
 * format-0 SMF `Uint8Array` at the given `bpm`. `ppq` is the ticks-per-quarter
 * division. Notes are sorted by onset; at each tick note-offs are emitted before
 * note-ons so a note ending where the next begins doesn't overlap.
 */
export function encodeMidiFile(
  notes: MidiNote[],
  { bpm, ppq = 480 }: { bpm: number; ppq?: number },
): Uint8Array {
  const ticksPerSecond = (bpm / 60) * ppq;
  const toTicks = (s: number) => Math.max(0, Math.round(s * ticksPerSecond));

  // Fan each note out to an on/off pair at absolute ticks, then order by tick
  // (offs first on ties) so the delta-time stream is monotonic.
  const events: Event[] = [];
  for (const note of notes) {
    const pitch = clampPitch(note.midi_pitch);
    const on = toTicks(note.onset_s);
    const off = Math.max(on + 1, toTicks(note.offset_s));
    events.push({ tick: on, kind: "on", pitch });
    events.push({ tick: off, kind: "off", pitch });
  }
  events.sort((a, b) => a.tick - b.tick || rank(a.kind) - rank(b.kind));

  const track: number[] = [];
  // Tempo meta: microseconds per quarter note.
  const usPerQuarter = Math.round(60_000_000 / bpm);
  track.push(
    ...writeVarLen(0),
    0xff,
    0x51,
    0x03,
    (usPerQuarter >> 16) & 0xff,
    (usPerQuarter >> 8) & 0xff,
    usPerQuarter & 0xff,
  );

  let prevTick = 0;
  for (const ev of events) {
    track.push(...writeVarLen(ev.tick - prevTick));
    prevTick = ev.tick;
    if (ev.kind === "on") {
      track.push(NOTE_ON, ev.pitch, DEFAULT_VELOCITY);
    } else {
      track.push(NOTE_OFF, ev.pitch, 0);
    }
  }
  // End-of-Track meta.
  track.push(...writeVarLen(0), 0xff, 0x2f, 0x00);

  const header = [
    ...ascii("MThd"),
    ...u32(6),
    ...u16(0), // format 0
    ...u16(1), // one track
    ...u16(ppq),
  ];
  const trackChunk = [...ascii("MTrk"), ...u32(track.length), ...track];
  return Uint8Array.from([...header, ...trackChunk]);
}

// note-off (0) sorts before note-on (1) at the same tick.
function rank(kind: "on" | "off"): number {
  return kind === "off" ? 0 : 1;
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
