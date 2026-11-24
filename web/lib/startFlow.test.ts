import { describe, expect, it } from "vitest";
import { hintBpm, parseBpm, phaseProgress, startStep, usableBpm, type StartEvent, type StartState } from "./startFlow";

const run = (events: StartEvent[], from: StartState = { kind: "idle", error: null }) => events.reduce(startStep, from);

describe("start flow", () => {
  it("asks for the BPM: the estimate is a hint that fills the field only when picked", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "tempo", song: 1, bpm: 246.13 },
    ]);
    expect(s).toMatchObject({ kind: "ready", bpm: null, detected: 246.1 });
    const refused = run([{ type: "transcribe", attempt: 1 }], s);
    expect(refused.kind).toBe("ready");
    expect((refused as { error: string }).error).toMatch(/BPM/);
    expect(run([{ type: "use-hint", multiple: 0.5 }], s)).toMatchObject({ bpm: 123 });
    expect(run([{ type: "use-hint", multiple: 1 }], s)).toMatchObject({ bpm: 246 });
    const going = run([{ type: "use-hint", multiple: 0.5 }, { type: "transcribe", attempt: 1 }, { type: "submitted", attempt: 1, job: "j1" }, { type: "progress", job: "j1", phase: "separating-vocals", elapsedS: 3, log: [], error: null }], refused);
    expect(going).toMatchObject({ kind: "running", bpm: 123, job: "j1", phase: "separating-vocals" });
    expect(run([{ type: "results-loaded", job: "j1" }], going)).toEqual({ kind: "loaded" });
  });

  it("takes a typed decimal BPM before the estimate arrives, and keeps it when the estimate comes", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "set-bpm", bpm: parseBpm("123,45") },
      { type: "transcribe", attempt: 1 },
    ]);
    expect(s).toMatchObject({ kind: "running", bpm: 123.45, detected: "pending" });
    const later = run([{ type: "tempo", song: 1, bpm: 246.9 }, { type: "cancel" }], s);
    expect(later).toMatchObject({ kind: "ready", bpm: 123.45, detected: 246.9, error: "Cancelled." });
  });

  it("reads a typed BPM with a point or a comma, and nothing else", () => {
    expect(parseBpm(" 128 ")).toBe(128);
    expect(parseBpm("127.5")).toBe(127.5);
    expect(parseBpm("127,5")).toBe(127.5);
    expect(parseBpm(".5")).toBe(0.5);
    expect(parseBpm("")).toBeNull();
    expect(parseBpm("12a")).toBeNull();
    expect(parseBpm("1e3")).toBeNull();
    expect(parseBpm("-120")).toBeNull();
    expect(parseBpm("1.2.3")).toBeNull();
  });

  it("offers the estimate and its half as whole BPMs", () => {
    expect(hintBpm(246.1, 1)).toBe(246);
    expect(hintBpm(246.1, 0.5)).toBe(123);
    expect(hintBpm(187.5, 0.5)).toBe(94);
  });

  it("ignores a tempo answer for a song that was replaced while it was being detected", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "set-bpm", bpm: 128 },
      { type: "choose", song: 2, name: "b.mp3" },
      { type: "tempo", song: 1, bpm: 200 },
    ]);
    expect(s).toEqual({ kind: "ready", song: 2, name: "b.mp3", bpm: null, detected: "pending", hash: null, file: true, error: null });
    expect(run([{ type: "tempo", song: 2, bpm: 150 }], s)).toMatchObject({ kind: "ready", name: "b.mp3", bpm: null, detected: 150 });
  });

  it("has nothing to pick when no beat was found, and refuses to start without a usable BPM", () => {
    const s = run([{ type: "choose", song: 1, name: "a.wav" }, { type: "tempo", song: 1, bpm: null }, { type: "use-hint", multiple: 1 }, { type: "transcribe", attempt: 1 }]);
    expect(s).toMatchObject({ kind: "ready", bpm: null, detected: null });
    expect((s as { error: string }).error).toMatch(/BPM/);
    expect(run([{ type: "set-bpm", bpm: 900 }, { type: "transcribe", attempt: 1 }], s).kind).toBe("ready");
    expect(run([{ type: "set-bpm", bpm: parseBpm("abc") }, { type: "transcribe", attempt: 1 }], s).kind).toBe("ready");
    expect(run([{ type: "set-bpm", bpm: 128 }, { type: "transcribe", attempt: 1 }], s).kind).toBe("running");
  });

  it("shows why the tempo couldn't be estimated, for the song on screen only, and still takes a typed BPM", () => {
    const s = run([
      { type: "choose", song: 1, name: "a.mp3" },
      { type: "choose", song: 2, name: "b.mp3" },
      { type: "tempo-failed", song: 1, message: "about a.mp3" },
    ]);
    expect(s).toMatchObject({ kind: "ready", song: 2, detected: "pending" });
    const failed = run([{ type: "tempo-failed", song: 2, message: "ffmpeg could not read the file" }], s);
    expect(failed).toMatchObject({ kind: "ready", bpm: null, detected: { error: "ffmpeg could not read the file" } });
    expect(run([{ type: "tempo", song: 2, bpm: 120 }, { type: "use-hint", multiple: 1 }], failed)).toMatchObject({ bpm: null, detected: { error: "ffmpeg could not read the file" } });
    expect(run([{ type: "set-bpm", bpm: 120 }, { type: "transcribe", attempt: 1 }], failed)).toMatchObject({ kind: "running", bpm: 120 });
  });

  it("keeps one job: a new song can't be dropped while one runs, and stale progress is ignored", () => {
    const s = run([{ type: "choose", song: 1, name: "a" }, { type: "set-bpm", bpm: 180 }, { type: "transcribe", attempt: 1 }, { type: "submitted", attempt: 1, job: "j1" }]);
    expect(run([{ type: "choose", song: 2, name: "b" }], s)).toBe(s);
    expect(run([{ type: "set-bpm", bpm: 90 }], s)).toBe(s);
    expect(run([{ type: "progress", job: "old", phase: "done", elapsedS: 1, log: [], error: null }], s)).toBe(s);
    expect(run([{ type: "results-loaded", job: "old" }], s)).toBe(s);
  });

  it("comes back to the song, BPM and hint kept, after a failure or a cancel", () => {
    const s = run([{ type: "choose", song: 1, name: "a" }, { type: "tempo", song: 1, bpm: 180 }, { type: "set-bpm", bpm: 90 }, { type: "transcribe", attempt: 1 }, { type: "submitted", attempt: 1, job: "j1" }]);
    expect(run([{ type: "progress", job: "j1", phase: "failed", elapsedS: 9, log: [], error: "demucs crashed" }], s)).toMatchObject({ kind: "ready", bpm: 90, detected: 180, error: "It failed: demucs crashed." });
    expect(run([{ type: "cancel" }], s)).toMatchObject({ kind: "ready", bpm: 90, detected: 180, error: "Cancelled." });
    const before = run([{ type: "choose", song: 1, name: "a" }, { type: "set-bpm", bpm: 180 }, { type: "transcribe", attempt: 1 }]);
    expect(run([{ type: "failed-to-submit", attempt: 1, message: "server gone" }], before)).toMatchObject({ kind: "ready", bpm: 180, error: "server gone" });
  });

  it("never adopts the job of an attempt that was cancelled during its upload", () => {
    const ready = run([{ type: "choose", song: 1, name: "a" }, { type: "set-bpm", bpm: 180 }]);
    const second = run([{ type: "transcribe", attempt: 1 }, { type: "cancel" }, { type: "transcribe", attempt: 2 }], ready);
    expect(run([{ type: "submitted", attempt: 1, job: "cancelled-one" }], second)).toMatchObject({ kind: "running", attempt: 2, job: null });
    expect(run([{ type: "submitted", attempt: 2, job: "mine" }], second)).toMatchObject({ kind: "running", job: "mine" });
    expect(run([{ type: "failed-to-submit", attempt: 1, message: "x" }], second)).toMatchObject({ kind: "running", attempt: 2 });
  });

  it("names a song the server may have by its hash, and sends the file when the server hasn't got it", () => {
    const ready = run([{ type: "choose", song: 1, name: "a.mp3" }, { type: "set-bpm", bpm: 120 }]);
    expect(run([{ type: "transcribe", attempt: 1 }], ready)).toMatchObject({ kind: "running", sending: "upload", hash: null });
    const hashed = run([{ type: "hashed", song: 1, sha256: "a".repeat(64) }], ready);
    const going = run([{ type: "transcribe", attempt: 1 }], hashed);
    expect(going).toMatchObject({ kind: "running", sending: "hash", hash: "a".repeat(64) });
    expect(run([{ type: "submitted", attempt: 1, job: "j1" }], going)).toMatchObject({ job: "j1", sending: "hash" });
    const refused = run([{ type: "hash-refused", attempt: 1 }], going);
    expect(refused).toMatchObject({ kind: "running", sending: "upload", job: null });
    expect(run([{ type: "hash-refused", attempt: 1 }], refused)).toBe(refused);
    expect(run([{ type: "hash-refused", attempt: 0 }], going)).toBe(going);
  });

  it("never takes another song's hash", () => {
    const s = run([{ type: "choose", song: 1, name: "a" }, { type: "choose", song: 2, name: "b" }, { type: "hashed", song: 1, sha256: "1".repeat(64) }]);
    expect(s).toMatchObject({ song: 2, hash: null });
    const two = run([{ type: "hashed", song: 2, sha256: "2".repeat(64) }, { type: "set-bpm", bpm: 90 }, { type: "transcribe", attempt: 1 }], s);
    expect(two).toMatchObject({ song: 2, sending: "hash", hash: "2".repeat(64) });
    expect(run([{ type: "cancel" }, { type: "choose", song: 3, name: "c" }], two)).toMatchObject({ song: 3, hash: null, file: true });
  });

  it("re-transcribes the editor's song by its hash, and without the file says how to send it again", () => {
    const loaded: StartState = { kind: "loaded" };
    const again = run([{ type: "retranscribe", attempt: 1, song: 7, name: "a.mp3", bpm: 61.5, hash: "c".repeat(64), file: false }], loaded);
    expect(again).toMatchObject({ kind: "running", song: 7, bpm: 61.5, sending: "hash", hash: "c".repeat(64), detected: "unasked", job: null });
    const gone = run([{ type: "hash-refused", attempt: 1 }], again);
    expect(gone).toMatchObject({ kind: "ready", song: 7, bpm: 61.5, file: false });
    expect((gone as { error: string }).error).toMatch(/drop it here again/);
    expect(run([{ type: "transcribe", attempt: 2 }], gone)).toMatchObject({ kind: "running", sending: "hash" });
    const withFile = run([{ type: "retranscribe", attempt: 1, song: 7, name: "a.mp3", bpm: 61.5, hash: "c".repeat(64), file: true }, { type: "hash-refused", attempt: 1 }], loaded);
    expect(withFile).toMatchObject({ kind: "running", sending: "upload" });
    expect(run([{ type: "retranscribe", attempt: 1, song: 7, name: "a", bpm: 900, hash: "c".repeat(64), file: false }], loaded)).toBe(loaded);
    const busy = run([{ type: "choose", song: 1, name: "a" }, { type: "set-bpm", bpm: 90 }, { type: "transcribe", attempt: 1 }]);
    expect(run([{ type: "retranscribe", attempt: 2, song: 7, name: "a", bpm: 60, hash: "c".repeat(64), file: false }], busy)).toBe(busy);
    const noWay: StartState = { kind: "ready", song: 7, name: "a", bpm: 90, detected: "unasked", hash: null, file: false, error: null };
    expect(run([{ type: "transcribe", attempt: 1 }], noWay)).toMatchObject({ kind: "ready", error: expect.stringMatching(/drop it here again/) });
  });

  it("stays put without sona serve, and recovers when it appears", () => {
    const s = run([{ type: "serve-missing", reason: "no server" }, { type: "choose", song: 1, name: "a" }]);
    expect(s).toEqual({ kind: "unavailable", reason: "no server" });
    expect(run([{ type: "serve-found" }], s)).toEqual({ kind: "idle", error: null });
  });

  it("keeps its promises whatever the order of events", () => {
    const estimateOf = (song: number) => 200 + song;
    const messageOf = (song: number) => `song ${song} couldn't be read`;
    const hashOf = (song: number) => String(song).repeat(64);
    const events: StartEvent[] = [
      { type: "choose", song: 1, name: "a" },
      { type: "choose", song: 2, name: "b" },
      { type: "tempo", song: 1, bpm: estimateOf(1) },
      { type: "tempo", song: 2, bpm: estimateOf(2) },
      { type: "tempo", song: 2, bpm: null },
      { type: "tempo-failed", song: 1, message: messageOf(1) },
      { type: "tempo-failed", song: 2, message: messageOf(2) },
      { type: "set-bpm", bpm: 0 },
      { type: "set-bpm", bpm: 140 },
      { type: "set-bpm", bpm: 97.25 },
      { type: "set-bpm", bpm: null },
      { type: "use-hint", multiple: 1 },
      { type: "use-hint", multiple: 0.5 },
      { type: "transcribe", attempt: 1 },
      { type: "transcribe", attempt: 2 },
      { type: "submitted", attempt: 1, job: "j" },
      { type: "submitted", attempt: 2, job: "k" },
      { type: "progress", job: "j", phase: "transcribing", elapsedS: 1, log: [], error: null },
      { type: "progress", job: "k", phase: "failed", elapsedS: 1, log: [], error: "x" },
      { type: "cancel" },
      { type: "start-over" },
      { type: "hashed", song: 1, sha256: hashOf(1) },
      { type: "hashed", song: 2, sha256: hashOf(2) },
      { type: "retranscribe", attempt: 3, song: 3, name: "c", bpm: 150, hash: hashOf(3), file: false },
      { type: "retranscribe", attempt: 4, song: 1, name: "a", bpm: 75, hash: hashOf(1), file: true },
      { type: "retranscribe", attempt: 5, song: 3, name: "c", bpm: 0, hash: hashOf(3), file: false },
      { type: "hash-refused", attempt: 1 },
      { type: "hash-refused", attempt: 3 },
      { type: "hash-refused", attempt: 4 },
      { type: "submitted", attempt: 3, job: "r" },
    ];
    let seed = 3;
    const rnd = () => (seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31;
    for (let trial = 0; trial < 3000; trial++) {
      let s: StartState = { kind: "idle", error: null };
      for (let k = 0; k < 14; k++) {
        const prev = s;
        const e = events[Math.floor(rnd() * events.length)];
        s = startStep(s, e);
        if (s.kind === "running") expect(usableBpm(s.bpm)).toBe(true);
        if (s.kind === "running" && prev.kind === "running") expect(s.song).toBe(prev.song);
        if ((s.kind === "ready" || s.kind === "running") && typeof s.detected === "number") expect(s.detected).toBe(estimateOf(s.song));
        if ((s.kind === "ready" || s.kind === "running") && typeof s.detected === "object" && s.detected !== null) expect(s.detected.error).toBe(messageOf(s.song));
        if ((e.type === "tempo" || e.type === "tempo-failed") && (s.kind === "ready" || s.kind === "running")) expect(s.bpm).toBe((prev as { bpm: number | null }).bpm);
        if (e.type === "choose" && s.kind === "ready") expect([s.bpm, s.hash, s.file]).toEqual([null, null, true]);
        if ((s.kind === "ready" || s.kind === "running") && s.hash !== null) expect(s.hash).toBe(hashOf(s.song));
        if (s.kind === "running" && s.sending === "hash") expect(s.hash).toBe(hashOf(s.song));
        if (s.kind === "running" && s.sending === "upload") expect(s.file).toBe(true);
        if (e.type === "retranscribe" && s.kind === "running" && prev.kind !== "running") expect([s.song, s.sending, s.detected]).toEqual([e.song, "hash", "unasked"]);
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
