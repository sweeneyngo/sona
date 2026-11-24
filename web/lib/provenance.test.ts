import { describe, expect, it } from "vitest";
import { detailSections, parseProvenance } from "./provenance";

const sha = (c: string) => c.repeat(64);
const fromServe = {
  sona_version: "0.1.0",
  sona_commit: "38ff41e01234-dirty",
  created_unix_s: 1795400000,
  input: { role: "input", name: "stem.wav", bytes: 4_400_000, sha256: sha("a") },
  settings: { bpm: "123", engine: "game", grid: "16", game_segmenter_seed: "1" },
  models: [{ role: "game/segmenter", name: "segmenter.onnx", bytes: 84_000_000, sha256: sha("b") }],
  job: {
    id: "j1",
    upload: { role: "upload", name: "song.mp3", bytes: 600_000, sha256: sha("c") },
    steps: [
      { step: "queued", seconds: 0.01 },
      { step: "separating-vocals", seconds: 40.5 },
      { step: "transcribing", seconds: 20.2 },
    ],
    tools: { demucs: "4.0.1", ffmpeg: "7.0.2", odd: 3 },
    models: [{ role: "separator/lead", name: "UVR_MDXNET_KARA_2.onnx", bytes: 52_000_000, sha256: sha("d") }],
    stems: { key: sha("e"), reused: true, separated_by: "j0" },
  },
};

describe("provenance", () => {
  it("reads what sona serve writes, keeping only well-formed parts", () => {
    const p = parseProvenance(fromServe)!;
    expect(p.job?.tools).toEqual({ demucs: "4.0.1", ffmpeg: "7.0.2" });
    expect(p.models[0].sha256).toBe(sha("b"));
    expect(parseProvenance({ ...fromServe, job: { upload: "nope" } })!.job).toBeUndefined();
  });

  it("is absent for older annotations and anything malformed", () => {
    expect(parseProvenance(undefined)).toBeNull();
    expect(parseProvenance({ sona_version: "0.1.0" })).toBeNull();
    expect(parseProvenance({ sona_version: 1, input: fromServe.input })).toBeNull();
  });

  it("lays out the Details view with short hashes, step times and a total", () => {
    const s = Object.fromEntries(detailSections(parseProvenance(fromServe)!).map((x) => [x.title, Object.fromEntries(x.rows)]));
    expect(s["Made by"].sona).toBe("0.1.0 (38ff41e01234-dirty)");
    expect(s.Input.input).toBe("stem.wav · 4.2 MB · aaaaaaaaaaaa");
    expect(s.Input.upload).toBe("song.mp3 · 586 kB · cccccccccccc");
    expect(Object.keys(s.Models)).toEqual(["game/segmenter", "separator/lead"]);
    expect(s.Steps.total).toBe("60.7 s");
    expect(s.Settings.engine).toBe("game");
    expect(s.Stems).toEqual({ separated: "reused from job j0", "cache key": "eeeeeeeeeeee" });
  });

  it("keeps where the stems came from through a session, and drops a malformed record", () => {
    const p = parseProvenance(fromServe)!;
    expect(parseProvenance(JSON.parse(JSON.stringify(p)))).toEqual(p);
    expect(p.job?.stems).toEqual({ key: sha("e"), reused: true, separated_by: "j0" });
    const odd = { ...fromServe, job: { ...fromServe.job, stems: { key: sha("e"), reused: "yes" } } };
    expect(parseProvenance(odd)!.job?.stems).toBeUndefined();
    const { stems: _stems, ...noStems } = fromServe.job;
    expect(detailSections(parseProvenance({ ...fromServe, job: noStems })!).map((x) => x.title)).not.toContain("Stems");
  });

  it("keeps the job's separation and a job named by hash, and says so in Details", () => {
    const byHash = { ...fromServe, job: { ...fromServe.job, separation: "lead", stems: { ...fromServe.job.stems, by_hash: true } } };
    const p = parseProvenance(byHash)!;
    expect([p.job?.separation, p.job?.stems?.by_hash]).toEqual(["lead", true]);
    expect(parseProvenance(JSON.parse(JSON.stringify(p)))).toEqual(p);
    const stems = Object.fromEntries(detailSections(p).find((x) => x.title === "Stems")!.rows);
    expect(stems.upload).toBe("named by hash, not sent again");
    const odd = parseProvenance({ ...fromServe, job: { ...fromServe.job, separation: "karaoke", stems: { ...fromServe.job.stems, by_hash: "yes" } } })!;
    expect([odd.job?.separation, odd.job?.stems?.by_hash]).toEqual([undefined, undefined]);
  });

  it("leaves out job sections for a CLI annotation", () => {
    const { job: _job, ...cli } = fromServe;
    expect(detailSections(parseProvenance(cli)!).map((x) => x.title)).toEqual(["Made by", "Input", "Settings", "Models"]);
  });
});
