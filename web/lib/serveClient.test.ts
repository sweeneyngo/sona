import { afterEach, describe, expect, it, vi } from "vitest";
import { ServeError, defaultJob, estimateTempo, fetchJobOrCancel, isFinal, jobQuery, parseCapabilities, parseJobStatus } from "./serveClient";

describe("serve client boundary", () => {
  it("parses capabilities, treating anything but true as unavailable", () => {
    expect(parseCapabilities({ separation: { none: true, vocals: true, lead: "yes" }, engines: { pyin: true } })).toEqual({
      separation: { none: true, vocals: true, lead: false },
      engines: { pyin: true, game: false, cells: false },
      tempo: false,
      setup: [],
    });
    expect(parseCapabilities({ separation: {}, engines: {}, tempo: true }).tempo).toBe(true);
    expect(() => parseCapabilities(null)).toThrow(ServeError);
  });

  it("keeps the server's setup notes, dropping malformed ones", () => {
    const ffmpeg = { piece: "ffmpeg", status: "missing", blocks: ["vocals", "lead"], message: "ffmpeg doesn't run: install ffmpeg and restart sona serve." };
    const weights = { piece: "demucs-weights", status: "first-use", blocks: [], message: "The demucs weights download during the first separation." };
    const caps = parseCapabilities({ separation: {}, engines: {}, setup: [ffmpeg, weights, { piece: "x", status: "broken", message: "?" }, { piece: 3 }, null] });
    expect(caps.setup).toEqual([
      { piece: "ffmpeg", status: "missing", message: ffmpeg.message },
      { piece: "demucs-weights", status: "first-use", message: weights.message },
    ]);
  });

  it("defaults a job to the best separation and engine the server has", () => {
    const caps = (lead: boolean, vocals: boolean, game: boolean) => ({ separation: { none: true, vocals, lead }, engines: { pyin: true, game, cells: false }, tempo: true, setup: [] });
    expect(defaultJob(caps(true, true, true))).toEqual({ separation: "lead", engine: "game" });
    expect(defaultJob(caps(false, true, false))).toEqual({ separation: "vocals", engine: "pyin" });
    expect(defaultJob(caps(false, false, false))).toEqual({ separation: "none", engine: "pyin" });
    const withCells = { ...caps(true, true, true), engines: { pyin: true, game: true, cells: true } };
    expect(defaultJob(withCells)).toEqual({ separation: "lead", engine: "cells" });
    expect(defaultJob({ ...withCells, separation: { none: true, vocals: true, lead: false } }).engine).toBe("game");
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

  describe("the tempo estimate", () => {
    afterEach(() => vi.unstubAllGlobals());
    const answering = (res: () => Response) => vi.stubGlobal("fetch", async () => res());

    it("passes on the server's reason when it can't read the file (422)", async () => {
      answering(() => Response.json({ error: "ffmpeg could not read the file: Invalid data found when processing input" }, { status: 422 }));
      expect(await estimateTempo(new Blob(["x"]), "a.mp3")).toEqual({ error: "ffmpeg could not read the file: Invalid data found when processing input" });
    });

    it("tells an estimate and no steady beat apart from a failure", async () => {
      answering(() => Response.json({ bpm: 128.4 }));
      expect(await estimateTempo(new Blob(["x"]), "a.wav")).toEqual({ bpm: 128.4 });
      answering(() => Response.json({ bpm: null }));
      expect(await estimateTempo(new Blob(["x"]), "a.wav")).toEqual({ bpm: null });
      vi.stubGlobal("fetch", async () => {
        throw new TypeError("Failed to fetch");
      });
      expect(await estimateTempo(new Blob(["x"]), "a.wav")).toEqual({ error: expect.stringMatching(/isn't reachable/) });
    });
  });

  describe("a watcher that loses the job", () => {
    afterEach(() => vi.unstubAllGlobals());

    const server = (poll: () => Response) => {
      const calls: string[] = [];
      vi.stubGlobal("fetch", async (path: string, init?: RequestInit) => {
        calls.push(`${init?.method ?? "GET"} ${path}`);
        return init?.method === "DELETE" ? Response.json({ id: "j1", phase: "cancelled" }, { status: 202 }) : poll();
      });
      return calls;
    };

    it("cancels the job on the server when a poll fails, and still reports the failure", async () => {
      const calls = server(() => Response.json({ error: "boom" }, { status: 500 }));
      await expect(fetchJobOrCancel("j1")).rejects.toThrow("boom");
      expect(calls).toEqual(["GET /api/jobs/j1", "DELETE /api/jobs/j1"]);
    });

    it("leaves a job it can read alone", async () => {
      const calls = server(() => Response.json({ id: "j1", phase: "transcribing" }));
      expect((await fetchJobOrCancel("j1")).phase).toBe("transcribing");
      expect(calls).toEqual(["GET /api/jobs/j1"]);
    });
  });
});
