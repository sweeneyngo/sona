import { unitsFor } from "./units";

const japanese = unitsFor("ja");

const lines = (text: string): string[] => text.split(/\r\n|\r|\n/);

/** Each note's lyric when a lyric is laid on the notes in order, a unit a note, with no fit (lyrics-first 08). */
export function lyricsInOrder(text: string): string[] {
  return lines(text).flatMap((line) => japanese.inOrder(line).map((u) => u.lyric));
}

/** A lyric box's text: nothing yet, a lyric sona takes (line breaks kept), or the characters it can't take. */
export type LyricCheck = { kind: "empty" } | { kind: "ok"; lyric: string } | { kind: "bad"; chars: string[]; message: string };

const shown = (ch: string): string => (/[\p{C}\p{Z}]/u.test(ch) ? `U+${(ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0")}` : ch);

const MAX_LINES_NAMED = 3;

/**
 * Check a pasted lyric (lyrics-first spec): hiragana, ー, small kana and っ, with spaces and line breaks
 * between phrases. Anything else is named by line, never converted.
 */
export function checkLyric(text: string): LyricCheck {
  const lyric = lines(text)
    .map((l) => l.trimEnd())
    .join("\n")
    .replace(/^\n+|\n+$/g, "");
  if (!lyric.trim()) return { kind: "empty" };
  const chars: string[] = [];
  const named: string[] = [];
  lyric.split("\n").forEach((line, i) => {
    const bad = [...new Set(japanese.validate(line).map((p) => p.char))].filter((ch) => !chars.includes(ch));
    if (!bad.length) return;
    chars.push(...bad);
    named.push(`Line ${i + 1}: ${bad.map(shown).join(" ")}`);
  });
  if (!chars.length) return { kind: "ok", lyric };
  const named3 = named.length > MAX_LINES_NAMED ? [...named.slice(0, MAX_LINES_NAMED), "…"] : named;
  return { kind: "bad", chars, message: `Hiragana only for now. ${named3.join(" · ")}` };
}

export function assignLyrics({ tokens, count, from }: { tokens: string[]; count: number; from: number }): {
  assigned: Map<number, string>;
  leftover: number;
} {
  const assigned = new Map<number, string>();
  let i = 0;
  for (let n = Math.max(0, from); n < count && i < tokens.length; n++, i++) {
    assigned.set(n, tokens[i]);
  }
  return { assigned, leftover: tokens.length - i };
}
