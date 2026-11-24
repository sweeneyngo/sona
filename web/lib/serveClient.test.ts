import { afterEach, describe, expect, it, vi } from "vitest";
import { ServeError, defaultJob, estimateTempo, fetchJobOrCancel, isFinal, isSha256, jobQuery, parseCapabilities, parseJobStatus, songHash, submitJobByHash } from "./serveClient";

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

  it("asks for bar 1 only when the job opts in, and leaves the default query as it was", () => {
    const base = { bpm: 128, grid: 16, separation: "vocals", engine: "pyin", name: "a.wav" } as const;
    expect(jobQuery(base)).toBe("/api/jobs?bpm=128&grid=16&separation=vocals&engine=pyin&name=a.wav");
    expect(jobQuery({ ...base, barOne: false })).toBe(jobQuery(base));
    expect(new URLSearchParams(jobQuery({ ...base, barOne: true }).split("?")[1]).get("bar_one")).toBe("1");
  });

  it("sends the lyric with its line breaks, and leaves it out when the job has none", () => {
    const base = { bpm: 128, grid: 16, separation: "vocals", engine: "pyin", name: "a.wav" } as const;
    const lyric = "あめが ふる\nかぜが ふく&bpm=1";
    const q = new URLSearchParams(jobQuery({ ...base, lyric }, "f".repeat(64)).split("?")[1]);
    expect([q.get("lyric"), q.get("bpm"), q.get("upload")]).toEqual([lyric, "128", "f".repeat(64)]);
    expect(jobQuery({ ...base, lyric: undefined })).toBe(jobQuery(base));
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

  describe("a job named by the song's hash", () => {
    afterEach(() => vi.unstubAllGlobals());
    const req = { bpm: 61.5, grid: 24, separation: "lead", engine: "pyin", name: "a.mp3", barOne: true } as const;
    const sha = "0123456789abcdef".repeat(4);
    const server = (answer: () => Response) => {
      const sent: { path: string; body: unknown }[] = [];
      vi.stubGlobal("fetch", async (path: string, init?: RequestInit) => {
        sent.push({ path, body: init?.body });
        return answer();
      });
      return sent;
    };

    it("sends no audio, only the hash beside the job's settings", async () => {
      const sent = server(() => Response.json({ id: "j2", phase: "queued" }, { status: 201 }));
      expect(await submitJobByHash(req, sha)).toMatchObject({ id: "j2", phase: "queued" });
      expect(sent).toHaveLength(1);
      expect(sent[0].body).toBeUndefined();
      const q = new URLSearchParams(sent[0].path.split("?")[1]);
      expect([q.get("upload"), q.get("bpm"), q.get("grid"), q.get("separation"), q.get("bar_one")]).toEqual([sha, "61.5", "24", "lead", "1"]);
    });

    it("hands back the server's reason when it hasn't got the song or can't tell yet, so the file goes instead", async () => {
      server(() => Response.json({ error: "this server has no lead stems for that upload: send the file" }, { status: 404 }));
      expect(await submitJobByHash(req, sha)).toEqual({ refused: "this server has no lead stems for that upload: send the file" });
      server(() => Response.json({ error: "the server is still reading its separators' versions: send the file" }, { status: 409 }));
      expect(await submitJobByHash(req, sha)).toEqual({ refused: expect.stringMatching(/still reading/) });
    });

    it("fails on anything else, as an upload would", async () => {
      server(() => Response.json({ error: "upload must be the file's SHA-256" }, { status: 400 }));
      await expect(submitJobByHash(req, "nope")).rejects.toThrow("SHA-256");
      server(() => Response.json({ error: "boom" }, { status: 500 }));
      await expect(submitJobByHash(req, sha)).rejects.toThrow("boom");
      vi.stubGlobal("fetch", async () => {
        throw new TypeError("Failed to fetch");
      });
      await expect(submitJobByHash(req, sha)).rejects.toThrow(/isn't reachable/);
    });

    it("hashes a file as the server does", async () => {
      expect(await songHash(new Blob(["abc"]))).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
      expect(isSha256(await songHash(new Blob([])))).toBe(true);
      expect([isSha256(sha), isSha256(sha.toUpperCase()), isSha256(sha.slice(1)), isSha256(null)]).toEqual([true, false, false, false]);
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
