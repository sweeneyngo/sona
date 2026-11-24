export type Separation = "none" | "vocals" | "lead";
export type Engine = "pyin" | "game" | "cells";
export type Phase = "queued" | "decoding" | "separating-vocals" | "separating-lead" | "transcribing" | "done" | "failed" | "cancelled";

/** Something separation still needs: `missing` turns options off, `first-use` only means a download. */
export interface SetupNote {
  piece: string;
  status: "missing" | "first-use";
  message: string;
}

export interface Capabilities {
  separation: Record<Separation, boolean>;
  engines: Record<Engine, boolean>;
  tempo: boolean;
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
  grid: 16 | 24;
  separation: Separation;
  engine: Engine;
  name: string;
  /** Also estimate bar 1 from the music (opt-in; needs separation). */
  barOne?: boolean;
}

export class ServeError extends Error {}

const PHASES: readonly Phase[] = ["queued", "decoding", "separating-vocals", "separating-lead", "transcribing", "done", "failed", "cancelled"];
const FINAL: readonly Phase[] = ["done", "failed", "cancelled"];

export const isFinal = (p: Phase): boolean => FINAL.includes(p);

export const PHASE_LABEL: Record<Phase, string> = {
  queued: "Waiting",
  decoding: "Reading audio",
  "separating-vocals": "Separating vocals",
  "separating-lead": "Isolating the lead",
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
  if (typeof o.piece !== "string" || typeof o.message !== "string" || (o.status !== "missing" && o.status !== "first-use")) return [];
  return [{ piece: o.piece, status: o.status, message: o.message }];
}

export function parseCapabilities(v: unknown): Capabilities {
  const o = record(v);
  return {
    separation: bools(o.separation, ["none", "vocals", "lead"] as const),
    engines: bools(o.engines, ["pyin", "game", "cells"] as const),
    tempo: o.tempo === true,
    setup: Array.isArray(o.setup) ? o.setup.flatMap(parseSetupNote) : [],
  };
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

export function jobQuery(r: JobRequest): string {
  const q = new URLSearchParams({ bpm: String(r.bpm), grid: String(r.grid), separation: r.separation, engine: r.engine, name: r.name });
  if (r.barOne) q.set("bar_one", "1");
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
    throw new ServeError(msg);
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
