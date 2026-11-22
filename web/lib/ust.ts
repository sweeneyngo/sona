/**
 * A UST is a contiguous list of blocks in ticks: every block starts where the
 * previous one ended, and silence is an explicit block with lyric `R`.
 * Classic UTAU reads USTs as Shift-JIS (CP932).
 */

import { encodeShiftJis } from "./sjis";
import type { Expression } from "./expression";
import type { Sequence } from "./sequence";

const UST_SHAPE = { io: "", l: "s", o: "r", i: "j" } as const;

/** Mode2 pitch and vibrato lines, as OpenUtau's `UstNote.WritePitch`/`WriteVibrato` write them. */
export function ustExpressionLines(expr: Expression): string[] {
  const lines: string[] = [];
  const p = expr.points;
  if (p.length >= 2) {
    lines.push(
      `PBS=${p[0].x};${p[0].y}`,
      `PBW=${p.slice(1).map((pt, i) => pt.x - p[i].x).join(",")}`,
      `PBY=${p.slice(1).map((pt) => pt.y).join(",")}`,
      `PBM=${p.map((pt) => UST_SHAPE[pt.shape]).join(",")}`,
    );
  }
  const v = expr.vibrato;
  if (v && v.length > 0) lines.push(`VBR=${v.length},${v.period},${v.depth},${v.in},${v.out},${v.shift},${v.drift}`);
  return lines;
}

export type UstEncoding = "shift_jis" | "utf-8";

/** The UST text for `seq` at `bpm`, with lines ending in CRLF as UTAU writes them. */
export function ustText(seq: Sequence, opts: { bpm: number; projectName?: string; charset?: UstEncoding }): string {
  const lines: string[] = ["[#VERSION]", "UST Version1.2"];
  if (opts.charset === "utf-8") lines.push("Charset=UTF-8");
  lines.push(
    "[#SETTING]",
    `Tempo=${formatTempo(opts.bpm)}`,
    "Tracks=1",
    `ProjectName=${(opts.projectName ?? "sona").replace(/[\r\n]/g, " ")}`,
    "Mode2=True",
  );
  let index = 0;
  let tick = 0;
  let lastTone = seq.notes[0]?.tone ?? 60;
  const block = (length: number, lyric: string, tone: number, expr?: Expression) => {
    lines.push(
      `[#${String(index++).padStart(4, "0")}]`,
      `Length=${length}`,
      `Lyric=${lyric}`,
      `NoteNum=${tone}`,
      "PreUtterance=",
      "Intensity=100",
      "Modulation=0",
      ...(expr ? ustExpressionLines(expr) : []),
    );
  };
  for (const n of seq.notes) {
    if (n.position > tick) block(n.position - tick, "R", lastTone);
    block(n.duration, n.lyric.replace(/[\r\n]/g, " "), n.tone, n.expr);
    tick = n.position + n.duration;
    lastTone = n.tone;
  }
  lines.push("[#TRACKEND]");
  return lines.join("\r\n") + "\r\n";
}

/** Encode the UST: Shift-JIS when possible, else UTF-8 with a Charset line. */
export function encodeUst(seq: Sequence, opts: { bpm: number; projectName?: string }): { bytes: Uint8Array; encoding: UstEncoding } {
  const sjis = encodeShiftJis(ustText(seq, { ...opts, charset: "shift_jis" }));
  if (sjis) return { bytes: sjis, encoding: "shift_jis" };
  return { bytes: new TextEncoder().encode(ustText(seq, { ...opts, charset: "utf-8" })), encoding: "utf-8" };
}

function formatTempo(bpm: number): string {
  return Number.isInteger(bpm) ? `${bpm}.00` : String(Math.round(bpm * 100) / 100);
}
