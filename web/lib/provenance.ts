/** What produced an annotation (ADR-0024), as written by `sona transcribe` and `sona serve`. */

export interface FileFingerprint {
  role: string;
  name: string;
  bytes: number;
  sha256: string;
}

export interface JobRecord {
  id: string;
  upload: FileFingerprint;
  steps: { step: string; seconds: number }[];
  tools: Record<string, string>;
  models: FileFingerprint[];
  /** The separation the job asked for (ADR-0032); absent in older records. */
  separation?: "none" | "vocals" | "lead";
  /** Where the separated stems came from (ADR-0030); `by_hash` when the job named its upload by hash (ADR-0032). */
  stems?: { key: string; reused: boolean; separated_by: string; by_hash?: true };
}

export interface Provenance {
  sona_version: string;
  sona_commit: string | null;
  created_unix_s: number;
  input: FileFingerprint;
  settings: Record<string, string>;
  models: FileFingerprint[];
  job?: JobRecord;
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: unknown): string => (typeof v === "string" ? v : "");
const num = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) ? v : 0);
const strMap = (v: unknown): Record<string, string> =>
  isObj(v) ? Object.fromEntries(Object.entries(v).filter((e): e is [string, string] => typeof e[1] === "string")) : {};

function file(v: unknown): FileFingerprint | null {
  if (!isObj(v) || typeof v.sha256 !== "string") return null;
  return { role: str(v.role), name: str(v.name), bytes: num(v.bytes), sha256: v.sha256 };
}

const files = (v: unknown): FileFingerprint[] => (Array.isArray(v) ? v.map(file).filter((f): f is FileFingerprint => f !== null) : []);

/** The annotation's provenance, or null when it has none (older annotations) or it is malformed. */
export function parseProvenance(v: unknown): Provenance | null {
  if (!isObj(v) || typeof v.sona_version !== "string") return null;
  const input = file(v.input);
  if (!input) return null;
  let job: JobRecord | undefined;
  if (isObj(v.job)) {
    const upload = file(v.job.upload);
    if (upload) {
      job = {
        id: str(v.job.id),
        upload,
        steps: Array.isArray(v.job.steps) ? v.job.steps.filter(isObj).map((s) => ({ step: str(s.step), seconds: num(s.seconds) })) : [],
        tools: strMap(v.job.tools),
        models: files(v.job.models),
        ...(v.job.separation === "none" || v.job.separation === "vocals" || v.job.separation === "lead" ? { separation: v.job.separation } : {}),
        ...(isObj(v.job.stems) && typeof v.job.stems.key === "string" && typeof v.job.stems.reused === "boolean"
          ? { stems: { key: v.job.stems.key, reused: v.job.stems.reused, separated_by: str(v.job.stems.separated_by), ...(v.job.stems.by_hash === true ? { by_hash: true as const } : {}) } }
          : {}),
      };
    }
  }
  return {
    sona_version: v.sona_version,
    sona_commit: typeof v.sona_commit === "string" ? v.sona_commit : null,
    created_unix_s: num(v.created_unix_s),
    input,
    settings: strMap(v.settings),
    models: files(v.models),
    ...(job ? { job } : {}),
  };
}

export interface DetailSection {
  title: string;
  rows: [string, string][];
}

const short = (sha: string) => sha.slice(0, 12);
const size = (b: number) => (b >= 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} kB`);
const fileRow = (f: FileFingerprint): [string, string] => [f.role, `${f.name} · ${size(f.bytes)} · ${short(f.sha256)}`];

/** The Details view, section by section. Hashes are shortened here; the download keeps them whole. */
export function detailSections(p: Provenance): DetailSection[] {
  const made = new Date(p.created_unix_s * 1000);
  const out: DetailSection[] = [
    {
      title: "Made by",
      rows: [
        ["sona", p.sona_commit ? `${p.sona_version} (${p.sona_commit})` : p.sona_version],
        ["when", Number.isNaN(made.getTime()) || p.created_unix_s === 0 ? "unknown" : made.toLocaleString()],
      ],
    },
    { title: "Input", rows: [fileRow(p.input), ...(p.job ? [fileRow(p.job.upload)] : [])] },
    { title: "Settings", rows: Object.entries(p.settings) },
  ];
  const models = [...p.models, ...(p.job?.models ?? [])];
  if (models.length) out.push({ title: "Models", rows: models.map(fileRow) });
  if (p.job) {
    const total = p.job.steps.reduce((t, s) => t + s.seconds, 0);
    out.push({ title: "Steps", rows: [...p.job.steps.map((s): [string, string] => [s.step, `${s.seconds.toFixed(1)} s`]), ["total", `${total.toFixed(1)} s`]] });
    out.push({ title: "Tools", rows: Object.entries(p.job.tools) });
    const st = p.job.stems;
    if (st) {
      const rows: [string, string][] = [["separated", st.reused ? `reused from job ${st.separated_by}` : "by this job"], ["cache key", short(st.key)]];
      if (st.by_hash) rows.push(["upload", "named by hash, not sent again"]);
      out.push({ title: "Stems", rows });
    }
  }
  return out;
}
