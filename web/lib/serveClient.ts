export type Separation = "none" | "vocals" | "lead";
export type Engine = "pyin" | "game";
export type Phase = "queued" | "decoding" | "separating-vocals" | "separating-lead" | "transcribing" | "done" | "failed" | "cancelled";

export interface Capabilities {
  separation: Record<Separation, boolean>;
  engines: Record<Engine, boolean>;
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
}

export class ServeError extends Error {}

const PHASES: readonly Phase[] = ["queued", "decoding", "separating-vocals", "separating-lead", "transcribing", "done", "failed", "cancelled"];
const FINAL: readonly Phase[] = ["done", "failed", "cancelled"];

export const isFinal = (p: Phase): boolean => FINAL.includes(p);

export const PHASE_LABEL: Record<Phase, string> = {
  queued: "Waiting for the previous job",
  decoding: "Reading the audio",
  "separating-vocals": "Separating the vocals (demucs)",
  "separating-lead": "Separating the lead vocal (UVR karaoke)",
  transcribing: "Transcribing notes",
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

export function parseCapabilities(v: unknown): Capabilities {
  const o = record(v);
  return { separation: bools(o.separation, ["none", "vocals", "lead"] as const), engines: bools(o.engines, ["pyin", "game"] as const) };
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
export const submitJob = async (r: JobRequest, audio: Blob): Promise<JobStatus> => parseJobStatus(await call(jobQuery(r), { method: "POST", body: audio }));

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
