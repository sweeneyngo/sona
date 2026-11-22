/**
 * UST (UTAU sequence) export (ADR-0018).
 *
 * A UST is a contiguous list of blocks in ticks (480 per quarter): every
 * block starts where the previous one ended, and silence is an explicit block
 * with lyric `R`. Only the musical fields are written; synthesis parameters
 * are left at UTAU's defaults.
 *
 * The file is encoded as Shift-JIS when every character allows it, since
 * classic UTAU reads Shift-JIS. Otherwise (ゔ, emoji, most non-Japanese
 * scripts) it is UTF-8 with `Charset=UTF-8`, which UTAU ≥ 0.4.18 and OpenUtau
 * honour. sona's own Rust parser (`sona-eval` `ust.rs`) reads both.
 */

import { encodeShiftJis } from "./sjis";
import type { Sequence } from "./sequence";

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
  const block = (length: number, lyric: string, tone: number) => {
    lines.push(
      `[#${String(index++).padStart(4, "0")}]`,
      `Length=${length}`,
      `Lyric=${lyric}`,
      `NoteNum=${tone}`,
      "PreUtterance=",
      "Intensity=100",
      "Modulation=0",
    );
  };
  for (const n of seq.notes) {
    if (n.position > tick) block(n.position - tick, "R", lastTone);
    block(n.duration, n.lyric.replace(/[\r\n]/g, " "), n.tone);
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
