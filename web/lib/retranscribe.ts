/**
 * Transcribing the editor's song again at a new tempo or grid (ADR-0032): what the notes came from,
 * and the job that redoes them. The job names the song by its upload's SHA-256, so sona serve reuses
 * the separated stems without the file being sent again.
 */

import type { Annotation } from "./annotation";
import { DEFAULT_DIVISION, JOB_GRIDS, type JobDivision } from "./grid";
import { isSha256, type BarOneSource, type Capabilities, type Engine, type JobRequest, type Separation } from "./serveClient";
import { usableBpm } from "./startFlow";

/** The song and settings a sona serve job transcribed the notes from. */
export interface Transcribed {
  hash: string;
  name: string;
  bpm: number;
  grid: JobDivision | null;
  separation: Separation | null;
  engine: Engine | null;
  /** Where the job took bar 1 from in the music; null when it didn't. */
  barOne: BarOneSource | null;
  /** The lyric the job was sent, which the next job sends again; null for notes transcribed without one. */
  lyric: string | null;
}

const jobGrid = (division: number): JobDivision | null => JOB_GRIDS.find((g) => g.division === division)?.division ?? null;

/** Where the annotation's notes came from, or null when they didn't come from a sona serve job. */
export function transcribedFrom(a: Annotation | null): Transcribed | null {
  const p = a?.provenance;
  const job = p?.job;
  if (!a || !p || !job || !isSha256(job.upload.sha256)) return null;
  const engine = p.settings.engine === "game+cells" ? "cells" : p.settings.engine === "game" || p.settings.engine === "pyin" ? p.settings.engine : null;
  return {
    hash: job.upload.sha256,
    name: job.upload.name,
    bpm: a.bpm,
    grid: jobGrid(Number(p.settings.grid)),
    separation: job.separation ?? null,
    engine,
    barOne: p.settings.bar_one !== "mix" ? null : p.settings.bar_one_source === "beat-this" ? "beat-this" : "harmonic",
    lyric: job.lyric ?? null,
  };
}

/**
 * The job that transcribes `t` again at the editor's `bpm` and grid, or null when neither differs from
 * the transcription's (or the BPM isn't usable). A grid no job can use keeps the transcription's.
 * The separation, engine and bar 1 are the original job's where the server still offers them (bar 1 from
 * Beat This falls back to the harmony where Beat This isn't installed); the lyric is the original's.
 */
export function retranscribeJob(t: Transcribed, bpm: number, division: number, caps: Capabilities): JobRequest | null {
  const grid = jobGrid(division) ?? t.grid ?? DEFAULT_DIVISION;
  const changed = Math.abs(bpm - t.bpm) >= 0.05 || (t.grid !== null && grid !== t.grid);
  if (!usableBpm(bpm) || !changed) return null;
  const separation: Separation = t.separation && caps.separation[t.separation] ? t.separation : caps.separation.lead ? "lead" : caps.separation.vocals ? "vocals" : "none";
  const engine: Engine = t.engine && caps.engines[t.engine] && (t.engine !== "cells" || separation === "lead") ? t.engine : caps.engines.game ? "game" : "pyin";
  const barOne: BarOneSource | null = t.barOne === null || separation === "none" ? null : caps.barOne[t.barOne] ? t.barOne : caps.barOne.harmonic ? "harmonic" : null;
  return { bpm, grid, separation, engine, name: t.name, ...(barOne !== null ? { barOne } : {}), ...(t.lyric !== null ? { lyric: t.lyric } : {}) };
}

/** The offer's label: "Re-transcribe at 123.5 BPM". */
export const retranscribeLabel = (bpm: number): string => `Re-transcribe at ${Math.round(bpm * 10) / 10} BPM`;
