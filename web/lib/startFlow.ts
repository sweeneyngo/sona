import { checkLyric } from "./lyrics";
import type { Phase } from "./serveClient";

const MIN_BPM = 20;
const MAX_BPM = 400;

/** The tempo estimate for the song on screen: still coming, none found, the estimate, why it failed, or
 * not asked for (a re-transcription from the editor, whose BPM the user set there). */
export type Detected = number | null | "pending" | "unasked" | { error: string };

export type HintMultiple = 1 | 0.5;

/** How a job's audio reaches the server: named by the song's SHA-256, or sent (ADR-0032). */
export type Sending = "hash" | "upload";

/** `hash` is the song's SHA-256 once known; `file` says whether the app holds the song's file to send.
 * `lyric` is the song's lyric as typed; a running job's is the checked lyric it sends (lyrics-first 03). */
export type StartState =
  | { kind: "unavailable"; reason: string }
  | { kind: "idle"; error: string | null }
  | { kind: "ready"; song: number; name: string; bpm: number | null; lyric: string; detected: Detected; hash: string | null; file: boolean; error: string | null }
  | {
      kind: "running";
      song: number;
      attempt: number;
      name: string;
      bpm: number;
      lyric: string;
      detected: Detected;
      hash: string | null;
      file: boolean;
      sending: Sending;
      job: string | null;
      phase: Phase;
      elapsedS: number;
      log: string[];
    }
  | { kind: "loaded" };

export type StartEvent =
  | { type: "serve-missing"; reason: string }
  | { type: "serve-found" }
  | { type: "choose"; song: number; name: string }
  | { type: "hashed"; song: number; sha256: string }
  | { type: "tempo"; song: number; bpm: number | null }
  | { type: "tempo-failed"; song: number; message: string }
  | { type: "set-bpm"; bpm: number | null }
  | { type: "set-lyric"; text: string }
  | { type: "use-hint"; multiple: HintMultiple }
  | { type: "transcribe"; attempt: number }
  | { type: "retranscribe"; attempt: number; song: number; name: string; bpm: number; hash: string; file: boolean; lyric: string | null }
  | { type: "hash-refused"; attempt: number }
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
      return { kind: "ready", song: e.song, name: e.name, bpm: null, lyric: "", detected: "pending", hash: null, file: true, error: null };
    case "hashed":
      if ((s.kind !== "ready" && s.kind !== "running") || s.song !== e.song || s.hash !== null) return s;
      return { ...s, hash: e.sha256 };
    case "tempo":
    case "tempo-failed":
      if ((s.kind !== "ready" && s.kind !== "running") || s.song !== e.song || s.detected !== "pending") return s;
      return { ...s, detected: e.type === "tempo-failed" ? { error: e.message } : e.bpm === null ? null : round1(e.bpm) };
    case "set-bpm":
      return s.kind === "ready" ? { ...s, bpm: e.bpm, error: null } : s;
    case "set-lyric":
      return s.kind === "ready" ? { ...s, lyric: e.text, error: null } : s;
    case "use-hint":
      if (s.kind !== "ready" || typeof s.detected !== "number") return s;
      return { ...s, bpm: hintBpm(s.detected, e.multiple), error: null };
    case "transcribe": {
      if (s.kind !== "ready") return s;
      const bpm = s.bpm;
      if (!usableBpm(bpm)) return { ...s, error: `Enter the song's BPM (${MIN_BPM}–${MAX_BPM}): it sets the grid the notes snap to.` };
      if (s.hash === null && !s.file) return { ...s, error: GONE };
      return lyricToSend(s.lyric, (lyric) => running(s, e.attempt, bpm, lyric), (error) => ({ ...s, error }));
    }
    case "retranscribe": {
      if (s.kind === "running" || s.kind === "unavailable" || !usableBpm(e.bpm)) return s;
      const song = { song: e.song, name: e.name, detected: "unasked" as const, hash: e.hash, file: e.file };
      return lyricToSend(e.lyric ?? "", (lyric) => running(song, e.attempt, e.bpm, lyric), (error) => ({ kind: "ready", ...song, bpm: e.bpm, lyric: e.lyric ?? "", error }));
    }
    case "hash-refused":
      if (s.kind !== "running" || s.attempt !== e.attempt || s.job !== null || s.sending !== "hash") return s;
      return s.file ? { ...s, sending: "upload" } : backToSong(s, GONE);
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

const GONE = "sona serve no longer has this song: drop it here again to transcribe it.";
const NO_LYRIC = "Paste the song's lyric in hiragana, one sung line per line.";

/** A job needs a lyric sona takes: `go` with it, or `refuse` with what to fix. */
function lyricToSend(text: string, go: (lyric: string) => StartState, refuse: (error: string) => StartState): StartState {
  const c = checkLyric(text);
  return c.kind === "ok" ? go(c.lyric) : refuse(c.kind === "bad" ? c.message : NO_LYRIC);
}

type Running = Extract<StartState, { kind: "running" }>;

/** A job for the song in `s`, sent by its hash when there is one. */
function running(s: Pick<Running, "song" | "name" | "detected" | "hash" | "file">, attempt: number, bpm: number, lyric: string): Running {
  const { song, name, detected, hash, file } = s;
  return { kind: "running", song, attempt, name, bpm, lyric, detected, hash, file, sending: hash !== null ? "hash" : "upload", job: null, phase: "queued", elapsedS: 0, log: [] };
}

function backToSong(s: Running, error: string): StartState {
  return { kind: "ready", song: s.song, name: s.name, bpm: s.bpm, lyric: s.lyric, detected: s.detected, hash: s.hash, file: s.file, error };
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
