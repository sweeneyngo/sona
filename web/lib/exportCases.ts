/**
 * Editor sessions for the export tests: raw notes from a fixture or a seeded
 * generator, snapped the way the editor snaps them (`quantizeNote`, then
 * `resolveOverlaps`), with the session's BPM, division and grid anchor.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { quantizeNote, resolveOverlaps, gridSpacingSeconds } from "./grid";
import type { Expression } from "./expression";
import type { LyricNote } from "./sequence";

export interface RawNote {
  onset_s: number;
  offset_s: number;
  midi_pitch: number;
  onset_clarity?: number;
  manual?: boolean;
  lyric?: string;
  expr?: Expression;
}

export interface EditorSession {
  name: string;
  bpm: number;
  division: number;
  anchor: number;
  notes: LyricNote[];
}

export const DIVISIONS = [4, 8, 16, 24, 32] as const;

export function lcg(seed: number): () => number {
  let s = seed;
  return () => (s = (s * 1103515245 + 12345) % 2147483648) / 2147483648;
}

/** The notes as the editor exports them: each snapped to the grid (or escaped), then made monophonic. */
export function snapLikeTheEditor(raw: RawNote[], bpm: number, division: number, anchor: number): LyricNote[] {
  const spacing = gridSpacingSeconds(bpm, division);
  const snapped = resolveOverlaps(
    raw.map((n) => {
      const q = { onset_s: n.onset_s, offset_s: n.offset_s, sub_scores: { onset_clarity: n.onset_clarity ?? 0 } };
      return quantizeNote(q, n.manual ? { spacing, anchor, escapeDistanceCells: Infinity } : { spacing, anchor });
    }),
    spacing,
  );
  return raw.map((n, i) => ({
    onset_s: snapped[i].onset_s,
    offset_s: snapped[i].offset_s,
    midi_pitch: n.midi_pitch,
    lyric: n.lyric,
    ...(n.expr ? { expr: n.expr } : {}),
  }));
}

const LYRICS = [undefined, "ら", "きゃ", "ー", "a", "ゔ🎵", " ", "ん"];

function randomExpr(rand: () => number): Expression {
  let x = -Math.round(rand() * 120);
  const points = Array.from({ length: 2 + Math.floor(rand() * 3) }, () => {
    const pt = { x, y: Math.round((rand() - 0.5) * 100), shape: (["io", "l", "i", "o"] as const)[Math.floor(rand() * 4)] };
    x += 1 + Math.round(rand() * 200);
    return pt;
  });
  const vibrato = rand() < 0.5 ? null : { length: 40, period: 170, depth: 30, in: 10, out: 10, shift: 0, drift: 0 };
  return { points, snapFirst: rand() < 0.5, vibrato };
}

/** Seeded random sessions: fresh ones (anchor on the first onset) and dragged ones (any anchor). */
export function randomSessions(count: number, seed: number): EditorSession[] {
  const rand = lcg(seed);
  const out: EditorSession[] = [];
  for (let c = 0; c < count; c++) {
    const bpm = rand() < 0.5 ? 60 + Math.floor(rand() * 200) : Math.round((40 + rand() * 260) * 10) / 10;
    const division = DIVISIONS[Math.floor(rand() * DIVISIONS.length)];
    const raw: RawNote[] = [];
    let t = rand() * 4;
    for (let k = 0; k < 1 + Math.floor(rand() * 20); k++) {
      const len = 0.01 + rand() * 0.7;
      raw.push({
        onset_s: t,
        offset_s: t + len,
        midi_pitch: 40 + rand() * 45,
        onset_clarity: rand(),
        manual: rand() < 0.15,
        lyric: LYRICS[Math.floor(rand() * LYRICS.length)],
        ...(rand() < 0.2 ? { expr: randomExpr(rand) } : {}),
      });
      t += len + (rand() < 0.4 ? 0 : rand() < 0.5 ? rand() * 0.05 : rand() * 1.5);
      if (rand() < 0.1) t -= rand() * len;
    }
    const fresh = rand() < 0.5;
    const anchor = fresh ? Math.min(...raw.map((n) => n.onset_s)) : (rand() - 0.2) * 6;
    out.push({ name: `random ${seed}/${c}`, bpm, division, anchor, notes: snapLikeTheEditor(raw, bpm, division, anchor) });
  }
  return out;
}

const FIXTURES = ["sona-core/tests/fixtures/annotation-example.json", ...["staccato", "legato", "vibrato", "bleed"].map((n) => `sona-eval/fixtures/${n}.ref.json`)];

/** The committed synthetic annotations, each at a few tempos, grids and anchors. */
export function fixtureSessions(): EditorSession[] {
  const out: EditorSession[] = [];
  for (const path of FIXTURES) {
    const doc = JSON.parse(readFileSync(fileURLToPath(new URL(`../../crates/${path}`, import.meta.url)), "utf8"));
    const raw: RawNote[] = doc.notes.map((n: { onset_s: number; offset_s: number; midi_pitch: number; sub_scores?: { onset_clarity: number } }) => ({
      onset_s: n.onset_s,
      offset_s: n.offset_s,
      midi_pitch: n.midi_pitch,
      onset_clarity: n.sub_scores?.onset_clarity,
    }));
    const first = Math.min(...raw.map((n) => n.onset_s));
    for (const [bpm, division, anchor] of [
      [doc.bpm ?? 120, 16, first],
      [150, 24, first],
      [97.5, 8, first + 0.031],
      [174, 32, -0.4],
    ] as const) {
      out.push({ name: `${path} ${bpm}/${division}@${anchor}`, bpm, division, anchor, notes: snapLikeTheEditor(raw, bpm, division, anchor) });
    }
  }
  return out;
}

export interface ReadMidi {
  ppq: number;
  usPerQuarter: number | null;
  timeSignature: number[] | null;
  /** Sung notes as `[tick, length, tone, lyric]`, the lyric being the one at the note-on's tick. */
  notes: [number, number, number, string][];
}

/** A small SMF reader for format-0 files with full status bytes, enough to read back what `encodeMidiFile` writes. */
export function readMidi(bytes: Uint8Array): ReadMidi {
  const text = (a: number, n: number) => String.fromCharCode(...bytes.slice(a, a + n));
  const u = (a: number, n: number) => bytes.slice(a, a + n).reduce((v, b) => v * 256 + b, 0);
  if (text(0, 4) !== "MThd" || text(14, 4) !== "MTrk") throw new Error("not a format-0 SMF");
  const ppq = u(12, 2);
  const end = 22 + u(18, 4);
  let i = 22;
  const vlq = () => {
    let v = 0;
    for (;;) {
      const b = bytes[i++];
      v = v * 128 + (b & 0x7f);
      if (b < 0x80) return v;
    }
  };
  let tick = 0;
  let usPerQuarter: number | null = null;
  let timeSignature: number[] | null = null;
  let lyric = { tick: -1, text: "" };
  const open = new Map<number, { tick: number; lyric: string }>();
  const notes: ReadMidi["notes"] = [];
  while (i < end) {
    tick += vlq();
    const status = bytes[i++];
    if (status === 0xff) {
      const type = bytes[i++];
      const len = vlq();
      const data = bytes.slice(i, i + len);
      i += len;
      if (type === 0x51) usPerQuarter = u(i - 3, 3);
      if (type === 0x58) timeSignature = [...data];
      if (type === 0x05) lyric = { tick, text: new TextDecoder().decode(data) };
    } else if ((status & 0xf0) === 0x90 && bytes[i + 1] > 0) {
      open.set(bytes[i], { tick, lyric: lyric.tick === tick ? lyric.text : "" });
      i += 2;
    } else if ((status & 0xf0) === 0x80 || (status & 0xf0) === 0x90) {
      const on = open.get(bytes[i]);
      if (on) notes.push([on.tick, tick - on.tick, bytes[i], on.lyric]);
      open.delete(bytes[i]);
      i += 2;
    } else {
      throw new Error(`unexpected status ${status.toString(16)}`);
    }
  }
  notes.sort((a, b) => a[0] - b[0]);
  return { ppq, usPerQuarter, timeSignature, notes };
}
