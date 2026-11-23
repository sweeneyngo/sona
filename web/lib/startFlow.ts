import type { Phase } from "./serveClient";

export const MIN_BPM = 20;
export const MAX_BPM = 400;

export type StartState =
  | { kind: "unavailable"; reason: string }
  | { kind: "idle"; error: string | null }
  | { kind: "detecting"; song: number; name: string }
  | { kind: "ready"; song: number; name: string; bpm: number | null; detected: number | null; error: string | null }
  | { kind: "running"; song: number; attempt: number; name: string; bpm: number; job: string | null; phase: Phase; elapsedS: number; log: string[] }
  | { kind: "loaded" };

export type StartEvent =
  | { type: "serve-missing"; reason: string }
  | { type: "serve-found" }
  | { type: "choose"; song: number; name: string }
  | { type: "tempo"; song: number; bpm: number | null }
  | { type: "set-bpm"; bpm: number | null }
  | { type: "halve" }
  | { type: "double" }
  | { type: "transcribe"; attempt: number }
  | { type: "submitted"; attempt: number; job: string }
  | { type: "progress"; job: string; phase: Phase; elapsedS: number; log: string[]; error: string | null }
  | { type: "failed-to-submit"; attempt: number; message: string }
  | { type: "cancel" }
  | { type: "results-loaded"; job: string }
  | { type: "start-over" };

export const usableBpm = (bpm: number | null): bpm is number => bpm !== null && Number.isFinite(bpm) && bpm >= MIN_BPM && bpm <= MAX_BPM;

const round1 = (v: number) => Math.round(v * 10) / 10;

export function startStep(s: StartState, e: StartEvent): StartState {
  switch (e.type) {
    case "serve-missing":
      return s.kind === "idle" ? { kind: "unavailable", reason: e.reason } : s;
    case "serve-found":
      return s.kind === "unavailable" ? { kind: "idle", error: null } : s;
    case "choose":
      if (s.kind === "running" || s.kind === "unavailable") return s;
      return { kind: "detecting", song: e.song, name: e.name };
    case "tempo":
      if (s.kind !== "detecting" || s.song !== e.song) return s;
      return { kind: "ready", song: s.song, name: s.name, bpm: e.bpm === null ? null : round1(e.bpm), detected: e.bpm === null ? null : round1(e.bpm), error: null };
    case "set-bpm":
      return s.kind === "ready" ? { ...s, bpm: e.bpm, error: null } : s;
    case "halve":
    case "double":
      if (s.kind !== "ready" || !usableBpm(s.bpm)) return s;
      return { ...s, bpm: round1(e.type === "halve" ? s.bpm / 2 : s.bpm * 2), error: null };
    case "transcribe":
      if (s.kind !== "ready") return s;
      if (!usableBpm(s.bpm)) return { ...s, error: `Set the song's tempo (${MIN_BPM}–${MAX_BPM} BPM): it fixes the grid the notes snap to.` };
      return { kind: "running", song: s.song, attempt: e.attempt, name: s.name, bpm: s.bpm, job: null, phase: "queued", elapsedS: 0, log: [] };
    case "submitted":
      return s.kind === "running" && s.attempt === e.attempt && s.job === null ? { ...s, job: e.job } : s;
    case "failed-to-submit":
      if (s.kind !== "running" || s.attempt !== e.attempt || s.job !== null) return s;
      return { kind: "ready", song: s.song, name: s.name, bpm: s.bpm, detected: null, error: e.message };
    case "progress": {
      if (s.kind !== "running" || s.job !== e.job) return s;
      if (e.phase === "failed" || e.phase === "cancelled") {
        const why = e.phase === "cancelled" ? "Cancelled." : `It failed: ${e.error ?? "see the log"}.`;
        return { kind: "ready", song: s.song, name: s.name, bpm: s.bpm, detected: null, error: why };
      }
      return { ...s, phase: e.phase, elapsedS: e.elapsedS, log: e.log };
    }
    case "cancel":
      if (s.kind === "running") return { kind: "ready", song: s.song, name: s.name, bpm: s.bpm, detected: null, error: "Cancelled." };
      return s;
    case "results-loaded":
      return s.kind === "running" && s.job === e.job ? { kind: "loaded" } : s;
    case "start-over":
      return s.kind === "running" || s.kind === "unavailable" ? s : { kind: "idle", error: null };
  }
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
