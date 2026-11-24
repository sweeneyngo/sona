import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { DIVISIONS, fixtureSessions, lcg, randomSessions, readMidi, snapLikeTheEditor, type RawNote } from "./exportCases";
import { barNumber, barSeconds, cellSeconds, downbeat, gridLines, gridPhase, tickZero, updateFrame, type Frame, type FrameEvent, type LineKind } from "./frame";
import { snapTime } from "./grid";
import { encodeMidiFile } from "./midi";
import { TICKS_PER_QUARTER, exportSequence, sequenceInSeconds, type LyricNote, type Sequence } from "./sequence";
import { encodeUst, ustText } from "./ust";
import { ustxProject, ustxText } from "./ustx";

const BEAT_TICKS = TICKS_PER_QUARTER;
const BAR_TICKS = 4 * TICKS_PER_QUARTER;

const randomBpm = (rand: () => number) => (rand() < 0.5 ? 40 + Math.floor(rand() * 260) : Math.round((40 + rand() * 260) * 100) / 100);
const randomDivision = (rand: () => number) => DIVISIONS[Math.floor(rand() * DIVISIONS.length)];
const cellTicks = (f: Frame) => Math.round(cellSeconds(f) * (f.bpm / 60) * TICKS_PER_QUARTER);

describe("the default frame exports what the code before the frame exported", () => {
  const golden: { name: string; shiftS: number; ust: string; ustx: string }[] = JSON.parse(
    readFileSync(new URL("./fixtures/export-default-frame.json", import.meta.url), "utf8"),
  );
  const sha = (b: Uint8Array | string) => createHash("sha256").update(b).digest("hex").slice(0, 16);

  it("byte for byte, UST and USTX, on the committed fixtures and 400 random sessions", () => {
    const sessions = [...fixtureSessions(), ...randomSessions(400, 20261124)];
    expect(sessions).toHaveLength(golden.length);
    expect(golden.filter((g) => g.shiftS > 0.0005).length).toBeGreaterThan(400);
    sessions.forEach((s, i) => {
      const seq = exportSequence(s.notes, { bpm: s.bpm, division: s.division, anchor: s.anchor, barOne: null });
      const got = { name: s.name, shiftS: seq.shiftS, ust: sha(encodeUst(seq, { bpm: s.bpm, projectName: "sona" }).bytes), ustx: sha(ustxText(seq, { bpm: s.bpm, name: "sona" })) };
      expect(got).toEqual(golden[i]);
    });
  });
});

function ustSung(text: string): [number, number, number, string][] {
  let tick = 0;
  const out: [number, number, number, string][] = [];
  for (const m of text.matchAll(/^Length=(\d+)\r\nLyric=(.*)\r\nNoteNum=(\d+)/gm)) {
    if (m[2] !== "R") out.push([tick, Number(m[1]), Number(m[3]), m[2]]);
    tick += Number(m[1]);
  }
  return out;
}

function threeFormats(seq: Sequence, bpm: number) {
  const midi = readMidi(encodeMidiFile(seq, { bpm }));
  const scale = TICKS_PER_QUARTER / midi.ppq;
  return {
    midi: midi.notes.map(([t, l, tone, lyric]) => [t * scale, l * scale, tone, lyric]),
    ust: ustSung(ustText(seq, { bpm, charset: "utf-8" })),
    ustx: ustxProject(seq, { bpm }).voice_parts[0].notes.map((n) => [n.position, n.duration, n.tone]),
  };
}

function rawNotes(rand: () => number): LyricNote[] {
  const out: LyricNote[] = [];
  let t = rand() * 3;
  for (let k = 0; k < 1 + Math.floor(rand() * 14); k++) {
    const len = 0.0005 + rand() * 0.5;
    out.push({ onset_s: t, offset_s: t + len, midi_pitch: 45 + rand() * 30, lyric: rand() < 0.3 ? undefined : ["ら", "きゃ", "ん"][k % 3] });
    t += rand() < 0.3 ? -rand() * len : rand() < 0.5 ? len : len + rand() * 0.6;
    t = Math.max(0, t);
  }
  return out;
}

describe("MIDI, UST and USTX share one timeline (A01)", () => {
  it("place every note at the same tick, length and tone, with the same lyric in MIDI and UST (property)", () => {
    const rand = lcg(31);
    const sessions = randomSessions(150, 77);
    for (let trial = 0; trial < 300; trial++) {
      const s = trial < sessions.length ? sessions[trial] : { bpm: randomBpm(rand), division: randomDivision(rand), anchor: rand() * 4 - 1, notes: rawNotes(rand) };
      const frame: Frame = { bpm: s.bpm, division: s.division, anchor: s.anchor, barOne: rand() < 0.5 ? null : rand() * 8 - 1 };
      const seq = exportSequence(s.notes, frame);
      const { midi, ust, ustx } = threeFormats(seq, s.bpm);
      const want = seq.notes.map((n) => [n.position, n.duration, n.tone]);
      expect(ustx).toEqual(want);
      expect(ust.map((n) => n.slice(0, 3))).toEqual(want);
      expect(midi.map((n) => n.slice(0, 3))).toEqual(want);
      expect(midi.map((n) => n[3])).toEqual(ust.map((n) => n[3]));
    }
  });

  it("writes the MIDI on the UST's grid where the stem-seconds MIDI was 48 ticks late (the audit's case)", () => {
    const notes: LyricNote[] = [
      { onset_s: 1.05, offset_s: 1.3, midi_pitch: 64 },
      { onset_s: 1.3, offset_s: 1.55, midi_pitch: 66 },
    ];
    const seq = exportSequence(notes, { bpm: 120, division: 16, anchor: 1.05, barOne: null });
    expect(readMidi(encodeMidiFile(seq, { bpm: 120 })).notes.map((n) => n.slice(0, 2))).toEqual([
      [960, 240],
      [1200, 240],
    ]);
  });
});

/** Notes drawn on a random choice of the editor's lines, each to a later line. */
function notesOnLines(frame: Frame, rand: () => number): { notes: LyricNote[]; kinds: LineKind[] } {
  const lines = gridLines(frame, 0, 12 + rand() * 20);
  const notes: LyricNote[] = [];
  const kinds: LineKind[] = [];
  let i = Math.floor(rand() * 6);
  while (i < lines.length - 1) {
    const j = Math.min(lines.length - 1, i + 1 + Math.floor(rand() * 6));
    notes.push({ onset_s: lines[i].t, offset_s: lines[j].t, midi_pitch: 60 });
    kinds.push(lines[i].kind);
    i = j + (rand() < 0.5 ? 0 : Math.floor(rand() * 8));
  }
  return { notes, kinds };
}

/**
 * Every exported note is on the line it was drawn on: beat lines on beats, bar
 * lines on bars, the rest on cells. Returns what went wrong, if anything.
 */
function linesExportMisses(frame: Frame, notes: LyricNote[], kinds: LineKind[]): string[] {
  const seq = exportSequence(notes, frame);
  const back = sequenceInSeconds(seq, frame.bpm);
  const halfTick = 0.5 / ((frame.bpm / 60) * TICKS_PER_QUARTER);
  const misses: string[] = [];
  seq.notes.forEach((n, i) => {
    const ok =
      Math.abs(back[i].onset_s - notes[i].onset_s) < halfTick &&
      n.position % cellTicks(frame) === 0 &&
      (kinds[i] === "cell") === (n.position % BEAT_TICKS !== 0) &&
      (kinds[i] === "bar") === (n.position % BAR_TICKS === 0);
    if (!ok) misses.push(`${JSON.stringify(frame)}: a note on a ${kinds[i]} line at ${notes[i].onset_s} s exported to tick ${n.position}`);
  });
  return misses;
}

describe("OpenUtau's beats and bars are the editor's (A02)", () => {
  it("a note on a beat line exports to a multiple of 480 ticks, on a bar line to a multiple of 1920 (property)", () => {
    const rand = lcg(2026);
    let bars = 0;
    const misses: string[] = [];
    for (let trial = 0; trial < 1000; trial++) {
      const frame: Frame = { bpm: randomBpm(rand), division: randomDivision(rand), anchor: rand() * 20 - 5, barOne: rand() * 30 - 5 };
      const { notes, kinds } = notesOnLines(frame, rand);
      misses.push(...linesExportMisses(frame, notes, kinds));
      bars += kinds.filter((k) => k === "bar").length;
    }
    expect(misses.slice(0, 5)).toEqual([]);
    expect(bars).toBeGreaterThan(500);
  });

  it("holds for the default frame too, whose bar 1 is the first grid line", () => {
    const rand = lcg(4);
    const misses: string[] = [];
    for (let trial = 0; trial < 500; trial++) {
      const frame: Frame = { bpm: randomBpm(rand), division: randomDivision(rand), anchor: rand() * 20 - 5, barOne: null };
      const { notes, kinds } = notesOnLines(frame, rand);
      misses.push(...linesExportMisses(frame, notes, kinds));
    }
    expect(misses.slice(0, 5)).toEqual([]);
  });

  it("puts a pickup before the first bar line in the bar before it, not on tick 0", () => {
    const frame: Frame = { bpm: 120, division: 16, anchor: 0, barOne: 1.0 };
    const seq = exportSequence(
      [
        { onset_s: 0.5, offset_s: 1.0, midi_pitch: 60 },
        { onset_s: 1.0, offset_s: 1.5, midi_pitch: 62 },
      ],
      frame,
    );
    expect(seq.shiftS).toBeCloseTo(-1, 9);
    expect(seq.notes.map((n) => [n.position, n.duration])).toEqual([
      [1440, 480],
      [1920, 480],
    ]);
  });

  it("numbers bars as OpenUtau does, tick 0 being bar 1", () => {
    const frame: Frame = { bpm: 120, division: 16, anchor: 0.05, barOne: 4.05 };
    const t0 = tickZero(frame, [3]);
    expect(t0).toBeCloseTo(0.05, 9);
    const bars = gridLines(frame, 0, 6.1).filter((l) => l.kind === "bar");
    expect(bars.map((l) => barNumber(frame, l.t, t0))).toEqual([1, 2, 3, 4]);
    expect(bars.map((l) => Math.round(l.t * 1000))).toEqual([50, 2050, 4050, 6050]);
  });
});

describe("frame events (random sequences)", () => {
  function randomEvent(f: Frame, rand: () => number): FrameEvent {
    const r = rand();
    if (r < 0.2) return { type: "tempo", bpm: randomBpm(rand) };
    if (r < 0.35) return { type: "division", division: randomDivision(rand) };
    if (r < 0.65) return { type: "drag", from: f, dt: (rand() - 0.5) * 3 };
    if (r < 0.9) return { type: "bar-one", t: rand() * 20 };
    return { type: "auto-bar-one" };
  }

  function kindAt(f: Frame, t: number): LineKind | undefined {
    const cell = cellSeconds(f);
    return gridLines(f, t - cell / 2, t + cell / 2).find((l) => Math.abs(l.t - t) < 1e-6)?.kind;
  }

  it("keep bar 1 on a grid line, honour the line the user chose, move bars with a drag, and export beats on beats", () => {
    const rand = lcg(11);
    const misses: string[] = [];
    let steps = 0;
    for (let run = 0; run < 200; run++) {
      let f: Frame = { bpm: randomBpm(rand), division: randomDivision(rand), anchor: rand() * 3, barOne: null };
      for (let k = 0; k < 25; k++) {
        const e = randomEvent(f, rand);
        const before = f;
        f = updateFrame(f, e);
        steps++;
        const cell = cellSeconds(f);
        const d = downbeat(f);
        const t0 = tickZero(f, [0]);
        const bars = (d - t0) / barSeconds(f);
        const miss = (why: string) => misses.push(`${why} after ${JSON.stringify({ ...e, from: undefined })} on ${JSON.stringify(before)}`);
        if (Math.abs(d - snapTime(d, cell, f.anchor)) > 1e-9) miss("bar 1 off the grid");
        if (Math.abs(bars - Math.round(bars)) > 1e-6) miss("tick 0 not a bar line");
        if (e.type === "bar-one" && kindAt(f, snapTime(e.t, cell, f.anchor)) !== "bar") miss("the chosen line is not a bar line");
        if (e.type === "drag") {
          for (const l of gridLines(before, 6, 14)) if (kindAt(f, l.t + e.dt) !== l.kind) miss(`the ${l.kind} line at ${l.t} did not move with the drag`);
        }
        if (e.type === "auto-bar-one" && tickZero(f, [10]) !== gridPhase(f.anchor, cell)) miss("auto is not the default rule");
        if ((e.type === "tempo" || e.type === "division") && f.barOne !== before.barOne) miss("bar 1 moved in the stem");
        const { notes, kinds } = notesOnLines(f, rand);
        misses.push(...linesExportMisses(f, notes, kinds));
      }
    }
    expect(misses.slice(0, 5)).toEqual([]);
    expect(steps).toBe(5000);
  });

  it("change nothing on an invalid tempo or grid", () => {
    const f: Frame = { bpm: 120, division: 16, anchor: 0.1, barOne: 2 };
    expect(updateFrame(f, { type: "tempo", bpm: 0 })).toBe(f);
    expect(updateFrame(f, { type: "tempo", bpm: NaN })).toBe(f);
    expect(updateFrame(f, { type: "division", division: 0 })).toBe(f);
  });
});

describe("export helpers", () => {
  it("the snapped fixtures are what the editor shows: one voice, on its grid unless escaped", () => {
    const raw: RawNote[] = [
      { onset_s: 0.26, offset_s: 0.5, midi_pitch: 60 },
      { onset_s: 0.49, offset_s: 0.7, midi_pitch: 62, manual: true },
    ];
    expect(snapLikeTheEditor(raw, 120, 16, 0).map((n) => [n.onset_s, n.offset_s])).toEqual([
      [0.25, 0.5],
      [0.5, 0.75],
    ]);
  });
});
