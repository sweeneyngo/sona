import { japanese } from "./ja";

/** The languages sona has units for. Japanese only for now (lyrics-first map); a new one adds its module here. */
export type Lang = "ja";

/** How a unit begins, as `sona_core::lyric_fit::mora_of` reads a note's lyric. */
export type MoraKind = "consonant" | "open";

/** What one note sings. */
export interface Unit {
  /** What the note's lyric says (export writes it as is). */
  lyric: string;
  kind: MoraKind;
  /** A unit the singer may hold into the note before instead of giving it a note (a long-vowel tail). */
  optional: boolean;
  /** The unit is cut short by a following っ, which has no note of its own. */
  clipped: boolean;
  /** Where it came from in its line, as string offsets `[start, end)`. */
  span: [number, number];
}

/** A character the language's lyric box can't take, at `[start, end)` in its line. */
export interface Problem {
  char: string;
  start: number;
  end: number;
}

export interface LanguageUnits {
  /** The characters in one line the lyric box can't take, in order. */
  validate(line: string): Problem[];
  /** One line's units in sung order; characters `validate` reports make none. */
  split(line: string): Unit[];
  /** The units that get a note each when the line is laid in order with no fit (until lyrics-first 09/04). */
  inOrder(line: string): Unit[];
}

export function unitsFor(lang: Lang): LanguageUnits {
  switch (lang) {
    case "ja":
      return japanese;
  }
}
