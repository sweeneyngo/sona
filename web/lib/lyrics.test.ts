import { describe, expect, it } from "vitest";
import { assignLyrics, splitLyrics } from "./lyrics";

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
    const { assigned, leftover } = assignLyrics(["a", "b", "c"], 4, 2);
    expect([...assigned]).toEqual([
      [2, "a"],
      [3, "b"],
    ]);
    expect(leftover).toBe(1);
  });
});
