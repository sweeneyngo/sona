// sona review surface — browser layer (tickets 08–10).
//
// Loads an annotation JSON + the vocal stem and renders the piano-roll on a
// <canvas>: notes coloured by confidence, over the f0 contour and energy
// envelope, against a live BPM/division/phase grid. Onsets and offsets
// hard-snap to that grid (ADR-0003), with a confidence-aware escape hatch that
// spares — and flags — confident off-grid notes. The user drags notes to
// correct them (move / resize / add / delete), then exports the authoritative
// MIDI from the current snapped + corrected state (ADR-0004). Stem and
// synthesized notes play in sync via Web Audio.
//
// The pure geometry/colour/grid/schedule/MIDI maths live in ./lib/*.ts so they
// can be unit-tested under Vitest; this file is the DOM + Web Audio + canvas
// plumbing that can only run in a browser.

import {
  midiToNoteName,
  confidenceColor,
  weakestSubScore,
  makeScale,
  zoomToShowSeconds,
  defaultPitchTop,
  pitchTopRange,
} from "./lib/view";
import type { Scale } from "./lib/view";
import { detectKey, keyName, isInKey, snapToKey } from "./lib/key";
import type { KeyEstimate } from "./lib/key";
import { buildSchedule } from "./lib/synth";
import { clampPitch } from "./lib/math";
import { gridSpacingSeconds, snapTime, quantizeNote, noteFlags } from "./lib/grid";
import { encodeMidiFile } from "./lib/midi";
import type { Annotation, WorkingNote } from "./lib/annotation";

const RULER_H = 18; // top strip: click to seek, drag to shift the grid phase.
const PAD = { left: 44, top: RULER_H, bottom: 22 };
const NOTE_RADIUS = 6; // rounder note corners (clamped to the bar half-extent)
const NOTE_FILL_ALPHA = 0.62; // translucent fill so the f0 contour shows through
const RESIZE_GRAB_PX = 7; // right-edge zone that starts a resize instead of a move.
const FLAG_COLOR = "#ff5c5c";
const ESCAPE_COLOR = "#5cd0ff";
const OUT_OF_KEY_COLOR = "#c8a2ff";
const MIN_ZOOM_X = 1; // 1 = fit-to-width; can't zoom out past the whole song.
const MAX_ZOOM_X = 60;
const VISIBLE_SEMITONES = 24; // the roll shows ~2 octaves at a time; scroll for more.
const DEFAULT_VISIBLE_SECONDS = 10; // zoomed-in default view width.

const el = {
  json: document.getElementById("json-input") as HTMLInputElement,
  audio: document.getElementById("audio-input") as HTMLInputElement,
  play: document.getElementById("play") as HTMLButtonElement,
  stop: document.getElementById("stop") as HTMLButtonElement,
  muteStem: document.getElementById("mute-stem") as HTMLInputElement,
  muteSynth: document.getElementById("mute-synth") as HTMLInputElement,
  bpm: document.getElementById("bpm") as HTMLInputElement,
  division: document.getElementById("division") as HTMLSelectElement,
  flag: document.getElementById("flag") as HTMLInputElement,
  flagVal: document.getElementById("flag-val") as HTMLOutputElement,
  keyReadout: document.getElementById("key-readout") as HTMLOutputElement,
  keyDetail: document.getElementById("key-detail") as HTMLOutputElement,
  snapKey: document.getElementById("snap-key") as HTMLInputElement,
  save: document.getElementById("save") as HTMLButtonElement,
  export: document.getElementById("export") as HTMLButtonElement,
  status: document.getElementById("status") as HTMLElement,
  canvas: document.getElementById("roll") as HTMLCanvasElement,
  tooltip: document.getElementById("tooltip") as HTMLDivElement,
  help: document.getElementById("help") as HTMLButtonElement,
  helpOverlay: document.getElementById("help-overlay") as HTMLDivElement,
  helpClose: document.getElementById("help-close") as HTMLButtonElement,
  settings: document.getElementById("settings") as HTMLButtonElement,
  settingsOverlay: document.getElementById("settings-overlay") as HTMLDivElement,
  settingsClose: document.getElementById("settings-close") as HTMLButtonElement,
};
const ctx2d = el.canvas.getContext("2d")!;

// --- Types -----------------------------------------------------------------

interface EffectiveNote {
  raw: WorkingNote;
  onset_s: number;
  offset_s: number;
  pitch: number;
  flagged: boolean;
  lowConfidence: boolean;
  escaped: boolean;
  /** The note's pitch is outside the detected key (only when snap-to-key is off). */
  outOfKey: boolean;
}

interface Hitbox {
  x: number;
  y: number;
  w: number;
  h: number;
  note: WorkingNote;
  effective: EffectiveNote;
}

interface DragBase {
  startX: number;
  startY: number;
  moved: boolean;
}
type Drag =
  | (DragBase & { mode: "offset"; origAnchor: number })
  | (DragBase & {
      mode: "move" | "resize";
      note: WorkingNote;
      origOnset: number;
      origOffset: number;
      origPitch: number;
    });

interface Playback {
  startedAt: number;
  duration: number;
  from: number; // song time (seconds) playback started from (the seek offset)
  stemSource: AudioBufferSourceNode;
  stemGain: GainNode;
  synthGain: GainNode;
  voices: OscillatorNode[]; // every scheduled synth osc, so pause can stop them
  raf: number;
}

// --- State -----------------------------------------------------------------

let annotation: Annotation | null = null;
/** Editable working copy of the notes (raw seconds); the source of truth for edits. */
let workingNotes: WorkingNote[] = [];
/** Decoded stem, for playback and for the total roll duration. */
let audioBuffer: AudioBuffer | null = null;
/** Rendering geometry, rebuilt on load and resize. Null until data arrives. */
let scale: Scale | null = null;
/** Screen rects of drawn notes, for hover/hit-testing. Each carries its working note. */
let noteHitboxes: Hitbox[] = [];
/** The working note the user selected; null when nothing is selected. */
let selectedNote: WorkingNote | null = null;
/** In-progress drag (move / resize / grid-offset), or null. */
let drag: Drag | null = null;
/** True while the pointer is over the roll — gates the occupied-pitch highlight. */
let overRoll = false;
/** Unsaved edits since the last autosave (drives the 20s autosave tick). */
let dirty = false;

// Musical interpretation (ticket 09). Seeded on load; edited via the toolbar.
let bpm = 120;
let division = 16;
let gridAnchor = 0; // grid phase in seconds (auto = first onset, draggable).
let flagThreshold = 0.5;
// Snap each note's pitch to the detected key's scale (display + export). Off by
// default: out-of-key notes are flagged for review rather than auto-corrected.
let snapKey = false;
// Cached key estimate over the working notes; rebuilt lazily with the effective
// notes (invalidated on any edit).
let keyCache: KeyEstimate | null = null;
let keyComputed = false;

// View transform. Horizontal: zoom (⌘/Ctrl-scroll) + scrollX pan (shift-scroll).
// Vertical: a fixed 2-octave window whose top MIDI pitch is `pitchTop` (scroll).
let zoomX = 1;
let scrollX = 0;
let pitchTop = 79; // top visible MIDI pitch; window = [pitchTop-23, pitchTop].
// Playback start position in seconds; set by clicking the ruler (seek).
let seekOffset = 0;
// Cache of the interpreted notes; rebuilt lazily, invalidated on any edit/setting.
let effectiveCache: EffectiveNote[] | null = null;
// Visible time window (seconds), recomputed each draw() and used to cull grid
// lines / notes / contour / envelope to what's on screen — the perf lever on
// long stems.
let viewT0 = 0;
let viewT1 = 0;

const AudioCtx =
  window.AudioContext ||
  (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
let audioCtx: AudioContext | null = null;
let playback: Playback | null = null;

// --- Loading ---------------------------------------------------------------

el.json.addEventListener("change", async () => {
  const file = el.json.files?.[0];
  if (!file) return;
  try {
    const parsed = JSON.parse(await file.text());
    const session = applyLoadedJson(parsed);
    const what = session ? "session" : "annotation";
    setStatus(`Loaded ${what}: ${workingNotes.length} notes from “${file.name}”.`);
    dirty = false; // a freshly loaded file is not unsaved work
  } catch (err) {
    annotation = null;
    workingNotes = [];
    setStatus(`Could not read annotation JSON: ${(err as Error).message}`, true);
    rebuild();
  }
});

/**
 * Apply a parsed annotation/session object to the editing state (used by the
 * file loader and autosave restore). Returns whether it carried a `session`
 * block. Does not set a filename-specific status.
 */
function applyLoadedJson(parsed: any): boolean {
  annotation = normalizeAnnotation(parsed);
  // Build the editable set straight from the parsed notes so a saved session's
  // `manual` flags and exact raw positions survive the round-trip.
  workingNotes = (parsed.notes as any[]).map((n) => ({
    onset_s: Number(n.onset_s),
    offset_s: Number(n.offset_s),
    midi_pitch: Number(n.midi_pitch),
    confidence: Number(n.confidence),
    sub_scores: n.sub_scores ?? {},
    ...(n.manual ? { manual: true as const } : {}),
  }));
  selectedNote = null;
  seekOffset = 0;
  invalidateEffective();

  const session = parsed.session;
  if (session && typeof session === "object") {
    restoreSession(session); // a saved session carries its own review settings
  } else {
    // A fresh annotation: seed BPM from the file, grid phase on the first onset.
    bpm = annotation.bpm > 0 ? annotation.bpm : 120;
    el.bpm.value = String(bpm);
    gridAnchor = workingNotes.length ? Math.min(...workingNotes.map((n) => n.onset_s)) : 0;
  }
  resetView(); // zoomed-in default + 2-octave window centred on the notes.
  rebuild();
  return !!(session && typeof session === "object");
}

el.audio.addEventListener("change", async () => {
  const file = el.audio.files?.[0];
  if (!file) return;
  try {
    const bytes = await file.arrayBuffer();
    audioBuffer = await ensureAudioContext().decodeAudioData(bytes);
    setStatus(`Loaded stem “${file.name}” (${audioBuffer.duration.toFixed(2)} s).`);
    rebuild();
  } catch (err) {
    audioBuffer = null;
    setStatus(`Could not decode audio: ${(err as Error).message}`, true);
    rebuild();
  }
});

/**
 * Validate the shape we depend on and coerce the fields the roll reads. Throws
 * a human-readable error if the file isn't an annotation.
 */
function normalizeAnnotation(raw: any): Annotation {
  if (!raw || typeof raw !== "object" || !Array.isArray(raw.notes)) {
    throw new Error("expected an object with a `notes` array");
  }
  return {
    version: Number(raw.version) || 0,
    sample_rate: Number(raw.sample_rate) || 0,
    bpm: Number(raw.bpm) || 0,
    duration_s: Number(raw.duration_s) || 0,
    producer: raw.producer ?? "unknown",
    notes: raw.notes.map((n: any) => ({
      onset_s: Number(n.onset_s),
      offset_s: Number(n.offset_s),
      midi_pitch: Number(n.midi_pitch),
      confidence: Number(n.confidence),
      sub_scores: n.sub_scores ?? {},
    })),
    f0_contour: Array.isArray(raw.f0_contour) ? raw.f0_contour : [],
    energy_envelope: Array.isArray(raw.energy_envelope) ? raw.energy_envelope : [],
  };
}

// --- Musical interpretation (grid) -----------------------------------------

/** Current grid cell size in seconds. */
function spacing(): number {
  return gridSpacingSeconds(bpm, division);
}

/** Drop the cached interpretation; next `effectiveNotes()` recomputes it. */
function invalidateEffective(): void {
  effectiveCache = null;
  keyComputed = false;
}

/**
 * The key detected from the working notes' raw pitches (interpretation, ADR-0004
 * — never fed back into perception). Cached alongside the effective notes and
 * recomputed after edits; `null` when there are no notes.
 */
function currentKey(): KeyEstimate | null {
  if (!keyComputed) {
    keyCache = detectKey(workingNotes);
    keyComputed = true;
  }
  return keyCache;
}

/** Refresh the toolbar + settings key readouts from the current detection. */
function updateKeyReadout(): void {
  const key = currentKey();
  el.keyReadout.textContent = key ? keyName(key) : "—";
  el.keyDetail.textContent = key ? keyName(key) : "—";
}

/**
 * The notes as interpreted for display, playback, and export: each working note
 * hard-snapped to the grid (or escaped) with its flag state. Manual notes never
 * escape — the user placed them deliberately, so they always snap. Cached:
 * recomputing this per frame was a big part of the playback jank on long stems.
 */
function effectiveNotes(): EffectiveNote[] {
  if (effectiveCache) return effectiveCache;
  const s = spacing();
  const key = currentKey();
  effectiveCache = workingNotes.map((n) => {
    // Manual notes never escape; everything else uses grid.ts's documented
    // escape-hatch defaults (ADR-0003).
    const q = quantizeNote(
      n,
      n.manual ? { spacing: s, anchor: gridAnchor, escapeDistanceCells: Infinity } : { spacing: s, anchor: gridAnchor },
    );
    const flags = noteFlags(n, q, flagThreshold);
    // Pitch interpretation: snap to the detected key's scale when enabled, else
    // keep the raw pitch and flag it if it falls outside the key.
    const pitch = key && snapKey ? snapToKey(n.midi_pitch, key) : n.midi_pitch;
    const outOfKey = key ? !isInKey(pitch, key) : false;
    return { raw: n, onset_s: q.onset_s, offset_s: q.offset_s, pitch, ...flags, outOfKey };
  });
  return effectiveCache;
}

/** The effective notes reshaped for the MIDI encoder / synth (seconds + pitch). */
function midiNotes() {
  return effectiveNotes().map((e) => ({
    onset_s: e.onset_s,
    offset_s: e.offset_s,
    midi_pitch: e.pitch,
    confidence: e.raw.confidence,
  }));
}

/** MIDI pitch extent of the notes, for centring the default window. */
function noteMidiExtent(): { min: number; max: number } {
  const pitches = effectiveNotes().map((e) => e.pitch);
  if (pitches.length === 0) return { min: 60, max: 72 };
  return { min: Math.min(...pitches), max: Math.max(...pitches) };
}

/** Clamp `pitchTop` so the 2-octave window stays over the notes. */
function clampPitchTop(top: number): number {
  const { min, max } = noteMidiExtent();
  const r = pitchTopRange(min, max, VISIBLE_SEMITONES);
  return Math.min(r.max, Math.max(r.min, top));
}

/** Reset the view to the zoomed-in default: ~10s wide, window centred on notes. */
function resetView(): void {
  zoomX = zoomToShowSeconds(totalDuration(), DEFAULT_VISIBLE_SECONDS, MAX_ZOOM_X, MIN_ZOOM_X);
  scrollX = 0;
  const { min, max } = noteMidiExtent();
  pitchTop = clampPitchTop(defaultPitchTop(min, max, VISIBLE_SEMITONES));
}

// --- Toolbar controls -------------------------------------------------------

el.bpm.addEventListener("input", () => {
  const v = Number(el.bpm.value);
  if (v > 0 && Number.isFinite(v)) {
    bpm = v;
    invalidateEffective();
    markDirty();
    redraw();
  }
});
el.division.addEventListener("change", () => {
  division = Number(el.division.value);
  invalidateEffective();
  markDirty();
  redraw();
});
el.flag.addEventListener("input", () => {
  flagThreshold = Number(el.flag.value);
  el.flagVal.textContent = flagThreshold.toFixed(2);
  invalidateEffective();
  markDirty();
  redraw();
});
el.snapKey.addEventListener("change", () => {
  snapKey = el.snapKey.checked;
  invalidateEffective(); // snapping changes note pitches
  markDirty();
  redraw();
});
el.export.addEventListener("click", exportMidi);

// --- Modals (help + settings) -----------------------------------------------

function setModal(overlay: HTMLElement, open: boolean): void {
  // Only one modal at a time; opening one closes the other.
  if (open) {
    el.helpOverlay.hidden = true;
    el.settingsOverlay.hidden = true;
  }
  overlay.hidden = !open;
}
function anyModalOpen(): boolean {
  return !el.helpOverlay.hidden || !el.settingsOverlay.hidden;
}

el.help.addEventListener("click", () => setModal(el.helpOverlay, el.helpOverlay.hidden));
el.helpClose.addEventListener("click", () => setModal(el.helpOverlay, false));
el.helpOverlay.addEventListener("click", (e) => {
  if (e.target === el.helpOverlay) setModal(el.helpOverlay, false);
});
el.settings.addEventListener("click", () => setModal(el.settingsOverlay, el.settingsOverlay.hidden));
el.settingsClose.addEventListener("click", () => setModal(el.settingsOverlay, false));
el.settingsOverlay.addEventListener("click", (e) => {
  if (e.target === el.settingsOverlay) setModal(el.settingsOverlay, false);
});

// --- Layout / geometry ------------------------------------------------------

/** Total seconds the roll spans: the longer of the notes and the stem. */
function totalDuration(): number {
  const noteEnd = workingNotes.length
    ? Math.max(annotation?.duration_s ?? 0, ...workingNotes.map((n) => n.offset_s))
    : (annotation?.duration_s ?? 0);
  const audioEnd = audioBuffer ? audioBuffer.duration : 0;
  return Math.max(noteEnd, audioEnd, 1);
}

/** Plot area width in CSS px (canvas minus the left key gutter). */
function plotWidthPx(): number {
  return Math.max(1, cssSize().w - PAD.left);
}

/** How far the zoomed timeline can be panned left before its end reaches the edge. */
function maxScrollX(): number {
  return Math.max(0, plotWidthPx() * zoomX - plotWidthPx());
}

function clampScroll(s: number): number {
  return Math.min(maxScrollX(), Math.max(0, s));
}

/**
 * (Re)build the scale from the current view state: a fixed 2-octave pitch window
 * topped at `pitchTop`, plus the horizontal zoom/pan. Clamps both scrolls to
 * range. Called on every redraw, so it stays cheap and never reflows the axes
 * from note data (the window size is fixed).
 */
function syncScale(): void {
  if (!annotation) {
    scale = null;
    return;
  }
  scrollX = clampScroll(scrollX);
  pitchTop = clampPitchTop(pitchTop);
  scale = makeScale({
    durationS: totalDuration(),
    minPitch: pitchTop - (VISIBLE_SEMITONES - 1),
    maxPitch: pitchTop,
    width: el.canvas.width / devicePixelRatioSafe(),
    height: el.canvas.height / devicePixelRatioSafe(),
    padLeft: PAD.left,
    padTop: PAD.top,
    padBottom: PAD.bottom,
    zoomX,
    scrollX,
  });
}

/** Rebuild after a resize/load: rebuild the scale, redraw. */
function rebuild(): void {
  resizeCanvas();
  syncScale();
  updateKeyReadout();
  draw();
  el.play.disabled = !(annotation && audioBuffer) || !!playback;
  el.stop.disabled = !playback;
  el.export.disabled = !annotation;
  el.save.disabled = !annotation;
}

/** Redraw after a view/grid/edit change: rebuild the scale (not the canvas). */
function redraw(): void {
  syncScale();
  draw();
}

function devicePixelRatioSafe(): number {
  return window.devicePixelRatio || 1;
}

/** Size the backing store to the CSS box × DPR so lines stay crisp. */
function resizeCanvas(): void {
  const dpr = devicePixelRatioSafe();
  const rect = el.canvas.getBoundingClientRect();
  el.canvas.width = Math.max(1, Math.round(rect.width * dpr));
  el.canvas.height = Math.max(1, Math.round(rect.height * dpr));
  ctx2d.setTransform(dpr, 0, 0, dpr, 0, 0);
}

window.addEventListener("resize", rebuild);

// --- Drawing ----------------------------------------------------------------

function cssSize(): { w: number; h: number } {
  const dpr = devicePixelRatioSafe();
  return { w: el.canvas.width / dpr, h: el.canvas.height / dpr };
}

function draw(): void {
  const { w, h } = cssSize();
  ctx2d.clearRect(0, 0, w, h);
  ctx2d.fillStyle = "#0d0f14";
  ctx2d.fillRect(0, 0, w, h);

  if (!annotation || !scale) {
    drawPlaceholder(w, h);
    return;
  }

  // The visible time window — everything below culls to this so a zoomed-in
  // view of a long stem only touches on-screen frames/notes/grid lines.
  viewT0 = scale.xToTime(PAD.left);
  viewT1 = scale.xToTime(w);

  // Pitch rows + note-name labels live partly in the left gutter — draw unclipped.
  drawPitchGrid(w, h);
  drawRulerBg(w);

  // Ruler ticks: clipped to the ruler strip so they don't run into the gutter.
  ctx2d.save();
  ctx2d.beginPath();
  ctx2d.rect(PAD.left, 0, w - PAD.left, RULER_H);
  ctx2d.clip();
  drawRulerTicks();
  ctx2d.restore();

  // Plot content: clipped to the plot area (below the ruler, right of the
  // gutter) so notes outside the 2-octave window and off-screen x are cut off.
  ctx2d.save();
  ctx2d.beginPath();
  ctx2d.rect(PAD.left, PAD.top, w - PAD.left, h - PAD.top);
  ctx2d.clip();
  drawTimeGrid(w, h);
  if (overRoll || drag) drawPitchHighlights(w); // occupied-pitch guide on hover/edit
  drawEnergyEnvelope(w, h);
  drawF0Contour();
  drawNotes();
  // currentPlayheadS() returns the seek offset when stopped, so this covers both.
  drawPlayhead(currentPlayheadS(), h, !playback);
  ctx2d.restore();
}

function drawPlaceholder(w: number, h: number): void {
  ctx2d.fillStyle = "#5b6072";
  ctx2d.font = "13px system-ui, sans-serif";
  ctx2d.textAlign = "center";
  ctx2d.fillText("Open ⚙ Settings to load an annotation JSON.", w / 2, h / 2);
  ctx2d.textAlign = "left";
}

/** Horizontal rows per semitone, each labelled with its note name in the gutter. */
function drawPitchGrid(w: number, _h: number): void {
  if (!scale) return;
  ctx2d.font = '9px "Space Mono", ui-monospace, monospace';
  ctx2d.textAlign = "center";
  ctx2d.textBaseline = "middle";
  for (let pitch = scale.minPitch; pitch <= scale.maxPitch; pitch++) {
    const y = scale.pitchToY(pitch);
    const isC = pitch % 12 === 0;
    const black = isBlackKey(pitch);
    if (black) {
      ctx2d.fillStyle = "#171a22";
      ctx2d.fillRect(PAD.left, y, w - PAD.left, scale.rowHeight);
    }
    ctx2d.strokeStyle = isC ? "#333846" : "#20232c";
    ctx2d.beginPath();
    ctx2d.moveTo(PAD.left, y);
    ctx2d.lineTo(w, y);
    ctx2d.stroke();
    // Label every row, centred in the key gutter; C brighter, black keys dimmer.
    ctx2d.fillStyle = isC ? "#9aa0b0" : black ? "#565c6a" : "#787e8c";
    ctx2d.fillText(midiToNoteName(pitch), PAD.left / 2, y + scale.rowHeight / 2);
  }
  ctx2d.textAlign = "left";
  ctx2d.textBaseline = "alphabetic";
}

function isBlackKey(midi: number): boolean {
  return [1, 3, 6, 8, 10].includes(((midi % 12) + 12) % 12);
}

/**
 * Faintly band the pitch rows that visible notes occupy — a guide, shown while
 * hovering or editing, for the set of pitches in play ("the chord of the song").
 * Distinct pitches only, within the visible time window and the 2-octave band.
 */
function drawPitchHighlights(w: number): void {
  if (!scale) return;
  const seen = new Set<number>();
  for (const e of effectiveNotes()) {
    if (e.offset_s < viewT0 || e.onset_s > viewT1) continue;
    if (e.pitch < scale.minPitch || e.pitch > scale.maxPitch || seen.has(e.pitch)) continue;
    seen.add(e.pitch);
    ctx2d.fillStyle = "rgba(189, 181, 145, 0.1)"; // accent, faint
    ctx2d.fillRect(PAD.left, scale.pitchToY(e.pitch), w - PAD.left, scale.rowHeight);
  }
}

/**
 * The grid lines *within the visible time window* — screen x plus whether each
 * lands on a beat. Culled to `[viewT0, viewT1]` so a fine grid over a long stem
 * only produces the handful of lines actually on screen.
 */
function gridLineInfo(): { x: number; isBeat: boolean }[] {
  const s = spacing();
  const cellsPerBeat = division / 4; // a beat is a quarter note.
  const from = Math.max(0, viewT0);
  const to = Math.min(totalDuration(), viewT1);
  const kStart = Math.ceil((from - gridAnchor) / s - 1e-9);
  const kEnd = Math.floor((to - gridAnchor) / s + 1e-9);
  const out: { x: number; isBeat: boolean }[] = [];
  for (let k = kStart; k <= kEnd; k++) {
    const t = gridAnchor + k * s;
    if (t < -1e-9) continue;
    out.push({ x: scale!.timeToX(t), isBeat: ((k % cellsPerBeat) + cellsPerBeat) % cellsPerBeat === 0 });
  }
  return out;
}

/** Vertical grid lines from BPM/division/phase; beat lines drawn stronger. */
function drawTimeGrid(_w: number, h: number): void {
  for (const { x, isBeat } of gridLineInfo()) {
    ctx2d.strokeStyle = isBeat ? "#2f3646" : "#1c2029";
    ctx2d.beginPath();
    ctx2d.moveTo(x, PAD.top);
    ctx2d.lineTo(x, h);
    ctx2d.stroke();
  }
}

/** The transport/phase ruler background + its gutter label (drawn unclipped). */
function drawRulerBg(w: number): void {
  ctx2d.fillStyle = "#171a22";
  ctx2d.fillRect(PAD.left, 0, w - PAD.left, RULER_H);
  ctx2d.strokeStyle = "#2b2f3a";
  ctx2d.beginPath();
  ctx2d.moveTo(PAD.left, RULER_H - 0.5);
  ctx2d.lineTo(w, RULER_H - 0.5);
  ctx2d.stroke();
  ctx2d.fillStyle = "#5b6072";
  ctx2d.font = "9px system-ui, sans-serif";
  ctx2d.fillText("seek ▸", 6, RULER_H - 6);
}

/** A tick on each grid line in the ruler (clipped to the plot area). */
function drawRulerTicks(): void {
  for (const { x, isBeat } of gridLineInfo()) {
    ctx2d.strokeStyle = isBeat ? "#6c7488" : "#3a4050";
    ctx2d.beginPath();
    ctx2d.moveTo(x, isBeat ? 3 : RULER_H - 7);
    ctx2d.lineTo(x, RULER_H - 1);
    ctx2d.stroke();
  }
}

/**
 * Energy/RMS envelope as a filled area along the bottom — a loudness guide.
 * Culled to the visible time window (one frame of margin each side) and drawn as
 * a single filled path.
 */
function drawEnergyEnvelope(_w: number, h: number): void {
  if (!annotation || !scale) return;
  const env = annotation.energy_envelope;
  if (env.length === 0) return;
  const maxRms = Math.max(...env.map((f) => f.rms), 1e-6);
  const band = Math.min(70, h * 0.18);
  const base = h - PAD.bottom;
  let started = false;
  let lastX = 0;
  ctx2d.beginPath();
  for (let i = 0; i < env.length; i++) {
    const f = env[i];
    if (f.time_s < viewT0 && env[i + 1]?.time_s < viewT0) continue; // keep one before
    if (f.time_s > viewT1 && env[i - 1]?.time_s > viewT1) break; // and one after
    const x = scale.timeToX(f.time_s);
    if (!started) {
      ctx2d.moveTo(x, base);
      started = true;
    }
    ctx2d.lineTo(x, base - (f.rms / maxRms) * band);
    lastX = x;
  }
  if (!started) return;
  ctx2d.lineTo(lastX, base);
  ctx2d.closePath();
  ctx2d.fillStyle = "rgba(110, 168, 254, 0.12)";
  ctx2d.fill();
}

/**
 * f0 contour, culled to the visible window. Two passes (one stroke each) so the
 * confidently-voiced parts read brighter without the per-segment stroking that
 * was the main playback-jank source: a dim base over every voiced frame for
 * continuity, and a bright overlay on the strongly-voiced ones. The pen lifts on
 * unvoiced frames and across time gaps, so distinct phrases aren't joined.
 */
// Bridge unvoiced dropouts shorter than this (seconds); longer gaps — real rests
// between notes — still break the line so we never draw pitch that isn't there.
const CONTOUR_BRIDGE_S = 0.18;
// A contour segment steeper than this (semitones/sec) is not drawn as a connector
// — no voice slides an octave in ~10ms (~1200 st/s), so such a segment is a
// tracking spike or a jump between two notes, and a vertical streak across it just
// clutters the line. This is a *rendering* break (we don't connect the points),
// NOT a perception filter: the frame still exists and the notes are untouched
// (ADR-0004). It replaces the display role of the ticket-08 slope filter, which
// also (wrongly, for the contour) got removed with the note-level MAD trim.
const MAX_CONTOUR_SLOPE_ST_PER_S = 150;

function drawF0Contour(): void {
  if (!annotation || !scale) return;
  const contour = annotation.f0_contour;
  strokeContour(contour, 0, "rgba(255, 255, 255, 0.45)");
  strokeContour(contour, 0.4, "rgba(255, 255, 255, 0.9)");
}

function strokeContour(
  contour: { time_s: number; frequency_hz: number; voicing: number }[],
  minVoicing: number,
  style: string,
): void {
  ctx2d.strokeStyle = style;
  ctx2d.lineWidth = 1.5;
  ctx2d.beginPath();
  let pen = false;
  let lastT = 0;
  let lastHz = 0;
  for (const f of contour) {
    // Skip frames that don't qualify — but *don't* force the pen up here. Whether
    // to break is decided by the time gap to the last drawn point, so brief
    // dropouts bridge and only real gaps split the line.
    if (f.frequency_hz <= 0 || f.voicing < minVoicing) continue;
    if (f.time_s < viewT0 - 0.2 || f.time_s > viewT1 + 0.2) {
      pen = false; // off-screen: start a fresh sub-path when we re-enter
      continue;
    }
    const dt = f.time_s - lastT;
    // Break (don't connect) across an implausibly steep pitch slope — a spike or a
    // note-to-note jump — so it doesn't render as a vertical streak.
    const steep = pen && dt > 0 && Math.abs(12 * Math.log2(f.frequency_hz / lastHz)) / dt > MAX_CONTOUR_SLOPE_ST_PER_S;
    const x = scale!.timeToX(f.time_s);
    const y = freqToY(f.frequency_hz);
    if (!pen || dt > CONTOUR_BRIDGE_S || steep) ctx2d.moveTo(x, y);
    else ctx2d.lineTo(x, y);
    pen = true;
    lastT = f.time_s;
    lastHz = f.frequency_hz;
  }
  ctx2d.stroke();
  ctx2d.lineWidth = 1;
}

/** Map a raw f0 (Hz) to the same pitch axis as the notes, via fractional MIDI. */
function freqToY(hz: number): number {
  const midi = 69 + 12 * Math.log2(hz / 440);
  return scale!.pitchToY(midi) + scale!.rowHeight / 2;
}

/**
 * The (snapped/escaped) note bars, coloured by confidence. Flagged notes get a
 * red outline, escaped notes a dashed cyan outline, the selected note a white
 * ring. Hitboxes carry the working note so edits mutate the source of truth.
 */
function drawNotes(): void {
  if (!scale) return;
  noteHitboxes = [];
  const barH = Math.max(3, scale.rowHeight - 2);
  for (const eff of effectiveNotes()) {
    if (eff.offset_s < viewT0 || eff.onset_s > viewT1) continue; // cull off-screen
    const x = scale.timeToX(eff.onset_s);
    const y = scale.pitchToY(eff.pitch) + (scale.rowHeight - barH) / 2;
    const wNote = Math.max(3, scale.timeToX(eff.offset_s) - x);
    // Translucent fill so the f0 contour drawn underneath reads through the note.
    ctx2d.fillStyle = confidenceColor(eff.raw.confidence);
    ctx2d.globalAlpha = NOTE_FILL_ALPHA;
    roundRect(x, y, wNote, barH, NOTE_RADIUS);
    ctx2d.fill();
    ctx2d.globalAlpha = 1; // outlines and everything after stay fully opaque

    // Outline review cues: escaped (dashed cyan), flagged (red), out-of-key
    // (dashed violet) — the confidence flag takes visual priority over the key hint.
    ctx2d.save();
    if (eff.escaped) {
      ctx2d.setLineDash([4, 3]);
      ctx2d.strokeStyle = ESCAPE_COLOR;
      ctx2d.lineWidth = 2;
    } else if (eff.flagged) {
      ctx2d.strokeStyle = FLAG_COLOR;
      ctx2d.lineWidth = 2;
    } else if (eff.outOfKey) {
      ctx2d.setLineDash([2, 3]);
      ctx2d.strokeStyle = OUT_OF_KEY_COLOR;
      ctx2d.lineWidth = 2;
    } else {
      ctx2d.strokeStyle = "rgba(0,0,0,0.4)";
      ctx2d.lineWidth = 1;
    }
    ctx2d.stroke();
    ctx2d.restore();

    if (eff.raw === selectedNote) {
      ctx2d.strokeStyle = "#ffffff";
      ctx2d.lineWidth = 2;
      roundRect(x - 1.5, y - 1.5, wNote + 3, barH + 3, NOTE_RADIUS + 1);
      ctx2d.stroke();
      ctx2d.lineWidth = 1;
    }
    noteHitboxes.push({ x, y, w: wNote, h: barH, note: eff.raw, effective: eff });
  }
}

function roundRect(x: number, y: number, w: number, h: number, r: number): void {
  const radius = Math.min(r, w / 2, h / 2);
  ctx2d.beginPath();
  ctx2d.moveTo(x + radius, y);
  ctx2d.arcTo(x + w, y, x + w, y + h, radius);
  ctx2d.arcTo(x + w, y + h, x, y + h, radius);
  ctx2d.arcTo(x, y + h, x, y, radius);
  ctx2d.arcTo(x, y, x + w, y, radius);
  ctx2d.closePath();
}

/** The playhead: bright while playing, dim when stopped (the seek marker). */
function drawPlayhead(t: number, h: number, dim: boolean): void {
  const x = scale!.timeToX(t);
  ctx2d.strokeStyle = dim ? "rgba(255, 209, 102, 0.45)" : "#ffd166";
  ctx2d.lineWidth = 1.5;
  ctx2d.beginPath();
  ctx2d.moveTo(x, 0);
  ctx2d.lineTo(x, h);
  ctx2d.stroke();
  ctx2d.lineWidth = 1;
}

// --- Interaction: select, drag (move/resize), grid-offset, add, delete ------

el.canvas.addEventListener("mousedown", (e: MouseEvent) => {
  if (!scale) return;
  const { x, y } = canvasPoint(e);

  // The top ruler: a click seeks the playhead, a drag shifts the grid phase.
  if (y < RULER_H) {
    drag = { mode: "offset", startX: x, startY: y, origAnchor: gridAnchor, moved: false };
    return;
  }

  const hit = hitTest(x, y);
  if (hit) {
    selectedNote = hit.note;
    const onRightEdge = x >= hit.x + hit.w - RESIZE_GRAB_PX;
    drag = {
      mode: onRightEdge ? "resize" : "move",
      note: hit.note,
      startX: x,
      startY: y,
      origOnset: hit.note.onset_s,
      origOffset: hit.note.offset_s,
      origPitch: hit.note.midi_pitch,
      moved: false,
    };
    redraw();
    showSelectedTooltip();
  } else {
    // Click on empty space clears the selection.
    selectedNote = null;
    drag = null;
    el.tooltip.style.display = "none";
    redraw();
  }
});

window.addEventListener("mousemove", (e: MouseEvent) => {
  if (!scale) return;
  if (!drag) {
    updateHover(e);
    return;
  }
  const d = drag;
  const { x, y } = canvasPoint(e);
  d.moved = d.moved || Math.abs(x - d.startX) > 2 || Math.abs(y - d.startY) > 2;
  const dt = scale.xToTime(x) - scale.xToTime(d.startX);

  if (d.mode === "offset") {
    // Only shift the grid once it's really a drag; a sub-threshold click stays a
    // seek (handled on mouseup) with the phase untouched.
    if (d.moved) {
      gridAnchor = d.origAnchor + dt;
      invalidateEffective(); // snapping depends on the phase
      markDirty();
      redraw();
    }
    return;
  }

  const n = d.note;
  if (d.mode === "move") {
    n.onset_s = Math.max(0, d.origOnset + dt);
    n.offset_s = n.onset_s + (d.origOffset - d.origOnset);
    const dRows = scale.yToPitch(y) - scale.yToPitch(d.startY);
    n.midi_pitch = clampPitch(d.origPitch + dRows);
    n.manual = true;
  } else {
    // resize: keep at least a hair of length; the grid snap rounds it to a cell.
    n.offset_s = Math.max(n.onset_s + spacing() / 2, d.origOffset + dt);
    n.manual = true;
  }
  invalidateEffective(); // the note's quantized position changed
  markDirty();
  redraw();
  showSelectedTooltip();
});

window.addEventListener("mouseup", () => {
  const d = drag;
  drag = null;
  // A click on the ruler that never became a drag is a seek. (The grid phase is
  // only written once a drag passes the move threshold, so a click leaves it be.)
  if (d && d.mode === "offset" && !d.moved && scale) {
    setSeek(scale.xToTime(d.startX));
  }
});

// Wheel: ⌘/Ctrl = horizontal zoom (anchored at cursor); Shift or horizontal
// gesture = pan time; plain vertical = pan pitch (the roll only shows 2 octaves).
el.canvas.addEventListener(
  "wheel",
  (e: WheelEvent) => {
    if (!scale || !annotation) return;
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      const { x } = canvasPoint(e);
      const tAtCursor = scale.xToTime(x);
      const factor = Math.exp(-e.deltaY * 0.0015);
      zoomX = Math.min(MAX_ZOOM_X, Math.max(MIN_ZOOM_X, zoomX * factor));
      // Re-anchor so the time under the cursor stays put across the zoom.
      const content = plotWidthPx() * zoomX;
      scrollX = clampScroll(PAD.left + (tAtCursor / totalDuration()) * content - x);
    } else if (e.shiftKey || Math.abs(e.deltaX) > Math.abs(e.deltaY)) {
      // Pan time. Nothing to scroll at fit-to-width → let the page scroll.
      if (maxScrollX() <= 0) return;
      e.preventDefault();
      const dx = e.deltaX !== 0 ? e.deltaX : e.deltaY;
      scrollX = clampScroll(scrollX + dx);
    } else {
      // Pan pitch. If the notes already fit the window there's nothing to scroll.
      const r = pitchTopBounds();
      if (r.min >= r.max) return;
      e.preventDefault();
      const rows = Math.max(1, Math.round(Math.abs(e.deltaY) / 40));
      pitchTop = clampPitchTop(pitchTop - Math.sign(e.deltaY) * rows);
    }
    redraw();
  },
  { passive: false },
);

/** Current allowed range for `pitchTop` (for deciding whether pitch can scroll). */
function pitchTopBounds(): { min: number; max: number } {
  const { min, max } = noteMidiExtent();
  return pitchTopRange(min, max, VISIBLE_SEMITONES);
}

/** Move the playhead / playback start to `t` seconds; restart if playing. */
function setSeek(t: number): void {
  seekOffset = Math.min(totalDuration(), Math.max(0, t));
  if (playback) {
    stopPlayback();
    startPlayback();
  } else {
    followPlayhead(); // keep the (possibly off-screen) seek target in view
    redraw();
  }
}

el.canvas.addEventListener("dblclick", (e: MouseEvent) => {
  if (!scale || !annotation) return;
  const { x, y } = canvasPoint(e);
  // Double-click the ruler to reset zoom/pan/pitch back to the default view.
  if (y < RULER_H) {
    resetView();
    redraw();
    return;
  }
  const s = spacing();
  const onset = Math.max(0, snapTime(scale.xToTime(x), s, gridAnchor));
  const note: WorkingNote = {
    onset_s: onset,
    offset_s: onset + s,
    midi_pitch: clampPitch(scale.yToPitch(y)),
    confidence: 1,
    sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 1, offset_clarity: 1 },
    manual: true,
  };
  workingNotes.push(note);
  selectedNote = note;
  invalidateEffective();
  markDirty();
  redraw();
  showSelectedTooltip();
});

window.addEventListener("keydown", (e: KeyboardEvent) => {
  // Modal first: Escape closes any open modal, "?" toggles help — regardless of focus.
  if (e.key === "Escape" && anyModalOpen()) {
    setModal(el.helpOverlay, false);
    setModal(el.settingsOverlay, false);
    e.preventDefault();
    return;
  }
  const tag = document.activeElement?.tagName;
  const inField = tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA";
  if (e.key === "?" && !inField) {
    setModal(el.helpOverlay, el.helpOverlay.hidden);
    e.preventDefault();
    return;
  }

  // Don't hijack the roll shortcuts while a form control is focused or a modal
  // is open.
  if (inField || anyModalOpen()) return;
  if (!annotation) return;

  const beat = 60 / bpm; // seek step: one beat, or a bar with Shift.

  switch (e.key) {
    case "Delete":
    case "Backspace":
      if (!selectedNote) return;
      workingNotes = workingNotes.filter((n) => n !== selectedNote);
      selectedNote = null;
      el.tooltip.style.display = "none";
      invalidateEffective();
      markDirty();
      redraw();
      break;
    case " ": // play / pause
      togglePlay();
      break;
    case "ArrowRight":
      setSeek(seekOffset + beat * (e.shiftKey ? 4 : 1));
      break;
    case "ArrowLeft":
      setSeek(seekOffset - beat * (e.shiftKey ? 4 : 1));
      break;
    case "ArrowUp": // pan pitch up (a semitone, or an octave with Shift)
      pitchTop = clampPitchTop(pitchTop + (e.shiftKey ? 12 : 1));
      redraw();
      break;
    case "ArrowDown":
      pitchTop = clampPitchTop(pitchTop - (e.shiftKey ? 12 : 1));
      redraw();
      break;
    case "+":
    case "=":
      zoomBy(1.3);
      break;
    case "-":
    case "_":
      zoomBy(1 / 1.3);
      break;
    case "Home":
      setSeek(0);
      break;
    case "0":
      resetView();
      redraw();
      break;
    default:
      return; // not a shortcut — leave the event alone
  }
  e.preventDefault();
});

/** Zoom the timeline by `factor` about the centre of the visible plot. */
function zoomBy(factor: number): void {
  if (!scale) return;
  const centerX = PAD.left + plotWidthPx() / 2;
  const tAtCenter = scale.xToTime(centerX);
  zoomX = Math.min(MAX_ZOOM_X, Math.max(MIN_ZOOM_X, zoomX * factor));
  const content = plotWidthPx() * zoomX;
  scrollX = clampScroll(PAD.left + (tAtCenter / totalDuration()) * content - centerX);
  redraw();
}

function updateHover(e: MouseEvent): void {
  const { x, y } = canvasPoint(e);
  const hit = hitTest(x, y);
  // Cursor cues: resize on the right edge, move on the body, grid on the ruler.
  if (y < RULER_H) el.canvas.style.cursor = "ew-resize";
  else if (hit) el.canvas.style.cursor = x >= hit.x + hit.w - RESIZE_GRAB_PX ? "ew-resize" : "grab";
  else el.canvas.style.cursor = "default";

  if (hit) showTooltip(hit.note, hit.effective, x, y);
  else if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
}

el.canvas.addEventListener("mouseenter", () => {
  overRoll = true;
  redraw(); // reveal the occupied-pitch highlight
});
el.canvas.addEventListener("mouseleave", () => {
  overRoll = false;
  if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
  redraw(); // hide the highlight
});

function canvasPoint(e: MouseEvent): { x: number; y: number } {
  const rect = el.canvas.getBoundingClientRect();
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function showSelectedTooltip(): void {
  const box = noteHitboxes.find((b) => b.note === selectedNote);
  if (box) showTooltip(box.note, box.effective, box.x + box.w / 2, box.y);
}

function hitTest(px: number, py: number): Hitbox | null {
  for (let i = noteHitboxes.length - 1; i >= 0; i--) {
    const b = noteHitboxes[i];
    if (px >= b.x && px <= b.x + b.w && py >= b.y && py <= b.y + b.h) return b;
  }
  return null;
}

function showTooltip(note: WorkingNote, effective: EffectiveNote | undefined, px: number, py: number): void {
  const weakest = weakestSubScore(note.sub_scores);
  const rows = Object.entries(note.sub_scores)
    .map(([k, v]) => `<dt>${k.replace(/_/g, " ")}</dt><dd>${fmt(v)}</dd>`)
    .join("");
  const cue = effective?.escaped
    ? `<div class="reason" style="color:${ESCAPE_COLOR}">↷ off-grid — kept (escaped)</div>`
    : // Only warn if the weakest dimension is actually shaky — a perfect note
        // (all sub-scores 1.00, e.g. a manual one) needs no ⚠ reason.
        weakest && weakest.value < 1
        ? `<div class="reason">⚠ ${weakest.label} (${fmt(weakest.value)})</div>`
        : "";
  const key = currentKey();
  const keyCue = effective?.outOfKey
    ? `<div class="reason" style="color:${OUT_OF_KEY_COLOR}">◇ out of key${key ? ` (${keyName(key)})` : ""}</div>`
    : effective && effective.pitch !== note.midi_pitch
      ? `<div class="reason" style="color:${OUT_OF_KEY_COLOR}">↳ snapped to ${midiToNoteName(effective.pitch)}</div>`
      : "";
  el.tooltip.innerHTML = `
    <div class="pitch">${midiToNoteName(note.midi_pitch)} · conf ${fmt(note.confidence)}${note.manual ? " · manual" : ""}</div>
    <div>${fmt3(effective?.onset_s ?? note.onset_s)}s – ${fmt3(effective?.offset_s ?? note.offset_s)}s</div>
    ${cue}
    ${keyCue}
    <dl>${rows}</dl>`;
  el.tooltip.style.display = "block";
  const wrapW = el.canvas.getBoundingClientRect().width;
  const tipW = el.tooltip.offsetWidth;
  const left = px + tipW + 16 > wrapW ? px - tipW - 12 : px + 12;
  el.tooltip.style.left = `${Math.max(4, left)}px`;
  el.tooltip.style.top = `${py + 12}px`;
}

function fmt(v: number): string {
  return Number.isFinite(v) ? v.toFixed(2) : "–";
}
function fmt3(v: number): string {
  return Number.isFinite(v) ? v.toFixed(3) : "–";
}

// --- Save / export ----------------------------------------------------------

/** Trigger a browser download of `data` as `filename`. */
function download(data: BlobPart, filename: string, mime: string): void {
  const url = URL.createObjectURL(new Blob([data], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/** The review settings a saved session restores on load. */
interface SessionState {
  bpm: number;
  division: number;
  gridAnchor: number;
  flagThreshold: number;
  snapKey: boolean;
}

/**
 * The full editing session: the annotation (f0 contour, energy, per-note
 * confidence — all preserved) with `notes` replaced by the current working set
 * (raw seconds + `manual` flags) and the review settings appended.
 */
function buildSessionFile(): object {
  const session: SessionState = { bpm, division, gridAnchor, flagThreshold, snapKey };
  return { ...annotation, notes: workingNotes, session };
}

/**
 * Save the session to a JSON file. Loading it via the annotation input restores
 * everything, unlike a MIDI export which keeps only the snapped pitches/timings.
 */
function saveSession(): void {
  if (!annotation) return;
  download(JSON.stringify(buildSessionFile()), "sona-session.json", "application/json");
  dirty = false;
  setStatus(`Saved session: ${workingNotes.length} notes → sona-session.json.`);
}

// --- Autosave (localStorage) ------------------------------------------------

const AUTOSAVE_KEY = "sona.autosave.v1";
const AUTOSAVE_MS = 20_000;

function markDirty(): void {
  dirty = true;
}

/**
 * Persist the session to localStorage (not a download — that would spam the
 * Downloads folder). Retries without the immutable f0/energy tracks if the full
 * session exceeds the storage quota, so at least the edits + settings survive.
 */
function autosave(): void {
  if (!annotation) return;
  const file = buildSessionFile();
  const savedAt = new Date().toISOString();
  const write = (payload: object) => localStorage.setItem(AUTOSAVE_KEY, JSON.stringify(payload));
  try {
    write({ savedAt, file });
  } catch {
    try {
      write({ savedAt, file: { ...file, f0_contour: [], energy_envelope: [] }, lite: true });
    } catch {
      return; // storage unavailable / still too big — give up quietly
    }
  }
  dirty = false;
  setStatus(`Autosaved ${new Date(savedAt).toLocaleTimeString()} · ${workingNotes.length} notes.`);
}

/** Restore the last autosaved session on startup, if one exists. */
function restoreAutosave(): void {
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(AUTOSAVE_KEY);
  } catch {
    return;
  }
  if (!raw) return;
  try {
    const record = JSON.parse(raw);
    applyLoadedJson(record.file);
    dirty = false;
    const when = record.savedAt ? new Date(record.savedAt).toLocaleString() : "earlier";
    setStatus(
      `Restored autosave (${workingNotes.length} notes) from ${when}. Re-load the stem to play.`,
    );
  } catch (err) {
    console.error("sona: could not restore autosave", err);
  }
}

// Tick every 20s; only writes when there are unsaved edits. Also flush on close.
window.setInterval(() => {
  if (dirty) autosave();
}, AUTOSAVE_MS);
window.addEventListener("beforeunload", () => {
  if (dirty) autosave();
});

/** Restore review settings from a saved session's `session` block. */
function restoreSession(s: any): void {
  if (Number(s.bpm) > 0) bpm = Number(s.bpm);
  el.bpm.value = String(bpm);
  const div = Number(s.division);
  if ([4, 8, 16, 32].includes(div)) {
    division = div;
    el.division.value = String(div);
  }
  gridAnchor = Number.isFinite(Number(s.gridAnchor))
    ? Number(s.gridAnchor)
    : workingNotes.length
      ? Math.min(...workingNotes.map((n) => n.onset_s))
      : 0;
  if (Number.isFinite(Number(s.flagThreshold))) {
    flagThreshold = Math.min(1, Math.max(0, Number(s.flagThreshold)));
    el.flag.value = String(flagThreshold);
    el.flagVal.textContent = flagThreshold.toFixed(2);
  }
  snapKey = s.snapKey === true;
  el.snapKey.checked = snapKey;
}

el.save.addEventListener("click", saveSession);

/**
 * Export the authoritative MIDI (ADR-0004): the current snapped + corrected
 * state, seconds→ticks at the chosen BPM. Escaped notes export at their raw
 * position, as shown.
 */
function exportMidi(): void {
  if (!annotation) return;
  const notes = midiNotes();
  download(encodeMidiFile(notes, { bpm }) as BlobPart, "sona.mid", "audio/midi");
  setStatus(`Exported ${notes.length} notes to sona.mid at ${bpm} BPM.`);
}

// --- Playback ---------------------------------------------------------------

function ensureAudioContext(): AudioContext {
  if (!audioCtx) audioCtx = new AudioCtx();
  return audioCtx;
}

el.play.addEventListener("click", () => {
  if (!annotation || !audioBuffer || playback) return;
  startPlayback();
});
// The transport's "stop" is a pause: it halts audio and leaves the playhead
// where it was, so the next Play resumes from there.
el.stop.addEventListener("click", () => stopPlayback());

/** Space toggles play/pause. */
function togglePlay(): void {
  if (playback) stopPlayback();
  else if (annotation && audioBuffer) startPlayback();
}

el.muteStem.addEventListener("change", () => {
  if (playback) playback.stemGain.gain.value = el.muteStem.checked ? 0 : 1;
});
el.muteSynth.addEventListener("change", () => {
  if (playback) playback.synthGain.gain.value = el.muteSynth.checked ? 0 : 1;
});

function startPlayback(): void {
  if (!audioBuffer) return;
  const ac = ensureAudioContext();
  ac.resume();
  const startedAt = ac.currentTime + 0.05;
  // Both sources are placed on this shared clock as if the song started at t=0,
  // then fast-forwarded to `seekOffset` — so they stay in sync from the seek.
  const from = Math.min(seekOffset, totalDuration());

  const stemGain = ac.createGain();
  stemGain.gain.value = el.muteStem.checked ? 0 : 1;
  stemGain.connect(ac.destination);
  const stemSource = ac.createBufferSource();
  stemSource.buffer = audioBuffer;
  stemSource.connect(stemGain);
  // Start the buffer partway in (clamped to its length so we never seek past end).
  stemSource.start(startedAt, Math.min(from, audioBuffer.duration));

  // Synth the snapped/corrected notes, so you hear the interpretation, not the raw.
  const synthGain = ac.createGain();
  synthGain.gain.value = el.muteSynth.checked ? 0 : 1;
  synthGain.connect(ac.destination);
  const voices: OscillatorNode[] = [];
  for (const voice of buildSchedule(midiNotes())) {
    const osc = scheduleVoice(ac, synthGain, voice, startedAt, from);
    if (osc) voices.push(osc);
  }

  const duration = totalDuration();
  playback = { startedAt, duration, from, stemSource, stemGain, synthGain, voices, raf: 0 };
  stemSource.onended = () => {
    // Natural end: reset the playhead to the start so the next Play replays.
    if (playback && playback.stemSource === stemSource) stopPlayback(true);
  };

  el.play.disabled = true;
  el.stop.disabled = false;
  tickPlayhead();
}

/**
 * One oscillator + envelope per note, on the shared clock, offset so the song
 * effectively starts at `from` seconds. Notes entirely before the seek are
 * skipped; a note straddling it starts immediately for its remainder.
 */
function scheduleVoice(
  ac: AudioContext,
  out: GainNode,
  voice: { freq: number; start: number; duration: number; gain: number },
  startedAt: number,
  from: number,
): OscillatorNode | null {
  const voiceEnd = voice.start + voice.duration;
  if (voiceEnd <= from) return null; // fully before the seek point
  const t0 = startedAt + Math.max(0, voice.start - from);
  const t1 = startedAt + (voiceEnd - from);
  const osc = ac.createOscillator();
  osc.type = "triangle";
  osc.frequency.value = voice.freq;
  const g = ac.createGain();
  g.gain.setValueAtTime(0, t0);
  g.gain.linearRampToValueAtTime(voice.gain, t0 + 0.005);
  g.gain.setValueAtTime(voice.gain, Math.max(t0 + 0.005, t1 - 0.01));
  g.gain.linearRampToValueAtTime(0, t1);
  osc.connect(g);
  g.connect(out);
  osc.start(t0);
  osc.stop(t1 + 0.02);
  return osc;
}

/**
 * Halt playback. Crucially this also stops every scheduled synth oscillator —
 * they're scheduled ahead on the audio clock, so without stopping them they'd
 * keep sounding after a pause and stack up on the next Play. `resetSeek` sends
 * the playhead back to the start (used on natural end); otherwise it stays put
 * so Play resumes from where it paused.
 */
function stopPlayback(resetSeek = false): void {
  if (!playback) return;
  const p = playback;
  const pos = currentPlayheadS();
  playback = null;
  try {
    p.stemSource.onended = null;
    p.stemSource.stop();
  } catch {
    // Already stopped/ended — fine.
  }
  for (const osc of p.voices) {
    try {
      osc.stop();
      osc.disconnect();
    } catch {
      // Already ended — fine.
    }
  }
  try {
    p.synthGain.disconnect();
  } catch {
    // Fine.
  }
  cancelAnimationFrame(p.raf);
  seekOffset = resetSeek ? 0 : Math.min(pos, totalDuration());
  el.play.disabled = !(annotation && audioBuffer);
  el.stop.disabled = true;
  draw();
}

/** Current song time of the playhead: seek offset + elapsed on the audio clock. */
function currentPlayheadS(): number {
  if (!playback) return seekOffset;
  return playback.from + Math.max(0, ensureAudioContext().currentTime - playback.startedAt);
}

function tickPlayhead(): void {
  if (!playback) return;
  if (currentPlayheadS() >= playback.duration) {
    stopPlayback(true); // reached the end → back to the start for the next play
    return;
  }
  followPlayhead();
  draw();
  playback.raf = requestAnimationFrame(tickPlayhead);
}

/**
 * Keep the moving playhead on screen: when it runs past the right edge (or left)
 * of the visible plot, page the horizontal scroll so it lands back near the left
 * with lead room. Only meaningful when zoomed in (there's something to scroll).
 */
function followPlayhead(): void {
  if (!scale || maxScrollX() <= 0) return;
  const w = cssSize().w;
  const x = scale.timeToX(currentPlayheadS());
  const rightEdge = w - 24;
  if (x > rightEdge || x < PAD.left) {
    const dur = totalDuration();
    const content = plotWidthPx() * zoomX;
    const lead = 0.12 * plotWidthPx(); // land the playhead a little in from the gutter
    scrollX = clampScroll((currentPlayheadS() / dur) * content - lead);
    syncScale();
  }
}

// --- Misc -------------------------------------------------------------------

function setStatus(msg: string, isError = false): void {
  el.status.textContent = msg;
  el.status.classList.toggle("error", isError);
}

// First paint (placeholder) once the layout has a size, then restore any
// autosaved session so a reload/crash resumes where you left off. localStorage
// can lag a frame behind a reload in sandboxed environments, so retry briefly
// until it's readable (and stop if the user loads a file in the meantime).
function attemptRestore(tries = 0): void {
  if (annotation) return;
  let raw: string | null = null;
  try {
    raw = localStorage.getItem(AUTOSAVE_KEY);
  } catch {
    return; // storage unavailable
  }
  if (raw) restoreAutosave();
  else if (tries < 8) setTimeout(() => attemptRestore(tries + 1), 120);
}
// setTimeout, not requestAnimationFrame: rAF is paused while the tab is
// backgrounded, which would leave the initial paint and autosave restore
// hanging until the tab is focused.
setTimeout(() => {
  rebuild();
  attemptRestore();
}, 0);
