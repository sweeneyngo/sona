import type { Phase } from "./serveClient";

export const MIN_BPM = 20;
export const MAX_BPM = 400;

/** The tempo estimate for the song on screen: still coming, none found, the estimate, or why it failed. */
export type Detected = number | null | "pending" | { error: string };

export type HintMultiple = 1 | 0.5;

export type StartState =
  | { kind: "unavailable"; reason: string }
  | { kind: "idle"; error: string | null }
  | { kind: "ready"; song: number; name: string; bpm: number | null; detected: Detected; error: string | null }
  | { kind: "running"; song: number; attempt: number; name: string; bpm: number; detected: Detected; job: string | null; phase: Phase; elapsedS: number; log: string[] }
  | { kind: "loaded" };

export type StartEvent =
  | { type: "serve-missing"; reason: string }
  | { type: "serve-found" }
  | { type: "choose"; song: number; name: string }
  | { type: "tempo"; song: number; bpm: number | null }
  | { type: "tempo-failed"; song: number; message: string }
  | { type: "set-bpm"; bpm: number | null }
  | { type: "use-hint"; multiple: HintMultiple }
  | { type: "transcribe"; attempt: number }
  | { type: "submitted"; attempt: number; job: string }
  | { type: "progress"; job: string; phase: Phase; elapsedS: number; log: string[]; error: string | null }
  | { type: "failed-to-submit"; attempt: number; message: string }
  | { type: "cancel" }
  | { type: "results-loaded"; job: string }
  | { type: "start-over" };

export const usableBpm = (bpm: number | null): bpm is number => bpm !== null && Number.isFinite(bpm) && bpm >= MIN_BPM && bpm <= MAX_BPM;

/** The BPM a person typed, with a decimal point or a decimal comma; null when it isn't a plain number. */
export function parseBpm(text: string): number | null {
  const t = text.trim();
  if (!/^(\d+([.,]\d*)?|[.,]\d+)$/.test(t)) return null;
  return Number(t.replace(",", "."));
}

/** A tempo hint as offered: the estimate times `multiple`, to the whole BPM (the estimate is good to about 2%). */
export const hintBpm = (detected: number, multiple: HintMultiple): number => Math.round(detected * multiple);

const round1 = (v: number) => Math.round(v * 10) / 10;

export function startStep(s: StartState, e: StartEvent): StartState {
  switch (e.type) {
    case "serve-missing":
      return s.kind === "idle" ? { kind: "unavailable", reason: e.reason } : s;
    case "serve-found":
      return s.kind === "unavailable" ? { kind: "idle", error: null } : s;
    case "choose":
      if (s.kind === "running" || s.kind === "unavailable") return s;
      return { kind: "ready", song: e.song, name: e.name, bpm: null, detected: "pending", error: null };
    case "tempo":
    case "tempo-failed":
      if ((s.kind !== "ready" && s.kind !== "running") || s.song !== e.song || s.detected !== "pending") return s;
      return { ...s, detected: e.type === "tempo-failed" ? { error: e.message } : e.bpm === null ? null : round1(e.bpm) };
    case "set-bpm":
      return s.kind === "ready" ? { ...s, bpm: e.bpm, error: null } : s;
    case "use-hint":
      if (s.kind !== "ready" || typeof s.detected !== "number") return s;
      return { ...s, bpm: hintBpm(s.detected, e.multiple), error: null };
    case "transcribe":
      if (s.kind !== "ready") return s;
      if (!usableBpm(s.bpm)) return { ...s, error: `Enter the song's BPM (${MIN_BPM}–${MAX_BPM}): it sets the grid the notes snap to.` };
      return { kind: "running", song: s.song, attempt: e.attempt, name: s.name, bpm: s.bpm, detected: s.detected, job: null, phase: "queued", elapsedS: 0, log: [] };
    case "submitted":
      return s.kind === "running" && s.attempt === e.attempt && s.job === null ? { ...s, job: e.job } : s;
    case "failed-to-submit":
      if (s.kind !== "running" || s.attempt !== e.attempt || s.job !== null) return s;
      return backToSong(s, e.message);
    case "progress":
      if (s.kind !== "running" || s.job !== e.job) return s;
      if (e.phase === "failed" || e.phase === "cancelled") return backToSong(s, e.phase === "cancelled" ? "Cancelled." : `It failed: ${e.error ?? "see the log"}.`);
      return { ...s, phase: e.phase, elapsedS: e.elapsedS, log: e.log };
    case "cancel":
      return s.kind === "running" ? backToSong(s, "Cancelled.") : s;
    case "results-loaded":
      return s.kind === "running" && s.job === e.job ? { kind: "loaded" } : s;
    case "start-over":
      return s.kind === "running" || s.kind === "unavailable" ? s : { kind: "idle", error: null };
  }
}

function backToSong(s: Extract<StartState, { kind: "running" }>, error: string): StartState {
  return { kind: "ready", song: s.song, name: s.name, bpm: s.bpm, detected: s.detected, error };
}

export function phaseProgress(phase: Phase, separation: "none" | "vocals" | "lead"): number {
  const steps: Phase[] =
    separation === "lead"
      ? ["queued", "separating-vocals", "separating-lead", "transcribing", "done"]
      : separation === "vocals"
        ? ["queued", "separating-vocals", "transcribing", "done"]
        : ["queued", "decoding", "transcribing", "done"];
  const i = steps.indexOf(phase);
  return i < 0 ? 0 : i / (steps.length - 1);
}
