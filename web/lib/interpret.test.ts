import { describe, expect, it } from "vitest";
import type { WorkingNote } from "./annotation";
import { DIVISIONS, lcg } from "./exportCases";
import { cellSeconds, type Frame } from "./frame";
import { effectiveNotes, exportNotes, firstOnset, noteSpan, notesInTimeOrder, type EffectiveNote, type Reading } from "./interpret";
import { detectKey, isInKey } from "./key";
import { movedNote, newNote, resizedNote, withoutNote } from "./noteEdits";
import { TICKS_PER_QUARTER, exportSequence } from "./sequence";
import { docText, parseDoc, type SessionState } from "./session";
import { Undo } from "./undo";

const pick = <T>(rand: () => number, xs: readonly T[]): T => xs[Math.floor(rand() * xs.length)];

function randomFrame(rand: () => number): Frame {
  return {
    bpm: rand() < 0.5 ? 60 + Math.floor(rand() * 180) : Math.round((50 + rand() * 200) * 10) / 10,
    division: pick(rand, DIVISIONS),
    anchor: rand() * 3 - 0.5,
    barOne: rand() < 0.5 ? null : rand() * 6 - 0.5,
  };
}

function producerNotes(rand: () => number): WorkingNote[] {
  const out: WorkingNote[] = [];
  let t = rand() * 2;
  for (let k = 0; k < 1 + Math.floor(rand() * 12); k++) {
    const len = 0.02 + rand() * 0.6;
    out.push({
      onset_s: t,
      offset_s: t + len,
      midi_pitch: 50 + Math.floor(rand() * 25),
      confidence: rand(),
      sub_scores: { pitch_stability: rand(), voicing: rand(), onset_clarity: rand() < 0.6 ? rand() * 0.79 : rand(), offset_clarity: rand() },
    });
    t += rand() < 0.3 ? -rand() * len : len + (rand() < 0.5 ? 0 : rand() * 0.5);
    t = Math.max(0, t);
  }
  return out;
}

interface Doc {
  notes: WorkingNote[];
  settings: SessionState;
}

const frameOf = (s: SessionState): Frame => ({ bpm: s.bpm, division: s.division, anchor: s.gridAnchor, barOne: s.barOne });
const readingOf = (d: Doc): Reading => ({ flagThreshold: d.settings.flagThreshold, snapKey: d.settings.snapKey, key: detectKey(d.notes) });
const interpretDoc = (d: Doc) => effectiveNotes(d.notes, frameOf(d.settings), readingOf(d));

/** One thing a person does in the roll or the toolbar, the way `main.ts` applies it. */
function randomEdit(rand: () => number, d: Doc): void {
  const frame = frameOf(d.settings);
  const n = d.notes.length ? pick(rand, d.notes) : null;
  const dt = (rand() - 0.5) * 2;
  switch (Math.floor(rand() * 8)) {
    case 0:
    case 1:
      if (n) Object.assign(n, movedNote({ ...n }, dt, Math.round((rand() - 0.5) * 10)));
      break;
    case 2:
      if (n) Object.assign(n, resizedNote(n.onset_s, { ...n }, dt, frame));
      break;
    case 3:
      d.notes.push(newNote(rand() * 6, 40 + rand() * 40, frame));
      break;
    case 4:
      if (n) d.notes = withoutNote(d.notes, n);
      break;
    case 5:
      d.settings = { ...d.settings, gridAnchor: d.settings.gridAnchor + dt / 7 };
      break;
    case 6:
      d.settings = rand() < 0.5 ? { ...d.settings, bpm: Math.round((50 + rand() * 200) * 10) / 10 } : { ...d.settings, division: pick(rand, DIVISIONS) };
      break;
    case 7:
      d.settings = rand() < 0.5 ? { ...d.settings, snapKey: !d.settings.snapKey } : { ...d.settings, flagThreshold: Math.round(rand() * 100) / 100 };
      break;
  }
}

function randomDoc(rand: () => number): Doc {
  const f = randomFrame(rand);
  return {
    notes: producerNotes(rand),
    settings: { bpm: f.bpm, division: f.division, gridAnchor: f.anchor, barOne: f.barOne, flagThreshold: 0.5, snapKey: rand() < 0.3 },
  };
}

const onGrid = (t: number, f: Frame) => {
  const k = (t - f.anchor) / cellSeconds(f);
  return Math.abs(k - Math.round(k)) < 1e-6;
};

const sorted = (effs: EffectiveNote[]) => [...effs].sort((a, b) => a.onset_s - b.onset_s);

describe("effective notes after any sequence of edits (property)", () => {
  it("are one voice, keep every note, snap every manual note, and flag what they spare", () => {
    const rand = lcg(808);
    let states = 0;
    for (let trial = 0; trial < 200; trial++) {
      const d = randomDoc(rand);
      for (let step = 0; step < 25; step++) {
        randomEdit(rand, d);
        const effs = interpretDoc(d);
        states++;
        expect(effs.length === d.notes.length && effs.every((e, i) => e.raw === d.notes[i])).toBe(true);
        const line = sorted(effs);
        line.forEach((e, i) => {
          expect(e.offset_s).toBeGreaterThan(e.onset_s);
          if (i > 0) expect(e.onset_s).toBeGreaterThanOrEqual(line[i - 1].offset_s - 1e-9);
        });
        for (const e of effs) {
          if (e.raw.manual) expect(e.escaped).toBe(false);
          expect(e.flagged).toBe(e.raw.confidence < d.settings.flagThreshold || e.escaped);
        }
      }
    }
    expect(states).toBe(5000);
  });

  it("sit on the grid lines the roll draws when nothing escaped, and export to the ticks the roll shows", () => {
    const rand = lcg(4242);
    let exported = 0;
    for (let trial = 0; trial < 200; trial++) {
      const d = randomDoc(rand);
      for (let step = 0; step < 25; step++) {
        randomEdit(rand, d);
        const frame = frameOf(d.settings);
        const effs = interpretDoc(d);
        if (effs.some((e) => e.escaped)) continue;
        for (const e of effs) expect(onGrid(e.onset_s, frame) && onGrid(e.offset_s, frame)).toBe(true);
        const seq = exportSequence(exportNotes(effs), frame);
        const line = sorted(effs);
        if (!line.length || line[0].onset_s < seq.shiftS - 1e-9) continue;
        const ticksPerSecond = (frame.bpm / 60) * TICKS_PER_QUARTER;
        const tick = (t: number) => Math.round((t - seq.shiftS) * ticksPerSecond) + 0;
        expect(seq.notes.map((n) => [n.position, n.position + n.duration, n.tone])).toEqual(
          line.map((e) => [tick(e.onset_s), tick(e.offset_s), Math.round(e.pitch)]),
        );
        exported++;
      }
    }
    expect(exported).toBeGreaterThan(500);
  });

  it("are in the key when snap-to-key is on, and flag out-of-key pitches when it is off", () => {
    const rand = lcg(99);
    for (let trial = 0; trial < 100; trial++) {
      const d = randomDoc(rand);
      for (let step = 0; step < 10; step++) randomEdit(rand, d);
      const key = detectKey(d.notes);
      if (!key) continue;
      const frame = frameOf(d.settings);
      const on = effectiveNotes(d.notes, frame, { flagThreshold: 0.5, snapKey: true, key });
      const off = effectiveNotes(d.notes, frame, { flagThreshold: 0.5, snapKey: false, key });
      for (const e of on) expect([isInKey(e.pitch, key), e.outOfKey]).toEqual([true, false]);
      for (const e of off) expect([e.pitch, e.outOfKey]).toEqual([e.raw.midi_pitch, !isInKey(e.raw.midi_pitch, key)]);
    }
  });
});

describe("undo after any sequence of edits (property)", () => {
  it("steps back through every state exactly, and redo replays them", () => {
    const rand = lcg(2025);
    for (let trial = 0; trial < 60; trial++) {
      let d = randomDoc(rand);
      let t = 0;
      const undo = new Undo<string>({
        capture: () => docText(d.notes, d.settings),
        restore: (text) => {
          d = parseDoc(text);
        },
        equal: (a, b) => a === b,
        now: () => (t += 10_000),
      });
      const history = [docText(d.notes, d.settings)];
      const exports = [JSON.stringify(exportSequence(exportNotes(interpretDoc(d)), frameOf(d.settings)))];
      for (let step = 0; step < 20; step++) {
        const gesture = rand() < 0.4;
        if (gesture) undo.beginGesture();
        randomEdit(rand, d);
        undo.changed();
        if (gesture) {
          randomEdit(rand, d);
          undo.changed();
          undo.endGesture();
        }
        const now = docText(d.notes, d.settings);
        if (now !== history[history.length - 1]) {
          history.push(now);
          exports.push(JSON.stringify(exportSequence(exportNotes(interpretDoc(d)), frameOf(d.settings))));
        }
      }
      for (let k = history.length - 2; k >= 0; k--) {
        expect(undo.undo()).toBe(true);
        expect(docText(d.notes, d.settings)).toBe(history[k]);
        expect(JSON.stringify(exportSequence(exportNotes(interpretDoc(d)), frameOf(d.settings)))).toBe(exports[k]);
      }
      expect(undo.undo()).toBe(false);
      for (let k = 1; k < history.length; k++) {
        expect(undo.redo()).toBe(true);
        expect(docText(d.notes, d.settings)).toBe(history[k]);
      }
      expect(undo.redo()).toBe(false);
    }
  });
});

describe("effective-note helpers", () => {
  const frame: Frame = { bpm: 120, division: 16, anchor: 0, barOne: null };
  const note = (onset_s: number, offset_s: number, midi_pitch: number, extra: Partial<WorkingNote> = {}): WorkingNote => ({
    onset_s,
    offset_s,
    midi_pitch,
    confidence: 1,
    sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 0, offset_clarity: 1 },
    ...extra,
  });
  const read = (notes: WorkingNote[]) => effectiveNotes(notes, frame, { flagThreshold: 0.5, snapKey: false, key: null });

  it("spares a confident off-grid producer note but snaps the same note once the user has touched it", () => {
    const confident = { pitch_stability: 1, voicing: 1, onset_clarity: 0.95, offset_clarity: 1 };
    const [spared] = read([note(0.06, 0.3, 60, { sub_scores: confident })]);
    expect([spared.onset_s, spared.escaped, spared.flagged]).toEqual([0.06, true, true]);
    const [snapped] = read([note(0.06, 0.3, 60, { sub_scores: confident, manual: true })]);
    expect([snapped.onset_s, snapped.offset_s, snapped.escaped]).toEqual([0, 0.25, false]);
  });

  it("orders notes by when they sound, and gives a note's span the tone of the note it touches", () => {
    const a = note(0.5, 0.75, 62);
    const b = note(0, 0.5, 60);
    const c = note(1, 1.25, 64);
    const effs = read([a, b, c]);
    expect(notesInTimeOrder(effs)).toEqual([b, a, c]);
    expect(noteSpan(effs, a)).toEqual({ onset_s: 0.5, offset_s: 0.75, tone: 62, touchingPrevTone: 60 });
    expect(noteSpan(effs, c)?.touchingPrevTone).toBeNull();
    expect(noteSpan(effs, note(0, 1, 60))).toBeNull();
  });

  it("puts a fresh annotation's grid anchor on its first onset, or 0 s with no notes", () => {
    expect(firstOnset([note(1.2, 2, 60), note(0.7, 1, 60), note(3, 4, 60)])).toBe(0.7);
    expect(firstOnset([])).toBe(0);
  });
});
