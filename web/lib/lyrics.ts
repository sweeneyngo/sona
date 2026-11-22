/**
 * Lyrics → one token per note (ADR-0018).
 *
 * UTAU/OpenUtau sing one lyric per note, and Japanese singing is counted in
 * **morae**, not characters or syllables:
 * - a small kana joins the kana before it (きゃ, しゅ, ふぁ);
 * - ー, っ and ん are morae of their own (a long vowel, a geminate, a moraic n);
 * - other kana are one mora each.
 *
 * Everything that isn't kana is split on whitespace and punctuation:
 * - Latin text (romaji, English) stays one token per word, so separate romaji
 *   morae with spaces (`sa ku ra`);
 * - each kanji is its own token, since readings need a dictionary the browser
 *   doesn't have. Paste kana for best results.
 */

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

/** Splits text on whitespace and on Japanese and Western punctuation. */
const SEPARATOR = /[\s、。，．,.!?！？「」『』（）()［］[\]【】〈〉《》"'“”‘’…・〜~♪☆★:;：；/／\-–—]/u;

/** Split a lyric text into one token per note, in order. */
export function splitLyrics(text: string): string[] {
  const tokens: string[] = [];
  let word = "";
  const flushWord = () => {
    if (word) tokens.push(word);
    word = "";
  };
  // NFKC folds half-width katakana and full-width Latin into their usual forms.
  for (const ch of text.normalize("NFKC")) {
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

/**
 * Assign `tokens` to `count` notes starting at note `from`. Returns the new
 * lyric for each note index it touched, plus how many tokens didn't fit.
 */
export function assignLyrics(tokens: string[], count: number, from: number): { assigned: Map<number, string>; leftover: number } {
  const assigned = new Map<number, string>();
  let i = 0;
  for (let n = Math.max(0, from); n < count && i < tokens.length; n++, i++) {
    assigned.set(n, tokens[i]);
  }
  return { assigned, leftover: tokens.length - i };
}
