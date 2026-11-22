import { describe, expect, it } from "vitest";
import { ServeError, isFinal, jobQuery, parseCapabilities, parseJobStatus } from "./serveClient";

describe("serve client boundary", () => {
  it("parses capabilities, treating anything but true as unavailable", () => {
    expect(parseCapabilities({ separation: { none: true, vocals: true, lead: "yes" }, engines: { pyin: true } })).toEqual({
      separation: { none: true, vocals: true, lead: false },
      engines: { pyin: true, game: false },
    });
    expect(() => parseCapabilities(null)).toThrow(ServeError);
  });

  it("parses a job status and rejects unknown phases", () => {
    const s = parseJobStatus({ id: "a1", name: "song.wav", phase: "separating-lead", error: null, elapsed_s: 12.5, log: ["x", 3] });
    expect(s).toEqual({ id: "a1", name: "song.wav", phase: "separating-lead", error: null, elapsedS: 12.5, log: ["x"] });
    expect(isFinal(s.phase)).toBe(false);
    expect(isFinal(parseJobStatus({ id: "a", phase: "cancelled" }).phase)).toBe(true);
    expect(() => parseJobStatus({ id: "a", phase: "exploded" })).toThrow(ServeError);
  });

  it("encodes the job query so file names can't inject parameters", () => {
    const q = jobQuery({ bpm: 128, grid: 24, separation: "lead", engine: "game", name: "a&engine=pyin ✓.mp3" });
    const params = new URLSearchParams(q.split("?")[1]);
    expect(params.get("engine")).toBe("game");
    expect(params.get("name")).toBe("a&engine=pyin ✓.mp3");
    expect(params.getAll("engine")).toHaveLength(1);
  });
});
