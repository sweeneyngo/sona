import { describe, expect, it } from "vitest";
import type { Annotation } from "./annotation";
import { retranscribeJob, retranscribeLabel, transcribedFrom, type Transcribed } from "./retranscribe";
import type { Capabilities } from "./serveClient";

const sha = (c: string) => c.repeat(64);

function fromServe(settings: Record<string, string>, job: Record<string, unknown> = {}): Annotation {
  return {
    version: 1,
    sample_rate: 44100,
    bpm: Number(settings.bpm),
    duration_s: 10,
    producer: "pyin",
    notes: [],
    f0_contour: [],
    energy_envelope: [],
    provenance: {
      sona_version: "0.1.0",
      sona_commit: null,
      created_unix_s: 0,
      input: { role: "input", name: "stem.wav", bytes: 1, sha256: sha("a") },
      settings,
      models: [],
      job: { id: "j1", upload: { role: "upload", name: "song.mp3", bytes: 9, sha256: sha("c") }, steps: [], tools: {}, models: [], ...job },
    },
  };
}

const caps = (over: Partial<Capabilities> = {}): Capabilities => ({
  separation: { none: true, vocals: true, lead: true },
  engines: { pyin: true, game: true, cells: true },
  tempo: true,
  setup: [],
  ...over,
});

describe("re-transcribing the editor's song", () => {
  it("reads the song's hash and the job's settings from the notes' provenance", () => {
    const t = transcribedFrom(fromServe({ bpm: "246", grid: "16", engine: "game+cells", bar_one: "mix" }, { separation: "lead" }));
    expect(t).toEqual({ hash: sha("c"), name: "song.mp3", bpm: 246, grid: 16, separation: "lead", engine: "cells", barOne: true, lyric: null });
    expect(transcribedFrom(fromServe({ bpm: "120", grid: "none", engine: "pyin" }))).toMatchObject({ grid: null, separation: null, engine: "pyin", barOne: false });
  });

  it("has nothing to offer for notes that didn't come from sona serve, or an upload hash it can't trust", () => {
    expect(transcribedFrom(null)).toBeNull();
    const cli = fromServe({ bpm: "120" });
    delete cli.provenance!.job;
    expect(transcribedFrom(cli)).toBeNull();
    const { provenance: _p, ...bare } = cli;
    expect(transcribedFrom(bare)).toBeNull();
    const odd = fromServe({ bpm: "120" });
    odd.provenance!.job!.upload.sha256 = "C".repeat(64);
    expect(transcribedFrom(odd)).toBeNull();
  });

  it("offers a job only once the tempo or the job grid differs from the transcription's", () => {
    const t: Transcribed = { hash: sha("c"), name: "song.mp3", bpm: 246, grid: 16, separation: "lead", engine: "cells", barOne: true, lyric: null };
    expect(retranscribeJob(t, 246, 16, caps())).toBeNull();
    expect(retranscribeJob(t, 246.02, 16, caps())).toBeNull();
    expect(retranscribeJob(t, 246, 8, caps())).toBeNull();
    expect(retranscribeJob(t, 123, 16, caps())).toEqual({ bpm: 123, grid: 16, separation: "lead", engine: "cells", name: "song.mp3", barOne: true });
    expect(retranscribeJob(t, 246, 24, caps())).toMatchObject({ bpm: 246, grid: 24 });
    expect(retranscribeJob(t, 123, 8, caps())).toMatchObject({ grid: 16 });
    expect(retranscribeJob(t, 900, 16, caps())).toBeNull();
    expect(retranscribeJob({ ...t, grid: null }, 246, 24, caps())).toBeNull();
  });

  it("keeps the original job's separation and engine while the server offers them", () => {
    const t: Transcribed = { hash: sha("c"), name: "a.wav", bpm: 120, grid: 16, separation: "lead", engine: "cells", barOne: false, lyric: null };
    const noCells = caps({ engines: { pyin: true, game: true, cells: false } });
    expect(retranscribeJob(t, 60, 16, noCells)).toMatchObject({ separation: "lead", engine: "game" });
    const noLead = caps({ separation: { none: true, vocals: true, lead: false } });
    expect(retranscribeJob(t, 60, 16, noLead)).toMatchObject({ separation: "vocals", engine: "game" });
    expect(retranscribeJob({ ...t, separation: null, engine: null }, 60, 16, caps())).toMatchObject({ separation: "lead", engine: "game" });
    expect(retranscribeJob({ ...t, separation: "none", barOne: true }, 60, 16, caps())).not.toHaveProperty("barOne");
  });

  it("keeps the lyric the notes were transcribed with, line breaks and all, for the next job", () => {
    const lyric = "あめが ふる\nかぜが ふく";
    const t = transcribedFrom(fromServe({ bpm: "120", grid: "16", engine: "game" }, { separation: "lead", lyric }));
    expect(t?.lyric).toBe(lyric);
    expect(retranscribeJob(t!, 60, 16, caps())).toMatchObject({ bpm: 60, lyric });
    expect(retranscribeJob({ ...t!, lyric: null }, 60, 16, caps())).not.toHaveProperty("lyric");
  });

  it("labels the offer with the tempo it will use", () => {
    expect(retranscribeLabel(123)).toBe("Re-transcribe at 123 BPM");
    expect(retranscribeLabel(61.25)).toBe("Re-transcribe at 61.3 BPM");
  });
});
