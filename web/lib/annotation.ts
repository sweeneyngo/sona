/**
 * The **annotation JSON** — sona's primary artifact and the core↔UI contract
 * (ADR-0004). The Rust perception core (`crates/sona-core/src/annotation.rs`)
 * emits it; this review surface interprets it (grid anchoring, hard-snap
 * quantization, correction, MIDI export).
 *
 * These types mirror the Serde types one-for-one — keep the two in sync. All
 * timing is in **seconds** (raw, unquantized): snapping is a UI concern.
 */

/** Current annotation schema version (`SCHEMA_VERSION` in the Rust core). */
export const SCHEMA_VERSION = 1;

/** The backend or adapter that produced an annotation. */
export type Producer = "pyin" | "crepe" | "basic-pitch" | "game";

/**
 * Per-dimension confidence sub-scores, each in `0..=1`. A note is only as
 * trustworthy as its shakiest dimension (weakest-link soft-min → the scalar).
 */
export interface SubScores {
  /** How steady the f0 was across the note (vibrato-tolerant). */
  pitch_stability: number;
  /** Voicing strength vs. separation bleed / noise floor. */
  voicing: number;
  /** Sharpness of the onset transient. */
  onset_clarity: number;
  /** Steepness of the offset energy drop (sharp = high, slow tail = low). */
  offset_clarity: number;
}

/** A single detected note. Onset/offset are raw seconds; the UI hard-snaps. */
export interface Note {
  /** Note onset, in seconds from the start of the stem. */
  onset_s: number;
  /** Note offset, in seconds from the start of the stem. */
  offset_s: number;
  /** MIDI pitch number (0–127); 69 = A4 = 440 Hz. */
  midi_pitch: number;
  /** Combined confidence scalar in `0..=1` (weakest-link soft-min of sub-scores). */
  confidence: number;
  /** The per-dimension scores the scalar is composed from. */
  sub_scores: SubScores;
}

/**
 * One frame of the f0 contour (display aid). `voicing` is the per-frame
 * voiced/unvoiced confidence in `0..=1`; `frequency_hz` is unspecified
 * (typically `0`) where a frame is unvoiced.
 */
export interface F0Frame {
  /** Frame time, in seconds from the start of the stem. */
  time_s: number;
  /** Estimated fundamental frequency at this frame, in Hz. */
  frequency_hz: number;
  /** Voiced/unvoiced confidence at this frame, in `0..=1`. */
  voicing: number;
}

/** One frame of the energy/RMS envelope (display aid). */
export interface EnergyFrame {
  /** Frame time, in seconds from the start of the stem. */
  time_s: number;
  /** Root-mean-square amplitude at this frame. */
  rms: number;
}

/**
 * A complete annotation: the perception core's output for one vocal stem.
 * Timing is in seconds throughout. `bpm` is carried only to seed the UI grid —
 * the core never uses it (ADR-0004). `producer` is widened to `string` because
 * the loader tolerates an unknown/absent value for display.
 */
export interface Annotation {
  /** Schema version; see {@link SCHEMA_VERSION}. */
  version: number;
  /** Audio sample rate of the analysed stem, in Hz. */
  sample_rate: number;
  /** User-supplied tempo (single fixed BPM, v1). */
  bpm: number;
  /** Total stem duration, in seconds. */
  duration_s: number;
  /**
   * Which backend/adapter produced this annotation. The `& {}` keeps the
   * {@link Producer} literals as autocomplete suggestions while still tolerating
   * an unknown/absent value the loader may substitute for display.
   */
  producer: Producer | (string & {});
  /** Detected note events, in onset order. */
  notes: Note[];
  /** Per-frame fundamental-frequency contour (display aid). */
  f0_contour: F0Frame[];
  /** Per-frame energy/RMS envelope (display aid). */
  energy_envelope: EnergyFrame[];
}

/**
 * A note in the UI's editable working set: the raw annotation note plus a
 * `manual` flag marking notes the user placed/edited (which never escape the
 * grid — see `main.ts`).
 */
export interface WorkingNote extends Note {
  manual?: boolean;
}
