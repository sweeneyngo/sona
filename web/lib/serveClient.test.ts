import { describe, expect, it } from "vitest";
import { ServeError, defaultJob, isFinal, jobQuery, parseCapabilities, parseJobStatus } from "./serveClient";

describe("serve client boundary", () => {
  it("parses capabilities, treating anything but true as unavailable", () => {
    expect(parseCapabilities({ separation: { none: true, vocals: true, lead: "yes" }, engines: { pyin: true } })).toEqual({
      separation: { none: true, vocals: true, lead: false },
      engines: { pyin: true, game: false },
      tempo: false,
    });
    expect(parseCapabilities({ separation: {}, engines: {}, tempo: true }).tempo).toBe(true);
    expect(() => parseCapabilities(null)).toThrow(ServeError);
  });

  it("defaults a job to the best separation and engine the server has", () => {
    const caps = (lead: boolean, vocals: boolean, game: boolean) => ({ separation: { none: true, vocals, lead }, engines: { pyin: true, game }, tempo: true });
    expect(defaultJob(caps(true, true, true))).toEqual({ separation: "lead", engine: "game" });
    expect(defaultJob(caps(false, true, false))).toEqual({ separation: "vocals", engine: "pyin" });
    expect(defaultJob(caps(false, false, false))).toEqual({ separation: "none", engine: "pyin" });
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
