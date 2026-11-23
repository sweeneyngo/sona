import { describe, expect, it } from "vitest";
import { phaseProgress, startStep, usableBpm, type StartEvent, type StartState } from "./startFlow";

const run = (events: StartEvent[], from: StartState = { kind: "idle", error: null }) => events.reduce(startStep, from);

describe("start flow", () => {
  it("goes from a dropped song to a running job with the suggested tempo", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "tempo", song: 1, bpm: 246.13 },
      { type: "halve" },
      { type: "transcribe", attempt: 1 },
      { type: "submitted", attempt: 1, job: "j1" },
      { type: "progress", job: "j1", phase: "separating-vocals", elapsedS: 3, log: [], error: null },
    ]);
    expect(s).toMatchObject({ kind: "running", bpm: 123.1, job: "j1", phase: "separating-vocals" });
    expect(run([{ type: "results-loaded", job: "j1" }], s)).toEqual({ kind: "loaded" });
  });

  it("ignores a tempo answer for a song that was replaced while it was being detected", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "choose", song: 2, name: "b.mp3" },
      { type: "tempo", song: 1, bpm: 200 },
    ]);
    expect(s).toEqual({ kind: "detecting", song: 2, name: "b.mp3" });
    expect(run([{ type: "tempo", song: 2, bpm: 150 }], s)).toMatchObject({ kind: "ready", name: "b.mp3", bpm: 150, detected: 150 });
  });

  it("asks for a tempo when none was found, and refuses to start without a usable one", () => {
    const s = run([{ type: "choose", song: 1, name: "a.wav" }, { type: "tempo", song: 1, bpm: null }, { type: "transcribe", attempt: 1 }]);
    expect(s).toMatchObject({ kind: "ready", bpm: null });
    expect((s as { error: string }).error).toMatch(/tempo/);
    expect(run([{ type: "set-bpm", bpm: 900 }, { type: "transcribe", attempt: 1 }], s).kind).toBe("ready");
    expect(run([{ type: "set-bpm", bpm: 128 }, { type: "transcribe", attempt: 1 }], s).kind).toBe("running");
  });

  it("keeps one job: a new song can't be dropped while one runs, and stale progress is ignored", () => {
    const s = run([{ type: "choose", song: 1, name: "a" }, { type: "tempo", song: 1, bpm: 180 }, { type: "transcribe", attempt: 1 }, { type: "submitted", attempt: 1, job: "j1" }]);
    expect(run([{ type: "choose", song: 2, name: "b" }], s)).toBe(s);
    expect(run([{ type: "progress", job: "old", phase: "done", elapsedS: 1, log: [], error: null }], s)).toBe(s);
    expect(run([{ type: "results-loaded", job: "old" }], s)).toBe(s);
  });

  it("comes back to the song, tempo kept, after a failure or a cancel", () => {
    const s = run([{ type: "choose", song: 1, name: "a" }, { type: "tempo", song: 1, bpm: 180 }, { type: "transcribe", attempt: 1 }, { type: "submitted", attempt: 1, job: "j1" }]);
    expect(run([{ type: "progress", job: "j1", phase: "failed", elapsedS: 9, log: [], error: "demucs crashed" }], s)).toMatchObject({ kind: "ready", bpm: 180, error: "It failed: demucs crashed." });
    expect(run([{ type: "cancel" }], s)).toMatchObject({ kind: "ready", bpm: 180, error: "Cancelled." });
    const before = run([{ type: "choose", song: 1, name: "a" }, { type: "tempo", song: 1, bpm: 180 }, { type: "transcribe", attempt: 1 }]);
    expect(run([{ type: "failed-to-submit", attempt: 1, message: "server gone" }], before)).toMatchObject({ kind: "ready", error: "server gone" });
  });

  it("never adopts the job of an attempt that was cancelled during its upload", () => {
    const ready = run([{ type: "choose", song: 1, name: "a" }, { type: "tempo", song: 1, bpm: 180 }]);
    const second = run([{ type: "transcribe", attempt: 1 }, { type: "cancel" }, { type: "transcribe", attempt: 2 }], ready);
    expect(run([{ type: "submitted", attempt: 1, job: "cancelled-one" }], second)).toMatchObject({ kind: "running", attempt: 2, job: null });
    expect(run([{ type: "submitted", attempt: 2, job: "mine" }], second)).toMatchObject({ kind: "running", job: "mine" });
    expect(run([{ type: "failed-to-submit", attempt: 1, message: "x" }], second)).toMatchObject({ kind: "running", attempt: 2 });
  });

  it("stays put without sona serve, and recovers when it appears", () => {
    const s = run([{ type: "serve-missing", reason: "no server" }, { type: "choose", song: 1, name: "a" }]);
    expect(s).toEqual({ kind: "unavailable", reason: "no server" });
    expect(run([{ type: "serve-found" }], s)).toEqual({ kind: "idle", error: null });
  });

  it("never runs a job with an unusable tempo, whatever the order of events", () => {
    const events: StartEvent[] = [
      { type: "choose", song: 1, name: "a" },
      { type: "choose", song: 2, name: "b" },
      { type: "tempo", song: 1, bpm: 300 },
      { type: "tempo", song: 2, bpm: null },
      { type: "tempo", song: 2, bpm: 15 },
      { type: "set-bpm", bpm: 0 },
      { type: "set-bpm", bpm: 140 },
      { type: "set-bpm", bpm: null },
      { type: "halve" },
      { type: "double" },
      { type: "transcribe", attempt: 1 },
      { type: "submitted", attempt: 1, job: "j" },
      { type: "submitted", attempt: 2, job: "k" },
      { type: "progress", job: "j", phase: "transcribing", elapsedS: 1, log: [], error: null },
      { type: "cancel" },
      { type: "start-over" },
    ];
    let seed = 3;
    const rnd = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
    for (let trial = 0; trial < 3000; trial++) {
      let s: StartState = { kind: "idle", error: null };
      for (let k = 0; k < 12; k++) {
        const prev = s;
        s = startStep(s, events[Math.floor(rnd() * events.length)]);
        if (s.kind === "running") expect(usableBpm(s.bpm)).toBe(true);
        if (s.kind === "running" && prev.kind === "running") expect(s.song).toBe(prev.song);
        if (s.kind === "ready" && s.detected !== null) expect(s.song).toBe(prev.kind === "detecting" ? prev.song : (prev as { song: number }).song);
      }
    }
  });
});

describe("phaseProgress", () => {
  it("runs from 0 to 1 along the job's own phases", () => {
    expect(phaseProgress("queued", "lead")).toBe(0);
    expect(phaseProgress("separating-lead", "lead")).toBe(0.5);
    expect(phaseProgress("done", "none")).toBe(1);
  });
});
