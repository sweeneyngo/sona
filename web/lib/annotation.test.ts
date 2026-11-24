import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { Annotation } from "./annotation";
import { normalizeAnnotation } from "./session";

/** Written by `full_annotation_matches_the_contract_fixture` in `crates/sona-core/src/annotation.rs`. */
const written = () => JSON.parse(readFileSync(new URL("../../crates/sona-core/tests/fixtures/annotation-full.json", import.meta.url), "utf8"));

const sha = (c: string) => c.repeat(64);

/** The same annotation, spelled with this side's types: a field either side renames breaks it. */
const expected: Annotation = {
  version: 1,
  sample_rate: 44100,
  bpm: 174,
  duration_s: 1.5,
  producer: "game",
  notes: [
    {
      onset_s: 0.25,
      offset_s: 0.5,
      midi_pitch: 69,
      confidence: 0.82,
      sub_scores: { pitch_stability: 0.95, voicing: 0.88, onset_clarity: 0.9, offset_clarity: 0.82 },
      stage_doubt: { length: 0.75, unsure: 0.5 },
    },
  ],
  f0_contour: [
    { time_s: 0.25, frequency_hz: 440, voicing: 0.9 },
    { time_s: 0.26, frequency_hz: 441.2, voicing: 0.88 },
  ],
  energy_envelope: [
    { time_s: 0.25, rms: 0.12 },
    { time_s: 0.26, rms: 0.14 },
  ],
  provenance: {
    sona_version: "0.1.0",
    sona_commit: "v0.1.0-12-gabcdef0",
    created_unix_s: 1764000000,
    input: { role: "input", name: "lead.wav", bytes: 88244, sha256: sha("a") },
    settings: { bpm: "174", engine: "game+cells", grid: "24" },
    models: [{ role: "game/encoder", name: "encoder.onnx", bytes: 1024, sha256: sha("b") }],
    job: {
      id: "j1",
      upload: { role: "upload", name: "song.mp3", bytes: 4096, sha256: sha("c") },
      steps: [{ step: "transcribing", seconds: 1.5 }],
      tools: { demucs: "4.0.1" },
      models: [{ role: "separator/lead", name: "UVR.onnx", bytes: 2048, sha256: sha("d") }],
      separation: "lead",
      stems: { key: sha("e"), reused: true, separated_by: "j0", by_hash: true },
    },
  },
  bar_one: { time_s: 0.42, confidence: 0.75, harmonic: true },
};

describe("the annotation contract", () => {
  it("reads every field the Rust core writes, optional ones included", () => {
    expect(normalizeAnnotation(written())).toEqual(expected);
  });
});
