import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { DEFAULT_LYRIC, TICKS_PER_QUARTER, exportSequence, type LyricNote } from "./sequence";
import { gridPhase, type Frame } from "./frame";
import { DIVISIONS } from "./grid";
import { encodeShiftJis } from "./sjis";
import { encodeUst, ustText } from "./ust";
import { toYaml, ustxProject, ustxText } from "./ustx";

const BPM = 120;
const SPACING = 0.125;
const PHASE_S = 0.05;
const ANCHOR = 8 * SPACING + PHASE_S;
const CELL_TICKS = (SPACING * BPM * TICKS_PER_QUARTER) / 60;
const notes: LyricNote[] = [
  { onset_s: 1.05, offset_s: 1.3, midi_pitch: 64, lyric: "さ" },
  { onset_s: 1.3, offset_s: 1.55, midi_pitch: 66, lyric: "くら" },
  { onset_s: 1.8, offset_s: 2.05, midi_pitch: 67.2, lyric: "きゃ" },
  { onset_s: 2.05, offset_s: 2.3, midi_pitch: 67 },
];

const frameAt = (anchor: number): Frame => ({ bpm: BPM, division: 16, anchor, barOne: null });

const GOLDEN = fileURLToPath(new URL("../../crates/sona-eval/fixtures/ust/sona-export.ust", import.meta.url));

describe("exportSequence, default frame", () => {
  const seq = exportSequence(notes, frameAt(ANCHOR));

  it("shifts by the grid phase so notes land on whole ticks of the editor grid", () => {
    expect(gridPhase(ANCHOR, SPACING)).toBeCloseTo(PHASE_S, 9);
    expect(seq.shiftS).toBeCloseTo(PHASE_S, 9);
    expect(seq.notes.map((n) => [n.position, n.duration])).toEqual([
      [960, 240],
      [1200, 240],
      [1680, 240],
      [1920, 240],
    ]);
    for (const n of seq.notes) expect(n.position % CELL_TICKS).toBe(0);
  });

  it("rounds pitch and gives a lyric-less note the default lyric", () => {
    expect(seq.notes.map((n) => n.tone)).toEqual([64, 66, 67, 67]);
    expect(seq.notes[3].lyric).toBe(DEFAULT_LYRIC);
  });

  it("keeps the line monophonic and every note at least one tick long", () => {
    const s = exportSequence(
      [
        { onset_s: 0, offset_s: 1, midi_pitch: 60 },
        { onset_s: 0.5, offset_s: 0.5, midi_pitch: 62 },
        { onset_s: 0.5, offset_s: 0.75, midi_pitch: 64 },
      ],
      frameAt(0),
    );
    for (let i = 1; i < s.notes.length; i++) {
      expect(s.notes[i].position).toBeGreaterThanOrEqual(s.notes[i - 1].position + s.notes[i - 1].duration);
    }
    expect(s.notes.every((n) => n.duration >= 1)).toBe(true);
    expect(s.notes).toHaveLength(3);
    expect(s.notes[0].duration).toBe(480);
  });
});

describe("UST export", () => {
  const seq = exportSequence(notes, frameAt(ANCHOR));

  it("is contiguous: rests fill every gap, from tick 0", () => {
    const text = ustText(seq, { bpm: BPM, projectName: "golden" });
    const lengths = [...text.matchAll(/^Length=(\d+)/gm)].map((m) => Number(m[1]));
    const lyrics = [...text.matchAll(/^Lyric=(.*)$/gm)].map((m) => m[1].trim());
    expect(lyrics).toEqual(["R", "さ", "くら", "R", "きゃ", DEFAULT_LYRIC]);
    expect(lengths).toEqual([960, 240, 240, 240, 240, 240]);
    expect(text.endsWith("[#TRACKEND]\r\n")).toBe(true);
    expect(text).not.toMatch(/[^\r]\n/);
  });

  it("places every note exactly where the sequence says, for random note sets (property)", () => {
    let seed = 7;
    const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
    for (let trial = 0; trial < 200; trial++) {
      const raw: LyricNote[] = [];
      let t = rand() * 2;
      for (let k = 0; k < 1 + Math.floor(rand() * 12); k++) {
        const len = 0.001 + rand() * 0.6;
        raw.push({ onset_s: t, offset_s: t + len, midi_pitch: 48 + Math.floor(rand() * 36), lyric: rand() < 0.3 ? undefined : "ら" });
        t += len + (rand() < 0.5 ? 0 : rand() < 0.5 ? 0.0011 : rand() * 0.8);
      }
      const frame: Frame = { bpm: 60 + rand() * 140, division: DIVISIONS[Math.floor(rand() * DIVISIONS.length)], anchor: rand() * 3, barOne: rand() < 0.5 ? null : rand() * 4 };
      const s = exportSequence(raw, frame);
      const text = ustText(s, { bpm: 120 });
      let tick = 0;
      const sung: [number, number, number][] = [];
      for (const m of text.matchAll(/^Length=(\d+)\r\nLyric=(.*)\r\nNoteNum=(\d+)/gm)) {
        const len = Number(m[1]);
        expect(len).toBeGreaterThan(0);
        if (m[2] !== "R") sung.push([tick, len, Number(m[3])]);
        tick += len;
      }
      expect(sung).toEqual(s.notes.map((n) => [n.position, n.duration, n.tone]));
    }
  });

  it("encodes kana as Shift-JIS, and falls back to UTF-8 with a Charset line", () => {
    const { bytes, encoding } = encodeUst(seq, { bpm: BPM });
    expect(encoding).toBe("shift_jis");
    expect(new TextDecoder("shift_jis").decode(bytes)).toContain("Lyric=きゃ");
    const odd = exportSequence([{ onset_s: 0, offset_s: 0.5, midi_pitch: 60, lyric: "ゔ🎵" }], frameAt(0));
    const utf = encodeUst(odd, { bpm: BPM });
    expect(utf.encoding).toBe("utf-8");
    expect(new TextDecoder().decode(utf.bytes)).toContain("Charset=UTF-8\r\n");
  });

  it("matches the golden file the Rust UST parser reads back (sona-eval ust.rs)", () => {
    const { bytes } = encodeUst(seq, { bpm: BPM, projectName: "golden" });
    if (process.env.UPDATE_GOLDEN) writeFileSync(GOLDEN, bytes);
    expect(Buffer.from(bytes).equals(readFileSync(GOLDEN))).toBe(true);
  });

  it("encodes ASCII, kana and kanji, and rejects what Shift-JIS can't hold", () => {
    expect(Array.from(encodeShiftJis("Aあ漢") ?? [])).toEqual([0x41, 0x82, 0xa0, 0x8a, 0xbf]);
    expect(encodeShiftJis("🎵")).toBeNull();
  });
});

describe("USTX export", () => {
  const seq = exportSequence(notes, frameAt(ANCHOR));

  it("puts every note in one voice part with OpenUtau's melisma alias", () => {
    const p = ustxProject(exportSequence([...notes, { onset_s: 2.3, offset_s: 2.55, midi_pitch: 67, lyric: "ー" }], frameAt(ANCHOR)), { bpm: BPM });
    const part = p.voice_parts[0];
    expect(part.notes.map((n) => n.lyric)).toEqual(["さ", "くら", "きゃ", DEFAULT_LYRIC, "+~"]);
    expect(part.notes[0]).toMatchObject({ position: 960, duration: 240, tone: 64 });
    expect(part.duration).toBe(2400);
    expect(p.resolution).toBe(480);
    expect(p.tempos).toEqual([{ position: 0, bpm: BPM }]);
  });

  it("emits YAML a reader can't misparse: quoted strings, nested blocks", () => {
    expect(toYaml({ a: "x: y", b: [1, { c: true }], d: [], e: {} })).toBe('a: "x: y"\nb:\n  - 1\n  - c: true\nd: []\ne: {}');
    const text = ustxText(seq, { bpm: BPM });
    expect(text).toContain('ustx_version: "0.7"');
    expect(text).toContain('    lyric: "きゃ"');
  });
});

describe("pitch expression export (ADR-0019)", () => {
  const expr = {
    points: [
      { x: -80, y: 0, shape: "io" as const },
      { x: 30, y: 0, shape: "l" as const },
      { x: 400, y: 0, shape: "o" as const },
      { x: 500, y: -25, shape: "i" as const },
    ],
    snapFirst: true,
    vibrato: { length: 55, period: 170, depth: 35, in: 20, out: 15, shift: 30, drift: -5 },
  };
  const seq = exportSequence([{ onset_s: 0, offset_s: 0.5, midi_pitch: 64, lyric: "ら", expr }], frameAt(0));

  // OpenUtau's UstNote.ParsePitchBend / ParseVibrato (MIT), ported for the round trip.
  function parseLikeOpenUtau(block: string) {
    const get = (k: string) => block.match(new RegExp(`^${k}=(.*)\\r?$`, "m"))?.[1] ?? "";
    const [x0, y0] = get("PBS").split(";").map(Number);
    const points = [{ x: x0, y: y0, shape: "io" }];
    const w = get("PBW").split(",").map(Number);
    const y = get("PBY").split(",").map(Number);
    let x = x0;
    w.forEach((wi, i) => {
      x += wi;
      points.push({ x, y: y[i] ?? 0, shape: "io" });
    });
    const shapes: Record<string, string> = { r: "o", s: "l", j: "i" };
    get("PBM").split(",").forEach((m, i) => points[i] && (points[i].shape = shapes[m] ?? "io"));
    const v = get("VBR").split(",").map(Number);
    return { points, vibrato: { length: v[0], period: v[1], depth: v[2], in: v[3], out: v[4], shift: v[5], drift: v[6] } };
  }

  it("writes UST Mode2 lines that OpenUtau's parser reads back as the same points and vibrato", () => {
    const text = ustText(seq, { bpm: BPM });
    const block = text.split("[#0000]")[1];
    const back = parseLikeOpenUtau(block);
    expect(back.points).toEqual(expr.points);
    expect(back.vibrato).toEqual(expr.vibrato);
    expect(text).toContain("PBM=,s,r,j\r\n");
  });

  it("writes the same points and vibrato into the USTX note", () => {
    const note = ustxProject(seq, { bpm: BPM }).voice_parts[0].notes[0];
    expect(note.pitch).toEqual({ data: expr.points, snap_first: true });
    expect(note.vibrato).toEqual({ ...expr.vibrato, vol_link: 0 });
  });

  it("round-trips random point sets of every shape through OpenUtau's UST parser (property)", () => {
    let seed = 5;
    const rand = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
    const shapes = ["io", "l", "i", "o"] as const;
    for (let trial = 0; trial < 100; trial++) {
      let x = -Math.round(rand() * 200) || 0;
      const points = Array.from({ length: 2 + Math.floor(rand() * 5) }, () => {
        const pt = { x, y: Math.round((rand() - 0.5) * 240) || 0, shape: shapes[Math.floor(rand() * 4)] };
        x += 1 + Math.round(rand() * 300);
        return pt;
      });
      const e = { points, snapFirst: rand() < 0.5, vibrato: null };
      const s = exportSequence([{ onset_s: 0, offset_s: 1, midi_pitch: 60, expr: e }], frameAt(0));
      expect(parseLikeOpenUtau(ustText(s, { bpm: BPM }).split("[#0000]")[1]).points).toEqual(points);
    }
  });

  it("leaves notes without an expression at OpenUtau's neutral defaults and writes no pitch lines", () => {
    const plain = exportSequence([{ onset_s: 0, offset_s: 0.5, midi_pitch: 64 }], frameAt(0));
    expect(ustText(plain, { bpm: BPM })).not.toMatch(/PBS|VBR/);
    expect(ustxProject(plain, { bpm: BPM }).voice_parts[0].notes[0].vibrato.length).toBe(0);
  });
});
