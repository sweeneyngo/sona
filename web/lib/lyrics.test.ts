import { describe, expect, it } from "vitest";
import { assignLyrics, checkLyric, lyricsInOrder } from "./lyrics";

describe("lyricsInOrder", () => {
  it("gives each note of a hiragana lyric its mora, line after line", () => {
    expect(lyricsInOrder("きゃりー ぱみゅ\nあめが ふる")).toEqual(["きゃ", "り", "ぱ", "みゅ", "あ", "め", "が", "ふ", "る"]);
  });

  it("gives っ no note, and holds ー and an おう tail in the mora before", () => {
    expect(lyricsInOrder("いっしょに とうくー")).toEqual(["い", "しょ", "に", "と", "く"]);
  });

  it("returns nothing for an empty or blank lyric", () => {
    expect(lyricsInOrder("")).toEqual([]);
    expect(lyricsInOrder(" \n　\n")).toEqual([]);
  });
});

describe("assignLyrics", () => {
  it("fills consecutive notes from the start index and counts the overflow", () => {
    const { assigned, leftover } = assignLyrics({ tokens: ["a", "b", "c"], count: 4, from: 2 });
    expect([...assigned]).toEqual([
      [2, "a"],
      [3, "b"],
    ]);
    expect(leftover).toBe(1);
  });
});

describe("checkLyric", () => {
  it("takes hiragana with ー, small kana and っ, one sung line per line, and keeps the lines", () => {
    expect(checkLyric("きょうは  そらが\nあおいっ ねー\n")).toEqual({ kind: "ok", lyric: "きょうは  そらが\nあおいっ ねー" });
  });

  it("treats an empty or blank box as no lyric", () => {
    expect(checkLyric("")).toEqual({ kind: "empty" });
    expect(checkLyric(" \n　\n ")).toEqual({ kind: "empty" });
  });

  it("names the katakana it refuses, by line, and converts nothing", () => {
    expect(checkLyric("はるの ひ\nカゼが ふく")).toEqual({ kind: "bad", chars: ["カ", "ゼ"], message: "Hiragana only for now. Line 2: カ ゼ" });
  });

  it("refuses kanji, romaji and punctuation, and shows an invisible character by its code", () => {
    expect(checkLyric("ゆき、ふる\nyuki\n空​")).toEqual({
      kind: "bad",
      chars: ["、", "y", "u", "k", "i", "空", "​"],
      message: "Hiragana only for now. Line 1: 、 · Line 2: y u k i · Line 3: 空 U+200B",
    });
  });

  it("refuses half-width and full-width katakana alike, and iteration marks", () => {
    expect(checkLyric("ｶ").kind).toBe("bad");
    expect(checkLyric("ヴ").kind).toBe("bad");
    expect(checkLyric("ゝ").kind).toBe("bad");
  });

  it("names at most three lines", () => {
    const c = checkLyric("a\nb\nc\nd");
    expect(c.kind === "bad" && c.message).toBe("Hiragana only for now. Line 1: a · Line 2: b · Line 3: c · …");
  });

  it("writes Windows line breaks as plain ones and trims each line's end", () => {
    expect(checkLyric("\r\nあい  \r\nうえ\r\n")).toEqual({ kind: "ok", lyric: "あい\nうえ" });
  });
});
