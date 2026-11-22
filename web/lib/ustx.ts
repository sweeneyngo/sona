/**
 * USTX (OpenUtau project) export (ADR-0018).
 *
 * The USTX is YAML: one track, one voice part holding the notes at 480 ticks
 * per quarter, a single tempo and 4/4. Each note gets OpenUtau's neutral pitch
 * points and an off vibrato. Pitch curves are a later milestone.
 *
 * The layout follows what OpenUtau and external tools (UtaFormatix) read:
 * `ustx_version` 0.7, the full expression table, `exp_selectors`. Every string
 * is double-quoted, so lyrics can't break the YAML.
 */

import { TICKS_PER_QUARTER, type Sequence } from "./sequence";
import { USTX_EXPRESSIONS } from "./ustx-expressions";

/** OpenUtau's melisma lyric: `+~` extends the previous syllable's vowel. */
const LYRIC_ALIASES: Record<string, string> = { "-": "+~", "ー": "+~" };

/** Build the USTX project object for `seq`. */
export function ustxProject(seq: Sequence, opts: { bpm: number; name?: string }): Record<string, unknown> {
  const name = opts.name ?? "sona";
  const end = seq.notes.reduce((m, n) => Math.max(m, n.position + n.duration), 0);
  return {
    name,
    comment: "Exported from sona",
    output_dir: "Vocal",
    cache_dir: "UCache",
    ustx_version: "0.7",
    resolution: TICKS_PER_QUARTER,
    bpm: opts.bpm,
    beat_per_bar: 4,
    beat_unit: 4,
    expressions: USTX_EXPRESSIONS,
    exp_selectors: ["dyn", "pitd", "clr", "eng", "vel", "vol", "atk", "dec", "gen", "bre"],
    exp_primary: 0,
    exp_secondary: 1,
    key: 0,
    time_signatures: [{ bar_position: 0, beat_per_bar: 4, beat_unit: 4 }],
    tempos: [{ position: 0, bpm: opts.bpm }],
    tracks: [
      {
        phonemizer: "OpenUtau.Core.DefaultPhonemizer",
        renderer_settings: {},
        track_name: "Track1",
        track_color: "Blue",
        mute: false,
        solo: false,
        volume: 0,
        pan: 0,
        track_expressions: [],
        voice_color_names: [""],
      },
    ],
    voice_parts: [
      {
        duration: Math.max(TICKS_PER_QUARTER, end),
        name,
        comment: "",
        track_no: 0,
        position: 0,
        notes: seq.notes.map((n) => ({
          position: n.position,
          duration: n.duration,
          tone: n.tone,
          lyric: LYRIC_ALIASES[n.lyric] ?? n.lyric,
          pitch: {
            data: [
              { x: -40, y: 0, shape: "io" },
              { x: 15, y: 0, shape: "io" },
            ],
            snap_first: true,
          },
          vibrato: { length: 0, period: 175, depth: 25, in: 10, out: 10, shift: 0, drift: 0, vol_link: 0 },
          phoneme_expressions: [],
          phoneme_overrides: [],
        })),
        curves: [],
      },
    ],
    wave_parts: [],
  };
}

/** The USTX file text for `seq`. */
export function ustxText(seq: Sequence, opts: { bpm: number; name?: string }): string {
  return toYaml(ustxProject(seq, opts)) + "\n";
}

/** A minimal block-style YAML emitter for plain JSON-like data. */
export function toYaml(value: unknown, indent = 0): string {
  const pad = " ".repeat(indent);
  if (Array.isArray(value)) {
    if (value.length === 0) return "[]";
    return value
      .map((item) => {
        if (isPlainObject(item) && Object.keys(item).length) {
          const body = toYaml(item, indent + 2);
          return `${pad}- ${body.slice(indent + 2)}`;
        }
        return `${pad}- ${scalarOrInline(item, indent + 2)}`;
      })
      .join("\n");
  }
  if (isPlainObject(value)) {
    const entries = Object.entries(value);
    if (entries.length === 0) return "{}";
    return entries
      .map(([k, v]) => {
        const key = /^[A-Za-z_][A-Za-z0-9_+]*$/.test(k) ? k : JSON.stringify(k);
        if ((Array.isArray(v) && v.length) || (isPlainObject(v) && Object.keys(v).length)) {
          return `${pad}${key}:\n${toYaml(v, indent + 2)}`;
        }
        return `${pad}${key}: ${scalarOrInline(v, indent + 2)}`;
      })
      .join("\n");
  }
  return scalarOrInline(value, indent);
}

function scalarOrInline(v: unknown, indent: number): string {
  if (Array.isArray(v)) return v.length ? "\n" + toYaml(v, indent) : "[]";
  if (isPlainObject(v)) return Object.keys(v).length ? "\n" + toYaml(v, indent) : "{}";
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "0";
  if (typeof v === "boolean") return v ? "true" : "false";
  return "null";
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
