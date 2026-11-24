import { describe, expect, it } from "vitest";
import { unitsFor } from "./index";

const ja = unitsFor("ja");
const lyrics = (line: string) => ja.split(line).map((u) => u.lyric);

describe("Japanese units: one hiragana mora per note", () => {
  it("gives each mora a unit, with its kind and where it came from", () => {
    expect(ja.split("かい")).toEqual([
      { lyric: "か", kind: "consonant", optional: false, clipped: false, span: [0, 1] },
      { lyric: "い", kind: "open", optional: false, clipped: false, span: [1, 2] },
    ]);
  });

  it("joins small kana to the mora before", () => {
    expect(lyrics("きゃうぃふぉくゎ")).toEqual(["きゃ", "うぃ", "ふぉ", "くゎ"]);
    expect(ja.split("きゃ")[0].span).toEqual([0, 2]);
  });

  it("gives ん its own unit, an open one, and never joins a small kana to it", () => {
    expect(ja.split("さんゃ").map((u) => [u.lyric, u.kind])).toEqual([
      ["さ", "consonant"],
      ["ん", "open"],
      ["ゃ", "consonant"],
    ]);
  });

  it("calls a lone vowel open and anything with a consonant or two kana consonant, as mora_of does", () => {
    expect(ja.split("いぇを ぁ ゔ").map((u) => [u.lyric, u.kind])).toEqual([
      ["いぇ", "consonant"],
      ["を", "consonant"],
      ["ぁ", "open"],
      ["ゔ", "consonant"],
    ]);
  });
});

describe("Japanese units: っ is not a note", () => {
  it("makes no unit for っ and marks the mora before it clipped", () => {
    expect(ja.split("いっしょ").map((u) => [u.lyric, u.clipped])).toEqual([
      ["い", true],
      ["しょ", false],
    ]);
  });

  it("keeps the clipped mora's span to its own kana", () => {
    expect(ja.split("きゃっと")[0]).toMatchObject({ lyric: "きゃ", clipped: true, span: [0, 2] });
  });

  it("drops a っ with no mora before it in the line, and joins no small kana to it", () => {
    expect(ja.split("っか").map((u) => [u.lyric, u.clipped])).toEqual([["か", false]]);
    expect(lyrics("かっゃ")).toEqual(["か", "ゃ"]);
  });
});

describe("Japanese units: a long-vowel tail is optional", () => {
  const tails = (line: string) => ja.split(line).map((u) => [u.lyric, u.optional]);

  it("turns ー into an optional unit singing the vowel before, an open one", () => {
    expect(ja.split("きゃー")).toEqual([
      { lyric: "きゃ", kind: "consonant", optional: false, clipped: false, span: [0, 2] },
      { lyric: "あ", kind: "open", optional: true, clipped: false, span: [2, 3] },
    ]);
    expect(tails("ぴーー")).toEqual([
      ["ぴ", false],
      ["い", true],
      ["い", true],
    ]);
  });

  it("makes a spelled-out long vowel optional: the same vowel again, おう and えい", () => {
    expect(tails("かあ とう せい すう")).toEqual([
      ["か", false],
      ["あ", true],
      ["と", false],
      ["う", true],
      ["せ", false],
      ["い", true],
      ["す", false],
      ["う", true],
    ]);
  });

  it("keeps a vowel that changes the sound as a unit of its own", () => {
    expect(tails("かい こえ あお")).toEqual([
      ["か", false],
      ["い", false],
      ["こ", false],
      ["え", false],
      ["あ", false],
      ["お", false],
    ]);
  });

  it("starts no tail across a space or a っ", () => {
    expect(tails("か あ")).toEqual([
      ["か", false],
      ["あ", false],
    ]);
    expect(tails("かっあ")).toEqual([
      ["か", false],
      ["あ", false],
    ]);
  });

  it("gives a ー with no mora before it in its word no unit", () => {
    expect(lyrics("ーか ー")).toEqual(["か"]);
  });
});

describe("Japanese lyric box: hiragana only", () => {
  it("takes hiragana, small kana, っ, ー and spaces", () => {
    expect(ja.validate("きゃっと ぴーぃ　ゔゕゖ\t")).toEqual([]);
  });

  it("names each character it can't take, where it is in the line", () => {
    expect(ja.validate("あカ 空x、")).toEqual([
      { char: "カ", start: 1, end: 2 },
      { char: "空", start: 3, end: 4 },
      { char: "x", start: 4, end: 5 },
      { char: "、", start: 5, end: 6 },
    ]);
  });

  it("names a character outside the BMP whole", () => {
    expect(ja.validate("あ😀い")).toEqual([{ char: "😀", start: 1, end: 3 }]);
  });

  it("refuses half-width katakana and iteration marks", () => {
    expect(ja.validate("ｶゝ").map((p) => p.char)).toEqual(["ｶ", "ゝ"]);
  });
});

describe("Japanese units laid in order, with no fit", () => {
  const laid = (line: string) => ja.inOrder(line).map((u) => u.lyric);

  it("holds ー and an おう tail in the mora before, as the UST mostly does", () => {
    expect(laid("ひかりー とうきょう")).toEqual(["ひ", "か", "り", "と", "きょ"]);
  });

  it("still gives えい and a repeated vowel their own notes, as the UST mostly does", () => {
    expect(laid("せんせい かあさん")).toEqual(["せ", "ん", "せ", "い", "か", "あ", "さ", "ん"]);
  });

  it("gives っ no note", () => {
    expect(laid("きって")).toEqual(["き", "て"]);
  });
});
