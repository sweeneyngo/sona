import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { WorkingNote } from "./annotation";
import { NEUTRAL_EXPRESSION } from "./expression";
import { lcg } from "./exportCases";
import { DIVISIONS } from "./grid";
import { effectiveNotes, exportNotes } from "./interpret";
import { detectKey } from "./key";
import { exportSequence } from "./sequence";
import { baselineFrom, docText, isSession, normalizeAnnotation, parseDoc, sessionFile, settingsOnLoad, workingNotesFrom, type SessionState } from "./session";
import { encodeUst } from "./ust";

const example = () => JSON.parse(readFileSync(new URL("../../crates/sona-core/tests/fixtures/annotation-example.json", import.meta.url), "utf8"));

const before: SessionState = { bpm: 99, division: 8, gridAnchor: 7, barOne: 3, flagThreshold: 0.2, snapKey: true };

/** Load the way the editor does: annotation, working notes, baseline, then settings. */
function load(parsed: any, current: SessionState = before) {
  const annotation = normalizeAnnotation(parsed);
  const notes = workingNotesFrom(parsed.notes);
  return { annotation, notes, baseline: baselineFrom(parsed), settings: settingsOnLoad(parsed, annotation, notes, current), session: isSession(parsed) };
}

/** The UST the editor exports for these notes and settings. */
function ust(notes: WorkingNote[], s: SessionState): Uint8Array {
  const frame = { bpm: s.bpm, division: s.division, anchor: s.gridAnchor, barOne: s.barOne };
  const effective = effectiveNotes(notes, frame, { flagThreshold: s.flagThreshold, snapKey: s.snapKey, key: detectKey(notes) });
  return encodeUst(exportSequence(exportNotes(effective), frame), { bpm: s.bpm, projectName: "sona" }).bytes;
}

describe("opening a fresh annotation", () => {
  it("takes its BPM, puts the grid phase on its first onset and keeps the other settings", () => {
    const got = load(example());
    expect(got.session).toBe(false);
    expect(got.settings).toEqual({ ...before, bpm: 174, gridAnchor: 0.086, barOne: null });
    expect(got.baseline.map((n) => [n.onset_s, n.midi_pitch, n.confidence])).toEqual(got.notes.map((n) => [n.onset_s, n.midi_pitch, n.confidence]));
    expect(got.baseline[0]).not.toBe(got.notes[0]);
  });

  it("puts bar 1 where the music put it, and falls back to 120 BPM and a 0 s anchor", () => {
    const raw = { ...example(), bpm: 0, notes: [], bar_one: { time_s: 1.5, confidence: 0.8, harmonic: true } };
    expect(load(raw).settings).toEqual({ ...before, bpm: 120, gridAnchor: 0, barOne: 1.5 });
  });

  it("refuses a file without notes", () => {
    expect(() => normalizeAnnotation({ bpm: 120 })).toThrow(/notes/);
  });
});

describe("a saved session", () => {
  it("reopens with the same notes, baseline and settings, whatever was open before (property)", () => {
    const rand = lcg(77);
    for (let trial = 0; trial < 200; trial++) {
      const parsed = load(example());
      const notes: WorkingNote[] = parsed.notes.map((n) => ({
        ...n,
        onset_s: n.onset_s + rand(),
        ...(rand() < 0.5 ? { manual: true as const } : {}),
        ...(rand() < 0.5 ? { lyric: ["ら", "きゃ", "a"][trial % 3] } : {}),
        ...(rand() < 0.5 ? { expr: { ...NEUTRAL_EXPRESSION, vibrato: null } } : {}),
      }));
      const settings: SessionState = {
        bpm: 40 + rand() * 200,
        division: DIVISIONS[trial % DIVISIONS.length],
        gridAnchor: rand() * 3 - 1,
        barOne: rand() < 0.5 ? null : rand() * 4,
        flagThreshold: rand(),
        snapKey: rand() < 0.5,
      };
      const file = JSON.parse(JSON.stringify(sessionFile(parsed.annotation, notes, parsed.baseline, settings)));
      const reopened = load(file, { bpm: 1, division: 4, gridAnchor: 0, barOne: null, flagThreshold: 0, snapKey: false });
      expect(reopened.session).toBe(true);
      expect(reopened.notes).toEqual(notes);
      expect(reopened.baseline).toEqual(parsed.baseline);
      expect(reopened.settings).toEqual(settings);
    }
  });

  it("exports the same UST bytes after a reopen on every grid the app offers, 1/16T included", () => {
    const parsed = load(example());
    const fresh: SessionState = { ...parsed.settings, division: 16 };
    for (const division of DIVISIONS) {
      const settings: SessionState = { ...fresh, division };
      const file = JSON.parse(JSON.stringify(sessionFile(parsed.annotation, parsed.notes, parsed.baseline, settings)));
      const reopened = load(file, fresh);
      expect(reopened.settings.division).toBe(division);
      expect(ust(reopened.notes, reopened.settings)).toEqual(ust(parsed.notes, settings));
    }
    expect(ust(parsed.notes, { ...fresh, division: 24 })).not.toEqual(ust(parsed.notes, fresh));
  });

  it("puts the grid phase on the first onset when the session has none, and keeps what it gets wrong", () => {
    const raw = { ...example(), session: { bpm: -1, division: 5, flagThreshold: "x", barOne: "1" } };
    expect(load(raw).settings).toEqual({ ...before, gridAnchor: 0.086, barOne: null, snapKey: false });
    expect(load({ ...raw, session: { flagThreshold: 4 } }).settings.flagThreshold).toBe(1);
  });
});

describe("the undo snapshot", () => {
  it("gives back the notes and settings it was taken from", () => {
    const { notes } = load(example());
    const settings: SessionState = { ...before, barOne: null };
    const back = parseDoc(docText(notes, settings));
    expect(back).toEqual({ notes, settings });
    expect(back.notes[0]).not.toBe(notes[0]);
  });
});
