const SMALL_KANA = new Set("ぁぃぅぇぉゃゅょゎゕゖァィゥェォャュョヮヵヶㇰㇱㇲㇳㇴㇵㇶㇷㇸㇹㇺㇻㇼㇽㇾㇿ");
const OWN_MORA = new Set("ーっッんン");

function isKana(ch: string): boolean {
  const c = ch.codePointAt(0) ?? 0;
  return (c >= 0x3041 && c <= 0x309f) || (c >= 0x30a0 && c <= 0x30ff) || (c >= 0x31f0 && c <= 0x31ff);
}

function isKanji(ch: string): boolean {
  const c = ch.codePointAt(0) ?? 0;
  return (c >= 0x4e00 && c <= 0x9fff) || (c >= 0x3400 && c <= 0x4dbf) || c === 0x3005 || (c >= 0xf900 && c <= 0xfaff);
}

const SEPARATOR = /[\s、。，．,.!?！？「」『』（）()［］[\]【】〈〉《》"'“”‘’…・〜~♪☆★:;：；/／\-–—]/u;

/** Split a lyric text into one token per note, in order. */
const foldHalfAndFullWidth = (text: string): string => text.normalize("NFKC");

export function splitLyrics(text: string): string[] {
  const tokens: string[] = [];
  let word = "";
  const flushWord = () => {
    if (word) tokens.push(word);
    word = "";
  };
  for (const ch of foldHalfAndFullWidth(text)) {
    if (SEPARATOR.test(ch)) {
      flushWord();
    } else if (isKana(ch)) {
      flushWord();
      const prev = tokens[tokens.length - 1];
      if (SMALL_KANA.has(ch) && prev !== undefined && isKana(prev[prev.length - 1]) && !OWN_MORA.has(prev)) {
        tokens[tokens.length - 1] = prev + ch;
      } else {
        tokens.push(ch);
      }
    } else if (isKanji(ch)) {
      flushWord();
      tokens.push(ch);
    } else {
      word += ch;
    }
  }
  flushWord();
  return tokens;
}

/** A lyric box's text: nothing yet, a lyric sona takes (line breaks kept), or the characters it can't take. */
export type LyricCheck = { kind: "empty" } | { kind: "ok"; lyric: string } | { kind: "bad"; chars: string[]; message: string };

const isLyricChar = (ch: string): boolean => {
  const c = ch.codePointAt(0) ?? 0;
  return (c >= 0x3041 && c <= 0x3096) || c === 0x30fc || /\s/u.test(ch);
};

const shown = (ch: string): string => (/[\p{C}\p{Z}]/u.test(ch) ? `U+${(ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, "0")}` : ch);

const MAX_LINES_NAMED = 3;

/**
 * Check a pasted lyric (lyrics-first spec): hiragana, ー, small kana and っ, with spaces and line breaks
 * between phrases. Anything else is named by line, never converted.
 */
export function checkLyric(text: string): LyricCheck {
  const lyric = text
    .split(/\r\n|\r|\n/)
    .map((l) => l.trimEnd())
    .join("\n")
    .replace(/^\n+|\n+$/g, "");
  if (!lyric.trim()) return { kind: "empty" };
  const chars: string[] = [];
  const named: string[] = [];
  lyric.split("\n").forEach((line, i) => {
    const bad = [...new Set([...line].filter((ch) => !isLyricChar(ch) && !chars.includes(ch)))];
    if (!bad.length) return;
    chars.push(...bad);
    named.push(`Line ${i + 1}: ${bad.map(shown).join(" ")}`);
  });
  if (!chars.length) return { kind: "ok", lyric };
  const lines = named.length > MAX_LINES_NAMED ? [...named.slice(0, MAX_LINES_NAMED), "…"] : named;
  return { kind: "bad", chars, message: `Hiragana only for now. ${lines.join(" · ")}` };
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
