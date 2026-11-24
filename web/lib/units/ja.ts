import type { LanguageUnits, MoraKind, Problem, Unit } from "./index";

const SMALL = new Set("ぁぃぅぇぉゃゅょゎゕゖ");
const OPEN = new Set("あいうえおぁぃぅぇぉんー");
const LONG = "ー";

const VOWEL = new Map<string, string>();
for (const [vowel, row] of [
  ["あ", "あかさたなはまやらわがざだばぱぁゃゎゕ"],
  ["い", "いきしちにひみりぎじぢびぴぃゐ"],
  ["う", "うくすつぬふむゆるぐずづぶぷぅゅゔ"],
  ["え", "えけせてねへめれげぜでべぺぇゑゖ"],
  ["お", "おこそとのほもよろをごぞどぼぽぉょ"],
  ["ん", "ん"],
] as const) {
  for (const ch of row) VOWEL.set(ch, vowel);
}

/** The vowel a unit ends on (ん for ん), which a following ー holds. */
const vowelOf = (lyric: string): string | undefined => VOWEL.get(lyric[lyric.length - 1]);

/**
 * A long-vowel tail: ー, or a vowel kana that only lengthens the one before (かあ, とう, せい). A "held" tail
 * is one the dev USTs mostly sing inside the note before: ー (14 of 16) and おう (105 of 131). A "spelled" one
 * mostly has a note of its own there: えい (69 of 103) and a repeated vowel (45 of 66).
 */
type Tail = "held" | "spelled";

const tailOf = (before: string | undefined, ch: string): Tail | undefined => {
  if (before === undefined || !"あいうえお".includes(ch)) return undefined;
  if (before === "お" && ch === "う") return "held";
  return before === ch || (before === "え" && ch === "い") ? "spelled" : undefined;
};

const isHiragana = (ch: string): boolean => {
  const c = ch.codePointAt(0) ?? 0;
  return c >= 0x3041 && c <= 0x3096;
};

/** `sona_core::lyric_fit::mora_of`: a lone vowel, ん or ー is open; anything else starts on a consonant. */
const kindOf = (lyric: string): MoraKind => (lyric.length === 1 && OPEN.has(lyric) ? "open" : "consonant");

const unit = (lyric: string, start: number, optional: boolean): Unit => ({ lyric, kind: kindOf(lyric), optional, clipped: false, span: [start, start + 1] });

/**
 * One line's units as a Japanese UST writes them: a hiragana mora a note, small kana joined to the mora
 * before, ん its own; っ no note (it clips the mora before); a long-vowel tail an optional unit, which the
 * fit may give no note.
 */
function units(line: string): { unit: Unit; tail?: Tail }[] {
  const out: { unit: Unit; tail?: Tail }[] = [];
  let before: Unit | undefined;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    const last = out[out.length - 1]?.unit;
    if (ch === LONG) {
      const vowel = before && vowelOf(before.lyric);
      if (vowel) out.push({ unit: (before = unit(vowel, i, true)), tail: "held" });
    } else if (!isHiragana(ch)) {
      before = undefined;
    } else if (ch === "っ") {
      if (last) last.clipped = true;
      before = undefined;
    } else if (SMALL.has(ch) && before && before.lyric !== "ん" && !before.optional) {
      before.lyric += ch;
      before.kind = kindOf(before.lyric);
      before.span[1] = i + 1;
    } else {
      const tail = tailOf(before && vowelOf(before.lyric), ch);
      out.push({ unit: (before = unit(ch, i, tail !== undefined)), tail });
    }
  }
  return out;
}

/** The lyrics-first rule: hiragana (small kana and っ among them), ー, and spaces between phrases. */
function validate(line: string): Problem[] {
  const problems: Problem[] = [];
  let i = 0;
  for (const ch of line) {
    if (!isHiragana(ch) && ch !== LONG && !/\s/u.test(ch)) problems.push({ char: ch, start: i, end: i + ch.length });
    i += ch.length;
  }
  return problems;
}

export const japanese: LanguageUnits = {
  validate,
  split: (line) => units(line).map((u) => u.unit),
  inOrder: (line) => units(line).filter((u) => u.tail !== "held").map((u) => u.unit),
};
