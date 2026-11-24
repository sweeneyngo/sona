import { describe, expect, it } from "vitest";
import { assignLyrics, checkLyric, splitLyrics } from "./lyrics";

describe("splitLyrics", () => {
  it("splits kana into morae, joining small kana to the one before", () => {
    expect(splitLyrics("きゃりーぱみゅぱみゅ")).toEqual(["きゃ", "り", "ー", "ぱ", "みゅ", "ぱ", "みゅ"]);
  });

  it("keeps ー, っ and ん as their own morae", () => {
    expect(splitLyrics("いっしょにんー")).toEqual(["い", "っ", "しょ", "に", "ん", "ー"]);
  });

  it("does not attach a small kana to っ or ー", () => {
    expect(splitLyrics("っゃ")).toEqual(["っ", "ゃ"]);
  });

  it("splits on whitespace and punctuation, keeping Latin words whole", () => {
    expect(splitLyrics("sa ku ra、hello world!")).toEqual(["sa", "ku", "ra", "hello", "world"]);
  });

  it("gives each kanji its own token and folds half-width katakana", () => {
    expect(splitLyrics("星のｶｹﾗ")).toEqual(["星", "の", "カ", "ケ", "ラ"]);
  });

  it("returns nothing for empty or punctuation-only text", () => {
    expect(splitLyrics("")).toEqual([]);
    expect(splitLyrics(" 、。!? ")).toEqual([]);
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
