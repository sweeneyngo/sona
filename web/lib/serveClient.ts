import type { JobDivision } from "./grid";

export type Separation = "none" | "vocals" | "lead";
export type Engine = "pyin" | "game" | "cells";
export type Phase = "queued" | "decoding" | "separating-vocals" | "separating-lead" | "finding-bar-one" | "transcribing" | "done" | "failed" | "cancelled";
/** Where a job takes bar 1 from in the music: the harmony (ADR-0031) or Beat This's downbeats (ADR-0036). */
export type BarOneSource = "harmonic" | "beat-this";
/** The start screen's bar-1 choice: today's rule (the first grid line), or a source in the music. */
export type BarOneChoice = "off" | BarOneSource;

/** Something separation still needs: `missing` turns options off, `first-use` only means a download,
 * `optional` is an opt-in tool that isn't installed (only its own choice is off). */
export interface SetupNote {
  piece: string;
  status: "missing" | "first-use" | "optional";
  message: string;
}

export interface Capabilities {
  separation: Record<Separation, boolean>;
  engines: Record<Engine, boolean>;
  tempo: boolean;
  barOne: Record<BarOneSource, boolean>;
  setup: SetupNote[];
}

export interface JobStatus {
  id: string;
  name: string;
  phase: Phase;
  error: string | null;
  elapsedS: number;
  log: string[];
}

export interface JobRequest {
  bpm: number;
  grid: JobDivision;
  separation: Separation;
  engine: Engine;
  name: string;
  /** Also estimate bar 1 from the music, and from what (opt-in; needs separation). */
  barOne?: BarOneSource;
  /** The song's lyric, one sung line per line (lyrics-first 03); the server keeps it with the job. */
  lyric?: string;
}

export class ServeError extends Error {
  constructor(
    message: string,
    readonly status: number | null = null,
  ) {
    super(message);
  }
}

const PHASES: readonly Phase[] = ["queued", "decoding", "separating-vocals", "separating-lead", "finding-bar-one", "transcribing", "done", "failed", "cancelled"];
const FINAL: readonly Phase[] = ["done", "failed", "cancelled"];

export const isFinal = (p: Phase): boolean => FINAL.includes(p);

export const PHASE_LABEL: Record<Phase, string> = {
  queued: "Waiting",
  decoding: "Reading audio",
  "separating-vocals": "Separating vocals",
  "separating-lead": "Isolating the lead",
  "finding-bar-one": "Finding bar 1 (Beat This)",
  transcribing: "Writing notes",
  done: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

function record(v: unknown): Record<string, unknown> {
  if (typeof v !== "object" || v === null || Array.isArray(v)) throw new ServeError("unexpected response from sona serve");
  return v as Record<string, unknown>;
}

function bools<K extends string>(v: unknown, keys: readonly K[]): Record<K, boolean> {
  const o = record(v);
  return Object.fromEntries(keys.map((k) => [k, o[k] === true])) as Record<K, boolean>;
}

function parseSetupNote(v: unknown): SetupNote[] {
  if (typeof v !== "object" || v === null) return [];
  const o = v as Record<string, unknown>;
  if (typeof o.piece !== "string" || typeof o.message !== "string" || (o.status !== "missing" && o.status !== "first-use" && o.status !== "optional")) return [];
  return [{ piece: o.piece, status: o.status, message: o.message }];
}

/** A server from before ADR-0036 reports no `bar_one`: it offers the harmony only. */
function parseBarOneSources(v: unknown): Record<BarOneSource, boolean> {
  return v === undefined ? { harmonic: true, "beat-this": false } : bools(v, ["harmonic", "beat-this"] as const);
}

export function parseCapabilities(v: unknown): Capabilities {
  const o = record(v);
  return {
    separation: bools(o.separation, ["none", "vocals", "lead"] as const),
    engines: bools(o.engines, ["pyin", "game", "cells"] as const),
    tempo: o.tempo === true,
    barOne: parseBarOneSources(o.bar_one),
    setup: Array.isArray(o.setup) ? o.setup.flatMap(parseSetupNote) : [],
  };
}

/** The bar-1 choices the start screen offers: today's rule always, and each source this server can run. */
export function barOneChoices(caps: Capabilities): BarOneChoice[] {
  return ["off", ...(["harmonic", "beat-this"] as const).filter((s) => caps.barOne[s])];
}

/** The setup notes the start screen lists: what separation needs. An optional tool's note shows beside its own choice. */
export function setupList(caps: Capabilities): SetupNote[] {
  return caps.setup.filter((n) => n.status !== "optional");
}

/** Why Beat This isn't offered and how to install it, when the server says so. */
export function beatThisNote(caps: Capabilities): string | null {
  return caps.barOne["beat-this"] ? null : (caps.setup.find((n) => n.piece === "beat-this")?.message ?? null);
}

export function parseJobStatus(v: unknown): JobStatus {
  const o = record(v);
  const phase = PHASES.find((p) => p === o.phase);
  if (typeof o.id !== "string" || !phase) throw new ServeError("unexpected job status from sona serve");
  return {
    id: o.id,
    name: typeof o.name === "string" ? o.name : "",
    phase,
    error: typeof o.error === "string" ? o.error : null,
    elapsedS: typeof o.elapsed_s === "number" ? o.elapsed_s : 0,
    log: Array.isArray(o.log) ? o.log.filter((l): l is string => typeof l === "string") : [],
  };
}

/** The job's query; `upload` names the song by its SHA-256 instead of sending it (ADR-0032). */
export function jobQuery(r: JobRequest, upload?: string): string {
  const q = new URLSearchParams({ bpm: String(r.bpm), grid: String(r.grid), separation: r.separation, engine: r.engine, name: r.name });
  if (r.barOne) q.set("bar_one", r.barOne === "harmonic" ? "1" : r.barOne);
  if (r.lyric !== undefined) q.set("lyric", r.lyric);
  if (upload !== undefined) q.set("upload", upload);
  return `/api/jobs?${q.toString()}`;
}

async function call(path: string, init?: RequestInit): Promise<unknown> {
  let res: Response;
  try {
    res = await fetch(path, init);
  } catch {
    throw new ServeError("sona serve isn't reachable. Start it with `sona serve` and open the address it prints.");
  }
  const body: unknown = await res.json().catch(() => null);
  if (!res.ok) {
    const msg = body && typeof body === "object" && "error" in body && typeof body.error === "string" ? body.error : `HTTP ${res.status}`;
    throw new ServeError(msg, res.status);
  }
  return body;
}

export const fetchCapabilities = async (): Promise<Capabilities> => parseCapabilities(await call("/api/capabilities"));
export const fetchJob = async (id: string): Promise<JobStatus> => parseJobStatus(await call(`/api/jobs/${encodeURIComponent(id)}`));
export const cancelJob = async (id: string): Promise<JobStatus> => parseJobStatus(await call(`/api/jobs/${encodeURIComponent(id)}`, { method: "DELETE" }));

/** The job's status for a watcher that gives up on any error: the job is then cancelled on the server
 * too (best effort), so a retry doesn't queue behind a job nobody is watching. */
export async function fetchJobOrCancel(id: string): Promise<JobStatus> {
  try {
    return await fetchJob(id);
  } catch (err) {
    await cancelJob(id).catch(() => undefined);
    throw err;
  }
}
export const submitJob = async (r: JobRequest, audio: Blob): Promise<JobStatus> => parseJobStatus(await call(jobQuery(r), { method: "POST", body: audio }));

/** A job that names its song by SHA-256 and sends no audio (ADR-0032). When the server hasn't got the
 * song's stems (404) or can't tell yet (409), its reason comes back as `refused`: send the file instead. */
export async function submitJobByHash(r: JobRequest, sha256: string): Promise<JobStatus | { refused: string }> {
  try {
    return parseJobStatus(await call(jobQuery(r, sha256), { method: "POST" }));
  } catch (err) {
    if (err instanceof ServeError && (err.status === 404 || err.status === 409)) return { refused: err.message };
    throw err;
  }
}

/** A SHA-256 as the server writes it: 64 lowercase hex digits. */
export const isSha256 = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{64}$/.test(v);

/** The file's SHA-256 as the server writes it, or null where the browser can't hash (no secure context). */
export async function songHash(file: Blob): Promise<string | null> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return null;
  try {
    const digest = new Uint8Array(await subtle.digest("SHA-256", await file.arrayBuffer()));
    return Array.from(digest, (b) => b.toString(16).padStart(2, "0")).join("");
  } catch {
    return null;
  }
}

export async function fetchTempo(audio: Blob, name: string): Promise<number | null> {
  const o = record(await call(`/api/tempo?${new URLSearchParams({ name }).toString()}`, { method: "POST", body: audio }));
  return typeof o.bpm === "number" && Number.isFinite(o.bpm) ? o.bpm : null;
}

export type TempoAnswer = { bpm: number | null } | { error: string };

/** The tempo estimate (null when no steady beat was found), or why there is none: the server's own
 * words for a file it couldn't read, or a server it couldn't reach. */
export async function estimateTempo(audio: Blob, name: string): Promise<TempoAnswer> {
  try {
    return { bpm: await fetchTempo(audio, name) };
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) };
  }
}

export function defaultJob(caps: Capabilities): { separation: Separation; engine: Engine } {
  const separation: Separation = caps.separation.lead ? "lead" : caps.separation.vocals ? "vocals" : "none";
  const engine: Engine = caps.engines.cells && separation === "lead" ? "cells" : caps.engines.game ? "game" : "pyin";
  return { separation, engine };
}

export async function fetchResults(id: string): Promise<{ annotation: unknown; stem: ArrayBuffer }> {
  const base = `/api/jobs/${encodeURIComponent(id)}`;
  const [a, s] = await Promise.all([fetch(`${base}/annotation`), fetch(`${base}/stem`)]);
  if (!a.ok || !s.ok) throw new ServeError("the job's results aren't available");
  try {
    return { annotation: await a.json(), stem: await s.arrayBuffer() };
  } catch {
    throw new ServeError("the job's results couldn't be read");
  }
}
