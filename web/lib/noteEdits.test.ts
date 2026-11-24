import { describe, expect, it } from "vitest";
import type { WorkingNote } from "./annotation";
import { lcg } from "./exportCases";
import { cellSeconds, type Frame } from "./frame";
import { effectiveNotes } from "./interpret";
import { movedNote, newNote, resizedNote, withoutNote } from "./noteEdits";

const frame: Frame = { bpm: 120, division: 16, anchor: 0.03, barOne: null };
const cell = cellSeconds(frame);
const grabbed = { onset_s: 1, offset_s: 1.4, midi_pitch: 60 };

describe("moving a note", () => {
  it("keeps its length, follows the drag, rounds the pitch to a semitone and marks it manual", () => {
    const m = movedNote(grabbed, 0.25, 2.4);
    expect([m.onset_s, m.midi_pitch, m.manual]).toEqual([1.25, 62, true]);
    expect(m.offset_s).toBeCloseTo(1.65, 12);
  });

  it("never goes before 0 s or outside MIDI pitch, and keeps its length there (property)", () => {
    const rand = lcg(5);
    for (let i = 0; i < 2000; i++) {
      const from = { onset_s: rand() * 4, offset_s: 0, midi_pitch: Math.floor(rand() * 128) };
      from.offset_s = from.onset_s + 0.01 + rand();
      const m = movedNote(from, (rand() - 0.5) * 10, (rand() - 0.5) * 300);
      expect(m.onset_s).toBeGreaterThanOrEqual(0);
      expect(m.offset_s - m.onset_s).toBeCloseTo(from.offset_s - from.onset_s, 9);
      expect(Number.isInteger(m.midi_pitch) && m.midi_pitch >= 0 && m.midi_pitch <= 127).toBe(true);
    }
  });
});

describe("resizing a note", () => {
  it("follows the drag, but never below half a cell, which the snap rounds up to one cell", () => {
    const r = resizedNote(1, grabbed, 0.2, frame);
    expect(r.offset_s).toBeCloseTo(1.6, 12);
    expect(r.manual).toBe(true);
    const shortest = resizedNote(1, grabbed, -5, frame);
    expect(shortest.offset_s).toBeCloseTo(1 + cell / 2, 12);
    const [e] = effectiveNotes([{ ...grabbed, ...shortest, confidence: 1, sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 1, offset_clarity: 1 } }], frame, {
      flagThreshold: 0.5,
      snapKey: false,
      key: null,
    });
    expect(e.offset_s - e.onset_s).toBeCloseTo(cell, 12);
  });
});

describe("adding a note", () => {
  it("is one cell on the grid line nearest the click, at the clicked semitone, manual and fully confident", () => {
    const n = newNote(1.07, 64.4, frame);
    expect(n.onset_s).toBeCloseTo(frame.anchor + 8 * cell, 12);
    expect(n.offset_s - n.onset_s).toBeCloseTo(cell, 12);
    expect([n.midi_pitch, n.manual, n.confidence]).toEqual([64, true, 1]);
    const [e] = effectiveNotes([n], frame, { flagThreshold: 0.99, snapKey: false, key: null });
    expect([e.onset_s, e.offset_s, e.flagged]).toEqual([n.onset_s, n.offset_s, false]);
  });

  it("starts no earlier than 0 s", () => {
    expect(newNote(0.001, 60, { ...frame, anchor: 0.1 }).onset_s).toBe(0);
  });
});

describe("deleting a note", () => {
  it("removes that note only, even when another is identical", () => {
    const a: WorkingNote = { ...grabbed, confidence: 1, sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 1, offset_clarity: 1 } };
    const twin = { ...a };
    const notes = [a, twin];
    const left = withoutNote(notes, a);
    expect(left).toHaveLength(1);
    expect(left[0]).toBe(twin);
    expect(notes).toHaveLength(2);
  });
});
