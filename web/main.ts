// sona review surface — browser layer.
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
import { detectKey, keyName } from "./lib/key";
import type { KeyEstimate } from "./lib/key";
import { buildSchedule } from "./lib/synth";
import { encodeMidiFile } from "./lib/midi";
import { assignLyrics, splitLyrics } from "./lib/lyrics";
import { TICKS_PER_QUARTER, exportSequence, sequenceInSeconds, type Sequence } from "./lib/sequence";
import { barNumber, barOneLabel, barSeconds, downbeat, gridLines, tickZero, updateFrame, type Frame, type FrameEvent, type LineKind } from "./lib/frame";
import { effectiveNotes as interpret, exportNotes, noteSpan, notesInTimeOrder as inTimeOrder, playbackNotes, renderNotes, type EffectiveNote } from "./lib/interpret";
import { movedNote, newNote, resizedNote, withoutNote, type Grabbed } from "./lib/noteEdits";
import { docText, isSession, normalizeAnnotation, parseDoc, baselineFrom, sessionFile, settingsOnLoad, workingNotesFrom, type SessionState } from "./lib/session";
import { encodeUst } from "./lib/ust";
import { ustxText } from "./lib/ustx";
import { sungPitchCurve } from "./lib/sungPitch";
import { penErase, penStroke } from "./lib/pitchPen";
import { packContour } from "./lib/contourStore";
import { detailSections } from "./lib/provenance";
import { DEFAULT_VIBRATO, NEUTRAL_EXPRESSION, renderPitch, type Expression } from "./lib/expression";
import { fitExpressions } from "./lib/fitexpr";
import { addPoint, dragHandle, handlesFor, removePoint, type Handle, type NoteSpan } from "./lib/pitchHandles";
import { PHASE_LABEL, cancelJob, defaultJob, estimateTempo, fetchCapabilities, fetchJobOrCancel, fetchResults, isFinal, submitJob, type Capabilities, type Engine, type JobStatus, type Separation } from "./lib/serveClient";
import { hintBpm, parseBpm, phaseProgress, startStep, usableBpm, type StartEvent, type StartState } from "./lib/startFlow";
import { SharedSnapshot, Undo } from "./lib/undo";
import { correctionCost, correctionChanges } from "./lib/correction";
import type { NoteLike, Change, ChangeKind } from "./lib/correction";
import type { Annotation, F0Frame, Note, WorkingNote } from "./lib/annotation";

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
  muteStem: document.getElementById("mute-stem") as HTMLInputElement,
  muteSynth: document.getElementById("mute-synth") as HTMLInputElement,
  bpm: document.getElementById("bpm") as HTMLInputElement,
  division: document.getElementById("division") as HTMLSelectElement,
  flag: document.getElementById("flag") as HTMLInputElement,
  flagVal: document.getElementById("flag-val") as HTMLOutputElement,
  keyReadout: document.getElementById("key-readout") as HTMLOutputElement,
  barOneReadout: document.getElementById("bar-one-readout") as HTMLOutputElement,
  barOneHere: document.getElementById("bar-one-here") as HTMLButtonElement,
  barOneAuto: document.getElementById("bar-one-auto") as HTMLButtonElement,
  keyDetail: document.getElementById("key-detail") as HTMLOutputElement,
  correctionOpen: document.getElementById("correction-open") as HTMLButtonElement,
  correctionBadge: document.getElementById("correction-badge") as HTMLSpanElement,
  changesOverlay: document.getElementById("changes-overlay") as HTMLDivElement,
  changesClose: document.getElementById("changes-close") as HTMLButtonElement,
  changesList: document.getElementById("changes-list") as HTMLUListElement,
  changesSummary: document.getElementById("changes-summary") as HTMLParagraphElement,
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
  lyrics: document.getElementById("lyrics") as HTMLButtonElement,
  lyricsOverlay: document.getElementById("lyrics-overlay") as HTMLDivElement,
  lyricsClose: document.getElementById("lyrics-close") as HTMLButtonElement,
  lyricsText: document.getElementById("lyrics-text") as HTMLTextAreaElement,
  lyricsPreview: document.getElementById("lyrics-preview") as HTMLParagraphElement,
  lyricsFromSelected: document.getElementById("lyrics-from-selected") as HTMLButtonElement,
  lyricsFromStart: document.getElementById("lyrics-from-start") as HTMLButtonElement,
  lyricsClear: document.getElementById("lyrics-clear") as HTMLButtonElement,
  lyricEditor: document.getElementById("lyric-editor") as HTMLInputElement,
  exportUst: document.getElementById("export-ust") as HTMLButtonElement,
  exportUstx: document.getElementById("export-ustx") as HTMLButtonElement,
  exportUstxSung: document.getElementById("export-ustx-sung") as HTMLButtonElement,
  fitPitch: document.getElementById("fit-pitch") as HTMLButtonElement,
  showCurve: document.getElementById("show-curve") as HTMLInputElement,
  pitchPen: document.getElementById("pitch-pen") as HTMLInputElement,
  exprBar: document.getElementById("expr-bar") as HTMLDivElement,
  glideStart: document.getElementById("glide-start") as HTMLInputElement,
  glideEnd: document.getElementById("glide-end") as HTMLInputElement,
  vibOn: document.getElementById("vib-on") as HTMLInputElement,
  vibDepth: document.getElementById("vib-depth") as HTMLInputElement,
  vibPeriod: document.getElementById("vib-period") as HTMLInputElement,
  vibLength: document.getElementById("vib-length") as HTMLInputElement,
  vibIn: document.getElementById("vib-in") as HTMLInputElement,
  vibOut: document.getElementById("vib-out") as HTMLInputElement,
  exprFit: document.getElementById("expr-fit") as HTMLButtonElement,
  exprReset: document.getElementById("expr-reset") as HTMLButtonElement,
  generate: document.getElementById("generate") as HTMLButtonElement,
  openAnnotation: document.getElementById("open-annotation") as HTMLButtonElement,
  openStem: document.getElementById("open-stem") as HTMLButtonElement,
  editLyric: document.getElementById("edit-lyric") as HTMLButtonElement,
  editVibrato: document.getElementById("edit-vibrato") as HTMLButtonElement,
  editDelete: document.getElementById("edit-delete") as HTMLButtonElement,
  start: document.getElementById("start") as HTMLDivElement,
  startOpen: document.getElementById("start-open") as HTMLButtonElement,
  startResume: document.getElementById("start-resume") as HTMLButtonElement,
  startResumeDetail: document.getElementById("start-resume-detail") as HTMLSpanElement,
  startBack: document.getElementById("start-back") as HTMLButtonElement,
  drop: document.getElementById("drop") as HTMLLabelElement,
  songFile: document.getElementById("song-file") as HTMLInputElement,
  dropTitle: document.getElementById("drop-title") as HTMLElement,
  dropSub: document.getElementById("drop-sub") as HTMLSpanElement,
  songReady: document.getElementById("song-ready") as HTMLDivElement,
  songName: document.getElementById("song-name") as HTMLElement,
  songChange: document.getElementById("song-change") as HTMLButtonElement,
  songBpm: document.getElementById("song-bpm") as HTMLInputElement,
  songHintText: document.getElementById("song-hint-text") as HTMLSpanElement,
  songHintFull: document.getElementById("song-hint-full") as HTMLButtonElement,
  songHintHalf: document.getElementById("song-hint-half") as HTMLButtonElement,
  songMore: document.getElementById("song-more") as HTMLDetailsElement,
  songGrid: document.getElementById("song-grid") as HTMLSelectElement,
  songSeparation: document.getElementById("song-separation") as HTMLSelectElement,
  songEngine: document.getElementById("song-engine") as HTMLSelectElement,
  songBarOne: document.getElementById("song-bar-one") as HTMLInputElement,
  songGo: document.getElementById("song-go") as HTMLButtonElement,
  songError: document.getElementById("song-error") as HTMLParagraphElement,
  songRunning: document.getElementById("song-running") as HTMLDivElement,
  songRunningName: document.getElementById("song-running-name") as HTMLElement,
  songCancel: document.getElementById("song-cancel") as HTMLButtonElement,
  songBar: document.getElementById("song-bar") as HTMLDivElement,
  songPhase: document.getElementById("song-phase") as HTMLParagraphElement,
  songLog: document.getElementById("song-log") as HTMLPreElement,
  setupNotes: document.getElementById("setup-notes") as HTMLUListElement,
  tempoChip: document.getElementById("tempo-chip") as HTMLButtonElement,
  tempoChipText: document.getElementById("tempo-chip-text") as HTMLSpanElement,
  tempoPop: document.getElementById("tempo-pop") as HTMLDivElement,
  bpmHalf: document.getElementById("bpm-half") as HTMLButtonElement,
  bpmDouble: document.getElementById("bpm-double") as HTMLButtonElement,
  exprVib: document.getElementById("expr-vib") as HTMLButtonElement,
  exprExactToggle: document.getElementById("expr-exact-toggle") as HTMLButtonElement,
  exprExact: document.getElementById("expr-exact") as HTMLSpanElement,
  details: document.getElementById("details") as HTMLButtonElement,
  editUndo: document.getElementById("edit-undo") as HTMLButtonElement,
  editRedo: document.getElementById("edit-redo") as HTMLButtonElement,
  detailsOverlay: document.getElementById("details-overlay") as HTMLDivElement,
  detailsClose: document.getElementById("details-close") as HTMLButtonElement,
  detailsBody: document.getElementById("details-body") as HTMLDivElement,
  detailsDownload: document.getElementById("details-download") as HTMLButtonElement,
};
for (const [k, v] of Object.entries(el)) if (!v) throw new Error(`index.html is missing the element for ${k}`);
const ctx2d = el.canvas.getContext("2d")!;

// --- Types -----------------------------------------------------------------

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
  | (DragBase & { mode: "offset"; origFrame: Frame })
  | (DragBase & { mode: "handle"; note: WorkingNote; handle: Handle; origExpr: Expression; span: NoteSpan })
  | (DragBase & { mode: "pen"; lastT: number; lastCents: number; erase: boolean })
  | (DragBase & {
      mode: "move" | "resize";
      note: WorkingNote;
      orig: Grabbed;
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
/**
 * The producer's original notes, before any correction — the baseline the
 * correction cost is measured against. Cloned from the notes on first load (so
 * editing `workingNotes` never mutates it) and round-tripped through the
 * session's `baseline` field so a reopened session still knows where it started.
 *
 * Kept as full [`Note`]s (confidence + sub-scores, not just timing/pitch): a
 * saved session then pairs every producer note's scores with whether the user
 * corrected it, which is exactly the labelled data confidence calibration needs.
 */
let baselineNotes: Note[] = [];
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

// Musical interpretation. Seeded on load; edited via the toolbar.
let bpm = 120;
let division = 16;
let gridAnchor = 0; // grid phase in seconds (auto = first onset, draggable).
// A downbeat the user chose (ADR-0029); null = bar 1 on the first grid line.
let barOne: number | null = null;
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

function loadAnnotationJson(parsed: unknown, stemFollows = false): boolean {
  try {
    const session = applyLoadedJson(parsed);
    setStatus(`${workingNotes.length} notes` + (session || audioBuffer || stemFollows ? "" : " · File → Open stem audio to hear them"));
    dirty = false; // a freshly loaded file is not unsaved work
    return true;
  } catch (err) {
    annotation = null;
    workingNotes = [];
    setStatus(`Could not read annotation JSON: ${(err as Error).message}`, true);
    rebuild();
    edits.reset();
    syncUndoMenu();
    return false;
  }
}

el.json.addEventListener("change", async () => {
  const file = el.json.files?.[0];
  // Cleared so choosing the same file again still fires `change` (reopening means starting over).
  el.json.value = "";
  if (!file) return;
  let parsed: unknown;
  try {
    parsed = JSON.parse(await file.text());
  } catch (err) {
    setStatus(`Could not read annotation JSON: ${(err as Error).message}`, true);
    return;
  }
  loadAnnotationJson(parsed);
});

/**
 * Apply a parsed annotation/session object to the editing state (used by the
 * file loader and autosave restore). Returns whether it carried a `session`
 * block. Does not set a filename-specific status.
 */
function applyLoadedJson(parsed: any): boolean {
  annotation = normalizeAnnotation(parsed);
  workingNotes = workingNotesFrom(parsed.notes);
  baselineNotes = baselineFrom(parsed);

  selectedNote = null;
  seekOffset = 0;
  invalidateEffective();

  applySettings(settingsOnLoad(parsed, annotation, workingNotes, currentSettings()));
  resetView(); // zoomed-in default + 2-octave window centred on the notes.
  rebuild();
  edits.reset();
  syncUndoMenu();
  return isSession(parsed);
}

async function loadStemBytes(bytes: ArrayBuffer): Promise<void> {
  try {
    audioBuffer = await ensureAudioContext().decodeAudioData(bytes);
    setStatus(`Audio loaded`);
    rebuild();
  } catch (err) {
    audioBuffer = null;
    setStatus(`Could not decode audio: ${(err as Error).message}`, true);
    rebuild();
  }
}

el.audio.addEventListener("change", async () => {
  const file = el.audio.files?.[0];
  el.audio.value = "";
  if (file) await loadStemBytes(await file.arrayBuffer());
});

// --- Musical interpretation (grid) -----------------------------------------

/** The frame the editor draws and the export writes (ADR-0029). */
function currentFrame(): Frame {
  return { bpm, division, anchor: gridAnchor, barOne };
}

function changeFrame(e: FrameEvent): void {
  const f = updateFrame(currentFrame(), e);
  bpm = f.bpm;
  division = f.division;
  gridAnchor = f.anchor;
  barOne = f.barOne;
  syncBarOne();
}

/** Stem time of the export's tick 0 for the notes as they are now. */
function exportTickZero(): number {
  return tickZero(currentFrame(), effectiveNotes().map((e) => e.onset_s));
}

function syncBarOne(): void {
  const estimate = annotation?.bar_one ?? null;
  el.barOneReadout.textContent = barOneLabel(currentFrame(), estimate);
  el.barOneAuto.disabled = barOne === (estimate?.time_s ?? null);
  el.barOneAuto.title = estimate ? "Bar 1 where sona estimated it from the music" : "Bar 1 on the first grid line, as before";
}

/** "This line is beat 1": the grid line nearest `t` becomes a bar line. */
function setBarOneAt(t: number): void {
  changeFrame({ type: "bar-one", t });
  markDirty();
  redraw();
  setStatus(`Bar lines now fall on ${downbeat(currentFrame()).toFixed(3)} s`);
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
 * Refresh the correction-cost readout: how far the working notes have drifted
 * from the producer's baseline. The total shows in the toolbar; the full
 * breakdown (added / deleted / re-pitched / re-timed) is the tooltip.
 */
function updateCorrectionReadout(): void {
  if (!annotation) {
    el.correctionOpen.disabled = true;
    el.correctionBadge.hidden = true;
    el.correctionOpen.title = "Manual corrections vs. the producer's original notes.";
    return;
  }
  const c = correctionCost(baselineNotes, workingNotes);
  el.correctionOpen.disabled = false;
  // The badge shows the count only when there is something to show.
  el.correctionBadge.textContent = String(c.total);
  el.correctionBadge.hidden = c.total === 0;
  el.correctionOpen.title =
    `${c.total} corrections vs. ${annotation.producer} — click to list them: ` +
    `${c.added} added, ${c.deleted} deleted, ${c.pitchFixed} re-pitched, ` +
    `${c.onsetMoved} onset + ${c.offsetMoved} offset moved.`;
}

/** Uppercase tag label + colour class for each kind of change, for the modal. */
const CHANGE_TAG: Record<ChangeKind, string> = {
  added: "Added",
  deleted: "Deleted",
  pitch: "Pitch",
  onset: "Onset",
  offset: "Offset",
};

/** The onset time a change is filed under, for sorting the list chronologically. */
function changeTime(c: Change): number {
  return (c.to ?? c.from)!.onset_s;
}

/** A one-line human description of a single correction. */
function describeChange(c: Change): string {
  const t = (s: number) => `${s.toFixed(2)}s`;
  const name = (nn: NoteLike) => midiToNoteName(nn.midi_pitch);
  switch (c.kind) {
    case "added":
      return `${name(c.to!)} · ${t(c.to!.onset_s)}`;
    case "deleted":
      return `${name(c.from!)} · ${t(c.from!.onset_s)}`;
    case "pitch":
      return `${name(c.from!)} → ${name(c.to!)} · ${t(c.to!.onset_s)}`;
    case "onset":
      return `${t(c.from!.onset_s)} → ${t(c.to!.onset_s)} · ${name(c.to!)}`;
    case "offset":
      return `end ${t(c.from!.offset_s)} → ${t(c.to!.offset_s)} · ${name(c.to!)}`;
  }
}

/** Populate and open the modal listing every change behind the Fixes count. */
function openChangesModal(): void {
  if (!annotation) return;
  const changes = correctionChanges(baselineNotes, workingNotes);
  const c = correctionCost(baselineNotes, workingNotes);

  el.changesSummary.textContent =
    changes.length === 0
      ? "No changes yet — the working notes match the producer's output."
      : `${c.total} change${c.total === 1 ? "" : "s"} vs. ${annotation.producer}: ` +
        `${c.added} added, ${c.deleted} deleted, ${c.pitchFixed} re-pitched, ` +
        `${c.onsetMoved} onset + ${c.offsetMoved} offset moved.`;

  const rows = changes
    .slice()
    .sort((a, b) => changeTime(a) - changeTime(b))
    .map((change) => {
      const li = document.createElement("li");
      const tag = document.createElement("span");
      tag.className = `tag ${change.kind}`;
      tag.textContent = CHANGE_TAG[change.kind];
      const desc = document.createElement("span");
      desc.className = "desc";
      desc.textContent = describeChange(change);
      li.append(tag, desc);
      return li;
    });
  el.changesList.replaceChildren(...rows);

  setModal(el.changesOverlay, true);
}

/**
 * The notes as interpreted for display, playback, and export (`lib/interpret`).
 * Cached: recomputing this per frame was a big part of the playback jank on
 * long stems.
 */
function effectiveNotes(): EffectiveNote[] {
  effectiveCache ??= interpret(workingNotes, currentFrame(), { flagThreshold, snapKey, key: currentKey() });
  return effectiveCache;
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

function setBpm(v: number): void {
  if (!(v > 0 && Number.isFinite(v))) return;
  bpm = v;
  invalidateEffective();
  markDirty("bpm");
  syncTempoChip();
  redraw();
}

el.bpm.addEventListener("input", () => setBpm(Number(el.bpm.value)));
el.division.addEventListener("change", () => {
  division = Number(el.division.value);
  invalidateEffective();
  markDirty();
  syncTempoChip();
  redraw();
});

function syncTempoChip(): void {
  const grid = el.division.selectedOptions[0]?.textContent ?? `1/${division}`;
  el.tempoChipText.textContent = `${Math.round(bpm * 10) / 10} · ${grid}`;
  syncBarOne();
}

function setTempoPop(open: boolean): void {
  el.tempoPop.hidden = !open;
  el.tempoChip.setAttribute("aria-expanded", String(open));
  if (open) el.bpm.focus();
}

function scaleBpm(k: number): void {
  const v = Math.round(bpm * k * 10) / 10;
  if (!usableBpm(v)) return;
  el.bpm.value = String(v);
  setBpm(v);
}

el.tempoChip.addEventListener("click", () => setTempoPop(el.tempoPop.hidden));
el.bpmHalf.addEventListener("click", () => scaleBpm(0.5));
el.bpmDouble.addEventListener("click", () => scaleBpm(2));
el.tempoPop.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.stopPropagation();
    setTempoPop(false);
    el.tempoChip.focus();
  }
});
document.addEventListener("pointerdown", (e) => {
  if (!el.tempoPop.hidden && !(e.target as Element).closest(".tempo")) setTempoPop(false);
});
el.flag.addEventListener("input", () => {
  flagThreshold = Number(el.flag.value);
  el.flagVal.textContent = flagThreshold.toFixed(2);
  invalidateEffective();
  markDirty("flag");
  redraw();
});
el.snapKey.addEventListener("change", () => {
  snapKey = el.snapKey.checked;
  invalidateEffective(); // snapping changes note pitches
  markDirty();
  redraw();
});
el.export.addEventListener("click", exportMidi);
el.barOneHere.addEventListener("click", () => setBarOneAt(currentPlayheadS()));
el.barOneAuto.addEventListener("click", () => {
  const estimate = annotation?.bar_one ?? null;
  changeFrame({ type: "auto-bar-one", estimate: estimate?.time_s ?? null });
  markDirty();
  redraw();
  setStatus(estimate ? "Bar 1 is on the estimate from the music again" : "Bar 1 is on the first grid line again");
});

// --- Modals (help + settings) -----------------------------------------------

function setModal(overlay: HTMLElement, open: boolean): void {
  // Only one modal at a time; opening one closes the others.
  if (open) {
    el.helpOverlay.hidden = true;
    el.settingsOverlay.hidden = true;
    el.changesOverlay.hidden = true;
    el.lyricsOverlay.hidden = true;
    el.detailsOverlay.hidden = true;
  }
  overlay.hidden = !open;
}
function anyModalOpen(): boolean {
  return [el.helpOverlay, el.settingsOverlay, el.changesOverlay, el.lyricsOverlay, el.detailsOverlay].some((o) => !o.hidden);
}

el.help.addEventListener("click", () => setModal(el.helpOverlay, el.helpOverlay.hidden));
el.helpClose.addEventListener("click", () => setModal(el.helpOverlay, false));
el.helpOverlay.addEventListener("click", (e) => {
  if (e.target === el.helpOverlay) setModal(el.helpOverlay, false);
});

el.correctionOpen.addEventListener("click", openChangesModal);
el.changesClose.addEventListener("click", () => setModal(el.changesOverlay, false));
el.changesOverlay.addEventListener("click", (e) => {
  if (e.target === el.changesOverlay) setModal(el.changesOverlay, false);
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
  updateCorrectionReadout();
  draw();
  syncPlayButton();
  el.tempoChip.disabled = !annotation;
  el.play.hidden = !annotation;
  el.tempoChip.hidden = !annotation;
  syncTempoChip();
  el.start.hidden = !!annotation && !startShown;
  el.startBack.hidden = !annotation;
  el.export.disabled = !annotation;
  el.exportUst.disabled = !annotation;
  el.exportUstx.disabled = !annotation;
  el.exportUstxSung.disabled = !annotation?.f0_contour.length;
  el.pitchPen.disabled = !annotation?.f0_contour.length;
  if (el.pitchPen.disabled) el.pitchPen.checked = false;
  el.lyrics.disabled = !annotation;
  el.fitPitch.disabled = !annotation || !annotation.f0_contour.length;
  syncExprBar();
  el.save.disabled = !annotation;
  el.details.disabled = !annotation;
}

/** Redraw after a view/grid/edit change: rebuild the scale (not the canvas). */
function redraw(): void {
  syncScale();
  updateCorrectionReadout();
  draw();
  syncExprBar();
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
// The roll also changes size without the window doing so (the expression bar
// appears under it when a note is selected): keep the backing store in step,
// or drawing and pointer coordinates drift apart.
new ResizeObserver(() => {
  const r = el.canvas.getBoundingClientRect();
  const dpr = devicePixelRatioSafe();
  if (Math.round(r.width * dpr) !== el.canvas.width || Math.round(r.height * dpr) !== el.canvas.height) rebuild();
}).observe(el.canvas);

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
  if (el.showCurve.checked) {
    drawExpressionCurve();
    drawPitchHandles();
  }
  // currentPlayheadS() returns the seek offset when stopped, so this covers both.
  drawPlayhead(currentPlayheadS(), h, !playback);
  ctx2d.restore();
}

function drawPlaceholder(w: number, h: number): void {
  ctx2d.fillStyle = "#5b6072";
  ctx2d.font = "13px system-ui, sans-serif";
  ctx2d.textAlign = "center";
  ctx2d.fillText("Start a project, or open files from the File menu.", w / 2, h / 2);
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
 * The frame's grid lines *within the visible time window*: screen x, whether
 * each is a cell, beat or bar line, and its time. Culled to `[viewT0, viewT1]`
 * so a fine grid over a long stem only produces the lines actually on screen.
 */
function gridLineInfo(): { x: number; t: number; kind: LineKind }[] {
  return gridLines(currentFrame(), viewT0, Math.min(totalDuration(), viewT1)).map((l) => ({ x: scale!.timeToX(l.t), t: l.t, kind: l.kind }));
}

const GRID_LINE_COLOUR: Record<LineKind, string> = { cell: "#1c2029", beat: "#2f3646", bar: "#4a5470" };
const RULER_TICK: Record<LineKind, { colour: string; top: number }> = {
  cell: { colour: "#3a4050", top: RULER_H - 7 },
  beat: { colour: "#6c7488", top: RULER_H - 13 },
  bar: { colour: "#a9b0c2", top: 3 },
};

/** Vertical grid lines from the frame; beat lines stronger, bar lines strongest. */
function drawTimeGrid(_w: number, h: number): void {
  for (const { x, kind } of gridLineInfo()) {
    ctx2d.strokeStyle = GRID_LINE_COLOUR[kind];
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

/** A tick on each grid line in the ruler, and OpenUtau's bar number beside each bar line (clipped to the plot area). */
function drawRulerTicks(): void {
  const lines = gridLineInfo();
  const t0 = exportTickZero();
  const frame = currentFrame();
  const barPx = scale!.timeToX(barSeconds(frame)) - scale!.timeToX(0);
  const every = barPx >= 28 ? 1 : 2 ** Math.ceil(Math.log2(28 / Math.max(1, barPx)));
  ctx2d.font = '9px "Space Mono", ui-monospace, monospace';
  for (const { x, t, kind } of lines) {
    const tick = RULER_TICK[kind];
    ctx2d.strokeStyle = tick.colour;
    ctx2d.beginPath();
    ctx2d.moveTo(x, tick.top);
    ctx2d.lineTo(x, RULER_H - 1);
    ctx2d.stroke();
    const bar = barNumber(frame, t, t0);
    if (kind === "bar" && (bar - 1) % every === 0) {
      ctx2d.fillStyle = "#a9b0c2";
      ctx2d.fillText(String(bar), x + 3, 11);
    }
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
// (ADR-0004). It replaces the display role of the earlier slope filter.
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

    if (eff.raw.lyric && wNote >= 12 && barH >= 9) {
      ctx2d.save();
      roundRect(x, y, wNote, barH, NOTE_RADIUS);
      ctx2d.clip();
      ctx2d.fillStyle = "#0b0b0b";
      ctx2d.font = `600 ${Math.min(13, Math.max(9, barH - 3))}px "Outfit", system-ui, sans-serif`;
      ctx2d.textBaseline = "middle";
      ctx2d.fillText(eff.raw.lyric, x + 4, y + barH / 2 + 0.5);
      ctx2d.restore();
    }

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

  // The top ruler: a click seeks the playhead, a drag shifts the grid phase,
  // and a Shift-click makes the nearest grid line beat 1 of a bar.
  if (y < RULER_H) {
    if (e.shiftKey) {
      setBarOneAt(scale.xToTime(x));
      return;
    }
    edits.beginGesture();
    drag = { mode: "offset", startX: x, startY: y, origFrame: currentFrame(), moved: false };
    return;
  }

  if (el.pitchPen.checked && annotation?.f0_contour.length) {
    const t = scale.xToTime(x);
    const c = scale.yToPitchExact(y) * 100;
    edits.beginGesture();
    drag = { mode: "pen", lastT: t, lastCents: c, erase: e.altKey, startX: x, startY: y, moved: false };
    if (e.altKey) penErase(annotation.f0_contour, t, t);
    else penStroke(annotation.f0_contour, t, c, t, c);
    markDirty();
    redraw();
    return;
  }

  const h = handleAtPoint(x, y);
  if (h && selectedNote) {
    if (el.exprBar.contains(document.activeElement)) {
      applyExprBar();
      (document.activeElement as HTMLElement).blur();
    }
    if (e.altKey && h.handle.kind === "point") {
      selectedNote.expr = removePoint(exprOf(selectedNote), h.handle.index);
      markDirty();
      redraw();
      return;
    }
    edits.beginGesture();
    drag = { mode: "handle", note: selectedNote, handle: h.handle, origExpr: exprOf(selectedNote), span: h.span, startX: x, startY: y, moved: false };
    el.canvas.style.cursor = "grabbing";
    return;
  }

  const hit = hitTest(x, y);
  if (hit) {
    selectedNote = hit.note;
    const onRightEdge = x >= hit.x + hit.w - RESIZE_GRAB_PX;
    edits.beginGesture();
    drag = {
      mode: onRightEdge ? "resize" : "move",
      note: hit.note,
      startX: x,
      startY: y,
      orig: { onset_s: hit.note.onset_s, offset_s: hit.note.offset_s, midi_pitch: hit.note.midi_pitch },
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

  if (d.mode === "pen") {
    if (!annotation) return;
    const t = scale.xToTime(x);
    const c = scale.yToPitchExact(y) * 100;
    if (d.erase) penErase(annotation.f0_contour, d.lastT, t);
    else penStroke(annotation.f0_contour, d.lastT, d.lastCents, t, c);
    d.lastT = t;
    d.lastCents = c;
    markDirty();
    redraw();
    return;
  }

  if (d.mode === "handle") {
    const dCents = (-(y - d.startY) / scale.rowHeight) * 100;
    d.note.expr = dragHandle(d.origExpr, d.span, d.handle, dt, dCents);
    markDirty();
    redraw();
    return;
  }

  if (d.mode === "offset") {
    // Only shift the grid once it's really a drag; a sub-threshold click stays a
    // seek (handled on mouseup) with the phase untouched.
    if (d.moved) {
      changeFrame({ type: "drag", from: d.origFrame, dt });
      invalidateEffective(); // snapping depends on the phase
      markDirty();
      redraw();
    }
    return;
  }

  const n = d.note;
  if (d.mode === "move") Object.assign(n, movedNote(d.orig, dt, scale.yToPitch(y) - scale.yToPitch(d.startY)));
  else Object.assign(n, resizedNote(n.onset_s, d.orig, dt, currentFrame()));
  invalidateEffective(); // the note's quantized position changed
  markDirty();
  redraw();
  showSelectedTooltip();
});

window.addEventListener("mouseup", () => {
  const d = drag;
  drag = null;
  if (d) {
    edits.endGesture();
    syncUndoMenu();
  }
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
  const span = selectedSpan();
  if (span && selectedNote && el.showCurve.checked) {
    const t = scale.xToTime(x);
    if (t >= span.onset_s - 0.25 && t <= span.offset_s) {
      const pitchAtY = scale.yToPitchExact(y);
      const curve = renderPitch(renderNotes(effectiveNotes()), [t])[0];
      if (Math.abs(pitchAtY - curve) * scale.rowHeight < 20) {
        selectedNote.expr = addPoint(exprOf(selectedNote), (t - span.onset_s) * 1000, (pitchAtY - span.tone) * 10).expr;
        markDirty();
        redraw();
        return;
      }
    }
  }
  const note = newNote(scale.xToTime(x), scale.yToPitch(y), currentFrame());
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
    setModal(el.changesOverlay, false);
    setModal(el.lyricsOverlay, false);
    setModal(el.detailsOverlay, false);
    e.preventDefault();
    return;
  }
  if (openMenu) {
    if (e.key === "Escape") closeMenu(true);
    return;
  }
  const tag = document.activeElement?.tagName;
  const inField = tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA";
  const mod = e.ctrlKey || e.metaKey;
  if (mod && !editsText(document.activeElement) && !anyModalOpen() && (e.key === "z" || e.key === "Z" || e.key === "y")) {
    if (e.key === "y" || e.shiftKey) redoEdit();
    else undoEdit();
    e.preventDefault();
    return;
  }
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
      const under = lastMouse && !el.pitchPen.checked ? handleAtPoint(lastMouse.x, lastMouse.y)?.handle : undefined;
      if (under?.kind === "point") {
        selectedNote.expr = removePoint(exprOf(selectedNote), under.index);
        hoveredHandle = null;
        markDirty();
        redraw();
        break;
      }
      deleteSelectedNote();
      break;
    case " ": // play / pause
      togglePlay();
      break;
    case "p":
    case "P":
      if (!annotation?.f0_contour.length) return;
      el.pitchPen.checked = !el.pitchPen.checked;
      setStatus(el.pitchPen.checked ? "Pen: drag to draw · Alt-drag to erase" : "Pen off");
      e.preventDefault();
      break;
    case "v":
    case "V":
      if (!selectedNote) return;
      toggleVibrato(selectedNote);
      e.preventDefault();
      break;
    case "l":
    case "L":
      if (!selectedNote) return;
      openLyricEditor(selectedNote);
      e.preventDefault();
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
  lastMouse = overRoll ? { x, y } : null;
  const hit = hitTest(x, y);
  hoveredHandle = el.pitchPen.checked ? null : (handleAtPoint(x, y)?.handle ?? null);
  if (el.pitchPen.checked && y >= RULER_H) el.canvas.style.cursor = e.altKey ? "cell" : "crosshair";
  else if (hoveredHandle) el.canvas.style.cursor = hoveredHandle.kind === "vibrato-start" || hoveredHandle.kind === "vibrato-period" ? "ew-resize" : "grab";
  else if (y < RULER_H) el.canvas.style.cursor = "ew-resize";
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
  lastMouse = null;
  if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
  redraw(); // hide the highlight
});

function canvasPoint(e: MouseEvent): { x: number; y: number } {
  const rect = el.canvas.getBoundingClientRect();
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function showSelectedTooltip(): void {
  // While its pitch handles are on screen, the pinned tooltip would cover them.
  if (el.showCurve.checked) {
    el.tooltip.style.display = "none";
    return;
  }
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

function currentSettings(): SessionState {
  return { bpm, division, gridAnchor, barOne, flagThreshold, snapKey };
}

/** Make `s` the review settings, and show them in the toolbar and the settings. */
function applySettings(s: SessionState): void {
  ({ bpm, division, gridAnchor, barOne, flagThreshold, snapKey } = s);
  el.bpm.value = String(bpm);
  el.division.value = String(division);
  el.flag.value = String(flagThreshold);
  el.flagVal.textContent = flagThreshold.toFixed(2);
  el.snapKey.checked = snapKey;
}

function buildSessionFile(): Annotation & { baseline: Note[]; session: SessionState } {
  return sessionFile(annotation!, workingNotes, baselineNotes, currentSettings());
}

/**
 * Save the session to a JSON file. Loading it via the annotation input restores
 * everything, unlike a MIDI export which keeps only the snapped pitches/timings.
 */
function saveSession(): void {
  if (!annotation) return;
  download(JSON.stringify(buildSessionFile()), "sona-session.json", "application/json");
  dirty = false;
  setStatus("Saved");
}

// --- Autosave (localStorage) ------------------------------------------------

const AUTOSAVE_KEY = "sona.autosave.v1";
const AUTOSAVE_MS = 20_000;

function markDirty(source?: string): void {
  dirty = true;
  edits.changed(source);
  syncUndoMenu();
}

interface DocState {
  doc: string;
  contour: F0Frame[] | null;
}

const contourShare = new SharedSnapshot<F0Frame>();

function captureDoc(): DocState {
  const doc = docText(workingNotes, currentSettings());
  return { doc, contour: annotation ? contourShare.capture(annotation.f0_contour) : null };
}

function restoreDoc(s: DocState): void {
  // The bar holds on to the note it shows while it has focus; that note is about to be replaced.
  if (exprBarHasFocus()) (document.activeElement as HTMLElement).blur();
  noteShownInExprBar = null;
  const d = parseDoc(s.doc);
  workingNotes = d.notes;
  applySettings(d.settings);
  syncBarOne();
  if (annotation && s.contour) annotation.f0_contour = contourShare.restore(s.contour);
  selectedNote = null;
  closeLyricEditor();
  el.tooltip.style.display = "none";
  dirty = true;
  invalidateEffective();
  rebuild();
}

const edits = new Undo<DocState>({ capture: captureDoc, restore: restoreDoc, equal: (a, b) => a.doc === b.doc && a.contour === b.contour });

/** A field with its own text undo (the browser's): everything else gets the document's undo. */
function editsText(el: Element | null): boolean {
  if (el instanceof HTMLTextAreaElement) return true;
  if (!(el instanceof HTMLInputElement)) return false;
  return ["text", "number", "search", "url", "email", "tel", "password"].includes(el.type);
}

function syncUndoMenu(): void {
  el.editUndo.disabled = !edits.canUndo;
  el.editRedo.disabled = !edits.canRedo;
}

function undoEdit(): void {
  if (edits.undo()) setStatus("Undone");
  syncUndoMenu();
}

function redoEdit(): void {
  if (edits.redo()) setStatus("Redone");
  syncUndoMenu();
}

/**
 * Persist the session to localStorage (not a download — that would spam the
 * Downloads folder). If the full session exceeds the storage quota it retries
 * with the energy track dropped and the pitch line packed, then without either.
 */
function autosave(): void {
  if (!annotation) return;
  const file = buildSessionFile();
  const savedAt = new Date().toISOString();
  const write = (payload: object) => localStorage.setItem(AUTOSAVE_KEY, JSON.stringify(payload));
  try {
    write({ savedAt, file });
  } catch {
    // The pitch pen edits the contour, so a lite save keeps it (packed) and drops it only as a last resort.
    try {
      write({ savedAt, file: { ...file, f0_contour: packContour(file.f0_contour), energy_envelope: [] }, lite: true });
    } catch {
      try {
        write({ savedAt, file: { ...file, f0_contour: [], energy_envelope: [] }, lite: true });
        setStatus("Storage full: pen edits not autosaved · File → Save session", true);
        dirty = false;
        return;
      } catch {
        return; // storage unavailable / still too big — give up quietly
      }
    }
  }
  dirty = false;
  setStatus(`Autosaved ${new Date(savedAt).toLocaleTimeString()}`);
}

/** Restore the last autosaved session on startup, if one exists. */
/** Show the start screen's Resume card for an autosave, without loading it. */
function offerResume(raw: string): void {
  try {
    const record: unknown = JSON.parse(raw);
    if (typeof record !== "object" || record === null) return;
    const r = record as { savedAt?: unknown; file?: { notes?: unknown } };
    const notes = Array.isArray(r.file?.notes) ? r.file.notes.length : 0;
    const when = typeof r.savedAt === "string" ? new Date(r.savedAt).toLocaleString() : "earlier";
    el.startResumeDetail.textContent = `${notes} notes, saved ${when}. Re-open the stem to play it.`;
    el.startResume.hidden = false;
  } catch {
    el.startResume.hidden = true;
  }
}

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
    setStatus(`Resumed (${when}) · File → Open stem audio to hear it`);
  } catch (err) {
    console.error("sona: could not restore autosave", err);
    edits.reset();
    syncUndoMenu();
  }
}

// Tick every 20s; only writes when there are unsaved edits. Also flush on close.
window.setInterval(() => {
  if (dirty) autosave();
}, AUTOSAVE_MS);
window.addEventListener("beforeunload", () => {
  if (dirty) autosave();
});

el.save.addEventListener("click", saveSession);

/**
 * Export the authoritative MIDI (ADR-0004): the same sequence as the UST and
 * USTX, so its notes sit on the same ticks. Escaped notes export at their raw
 * position, as shown.
 */
function exportMidi(): void {
  if (!annotation) return;
  const seq = sequenceForExport();
  download(encodeMidiFile(seq, { bpm }) as BlobPart, "sona.mid", "audio/midi");
  setStatus(`Exported sona.mid${shiftNote(seq)}`);
}

function notesInTimeOrder(): WorkingNote[] {
  return inTimeOrder(effectiveNotes());
}

function sequenceForExport(): Sequence {
  return exportSequence(exportNotes(effectiveNotes()), currentFrame());
}

/** Where tick 0 (bar 1 in OpenUtau) is in the stem, for lining the stem up with the export. */
function shiftNote(seq: Sequence): string {
  const ms = Math.round(seq.shiftS * 1000);
  if (barOne === null) return seq.shiftS > 0.0005 ? ` · shifted ${ms} ms onto the grid` : "";
  if (ms > 0) return ` · bar 1 starts ${ms} ms into the stem`;
  return ms < 0 ? ` · bar 1 starts ${-ms} ms before the stem` : "";
}

function exportUst(): void {
  if (!annotation) return;
  const seq = sequenceForExport();
  const { bytes, encoding } = encodeUst(seq, { bpm, projectName: "sona" });
  download(bytes as BlobPart, "sona.ust", "text/plain");
  setStatus(`Exported sona.ust${encoding === "shift_jis" ? "" : " (UTF-8)"}${shiftNote(seq)}`);
}

function exportUstx(): void {
  if (!annotation) return;
  const seq = sequenceForExport();
  download(ustxText(seq, { bpm, name: "sona" }), "sona.ustx", "text/yaml");
  setStatus(`Exported sona.ustx${shiftNote(seq)}`);
}

el.exportUst.addEventListener("click", exportUst);
el.exportUstx.addEventListener("click", exportUstx);
el.exportUstxSung.addEventListener("click", exportUstxSung);

function exportUstxSung(): void {
  if (!annotation) return;
  const seq = sequenceForExport();
  const curve = sungPitchCurve(annotation.f0_contour, sequenceInSeconds(seq, bpm), {
    shiftS: seq.shiftS,
    ticksPerSecond: (bpm / 60) * TICKS_PER_QUARTER,
  });
  download(ustxText(seq, { bpm, name: "sona", curves: [{ ...curve, abbr: "pitd" }] }), "sona.ustx", "text/yaml");
  setStatus(`Exported sona.ustx with the singer's pitch${shiftNote(seq)}`);
}

let lyricTarget: WorkingNote | null = null;

/** Notes are culled by time only, so an off-pitch note still has a hitbox: scroll pitch too. */
function scrollNoteIntoViewBothAxes(eff: EffectiveNote): void {
  if (eff.onset_s < viewT0 || eff.offset_s > viewT1) {
    scrollX = clampScroll(scrollX + ((eff.onset_s - viewT0 - (viewT1 - viewT0) * 0.3) * (plotWidthPx() * zoomX)) / Math.max(totalDuration(), 1e-9));
  }
  if (eff.pitch > pitchTop || eff.pitch < pitchTop - VISIBLE_SEMITONES + 1) {
    pitchTop = clampPitchTop(Math.round(eff.pitch) + Math.floor(VISIBLE_SEMITONES / 2));
  }
}

function openLyricEditor(note: WorkingNote): void {
  selectedNote = note;
  const eff = effectiveNotes().find((e) => e.raw === note);
  if (eff) scrollNoteIntoViewBothAxes(eff);
  redraw();
  const hit = noteHitboxes.find((h) => h.note === note);
  if (!hit) {
    closeLyricEditor();
    return;
  }
  lyricTarget = note;
  el.tooltip.style.display = "none";
  el.lyricEditor.value = note.lyric ?? "";
  el.lyricEditor.style.left = `${Math.max(2, hit.x)}px`;
  el.lyricEditor.style.top = `${Math.max(2, hit.y - 28)}px`;
  el.lyricEditor.hidden = false;
  el.lyricEditor.focus();
  el.lyricEditor.select();
}

function commitLyric(): void {
  if (!lyricTarget) return;
  const text = el.lyricEditor.value.trim();
  if (text) lyricTarget.lyric = text;
  else delete lyricTarget.lyric;
  markDirty();
}

function closeLyricEditor(): void {
  lyricTarget = null;
  el.lyricEditor.hidden = true;
  el.lyricEditor.blur();
  redraw();
}

el.lyricEditor.addEventListener("keydown", (e: KeyboardEvent) => {
  if (e.key === "Escape") {
    closeLyricEditor();
  } else if (e.key === "Enter" || e.key === "Tab") {
    e.preventDefault();
    commitLyric();
    const order = notesInTimeOrder();
    const i = lyricTarget ? order.indexOf(lyricTarget) : -1;
    const next = order[i + (e.shiftKey ? -1 : 1)];
    if (next) openLyricEditor(next);
    else closeLyricEditor();
  }
});
el.lyricEditor.addEventListener("blur", () => {
  if (lyricTarget) {
    commitLyric();
    closeLyricEditor();
  }
});

function updateLyricsPreview(): void {
  const tokens = splitLyrics(el.lyricsText.value);
  const total = workingNotes.length;
  const sung = workingNotes.filter((n) => n.lyric).length;
  el.lyricsPreview.textContent =
    `${tokens.length} token${tokens.length === 1 ? "" : "s"} · ${total} notes (${sung} with lyrics)` +
    (tokens.length ? ` · starts: ${tokens.slice(0, 12).join(" ")}${tokens.length > 12 ? " …" : ""}` : "");
  el.lyricsFromSelected.disabled = !selectedNote || !tokens.length;
  el.lyricsFromStart.disabled = !tokens.length;
}

function fillLyrics(from: WorkingNote | null): void {
  const order = notesInTimeOrder();
  const start = from ? Math.max(0, order.indexOf(from)) : 0;
  const tokens = splitLyrics(el.lyricsText.value);
  const { assigned, leftover } = assignLyrics({ tokens, count: order.length, from: start });
  for (const [i, lyric] of assigned) order[i].lyric = lyric;
  markDirty();
  setModal(el.lyricsOverlay, false);
  redraw();
  setStatus(`Lyrics on ${assigned.size} notes` + (leftover ? ` · ${leftover} left over` : ""));
}

el.lyrics.addEventListener("click", () => {
  setModal(el.lyricsOverlay, el.lyricsOverlay.hidden);
  updateLyricsPreview();
  if (!el.lyricsOverlay.hidden) el.lyricsText.focus();
});
el.lyricsClose.addEventListener("click", () => setModal(el.lyricsOverlay, false));
el.lyricsOverlay.addEventListener("click", (e) => {
  if (e.target === el.lyricsOverlay) setModal(el.lyricsOverlay, false);
});
el.lyricsText.addEventListener("input", updateLyricsPreview);
el.lyricsFromSelected.addEventListener("click", () => fillLyrics(selectedNote));
el.lyricsFromStart.addEventListener("click", () => fillLyrics(null));
el.lyricsClear.addEventListener("click", () => {
  for (const n of workingNotes) delete n.lyric;
  markDirty();
  setModal(el.lyricsOverlay, false);
  redraw();
  setStatus("Lyrics cleared");
});

let hoveredHandle: Handle | null = null;
let lastMouse: { x: number; y: number } | null = null;
const HANDLE_RADIUS = 5;
const HANDLE_GRAB_PX = 9;

function selectedSpan(): NoteSpan | null {
  return selectedNote ? noteSpan(effectiveNotes(), selectedNote) : null;
}

function handleScreen(t: number, pitch: number): { x: number; y: number } {
  return { x: scale!.timeToX(t), y: scale!.pitchToY(pitch) + scale!.rowHeight / 2 };
}

function handleAtPoint(px: number, py: number): { handle: Handle; span: NoteSpan } | null {
  const span = selectedSpan();
  if (!scale || !span || !el.showCurve.checked || !selectedNote) return null;
  let best: { handle: Handle; d: number } | null = null;
  for (const h of handlesFor(exprOf(selectedNote), span)) {
    const p = handleScreen(h.t, h.pitch);
    const d = Math.hypot(p.x - px, p.y - py);
    if (d <= HANDLE_GRAB_PX && (!best || d < best.d)) best = { handle: h.handle, d };
  }
  return best ? { handle: best.handle, span } : null;
}

function sameHandle(a: Handle | null, b: Handle): boolean {
  return !!a && a.kind === b.kind && (a.kind !== "point" || (b.kind === "point" && a.index === b.index));
}

const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const HANDLE_FADE_MS = 160;
let handlesShownFor: WorkingNote | null = null;
let handlesShownAt = 0;
let fadeFrame = 0;

function noteHandlesShownFor(note: WorkingNote | null): void {
  if (handlesShownFor === note) return;
  handlesShownFor = note;
  handlesShownAt = performance.now();
}

function handleFadeAlpha(): number {
  if (reducedMotion.matches) return 1;
  const k = Math.min(1, (performance.now() - handlesShownAt) / HANDLE_FADE_MS);
  if (k < 1 && !fadeFrame) {
    fadeFrame = requestAnimationFrame(() => {
      fadeFrame = 0;
      redraw();
    });
  }
  return 1 - (1 - k) ** 3;
}

function drawPitchHandles(): void {
  const span = selectedSpan();
  noteHandlesShownFor(span ? selectedNote : null);
  if (!scale || !span || !selectedNote) return;
  const expr = exprOf(selectedNote);
  const hs = handlesFor(expr, span);
  ctx2d.save();
  ctx2d.globalAlpha = handleFadeAlpha();
  const v = expr.vibrato;
  if (v && v.length > 0) {
    const at = (k: string) => hs.find((h) => h.handle.kind === k)!;
    const start = handleScreen(at("vibrato-start").t, span.tone);
    const fin = handleScreen(at("vibrato-fade-in").t, at("vibrato-fade-in").pitch);
    const fout = handleScreen(at("vibrato-fade-out").t, at("vibrato-fade-out").pitch);
    const end = handleScreen(span.offset_s, span.tone);
    ctx2d.setLineDash([4, 4]);
    ctx2d.strokeStyle = "rgba(255,159,64,0.7)";
    ctx2d.lineWidth = 1.5;
    ctx2d.beginPath();
    ctx2d.moveTo(start.x, start.y);
    ctx2d.lineTo(fin.x, fin.y);
    ctx2d.lineTo(fout.x, fout.y);
    ctx2d.lineTo(end.x, end.y);
    ctx2d.stroke();
    ctx2d.setLineDash([]);
  }
  for (const h of hs) {
    const p = handleScreen(h.t, h.pitch);
    const hot = sameHandle(hoveredHandle, h.handle) || (drag?.mode === "handle" && sameHandle(drag.handle, h.handle));
    const r = hot ? HANDLE_RADIUS + 2 : HANDLE_RADIUS;
    ctx2d.beginPath();
    if (h.handle.kind === "point") {
      ctx2d.arc(p.x, p.y, r, 0, Math.PI * 2);
    } else {
      ctx2d.moveTo(p.x, p.y - r - 1);
      ctx2d.lineTo(p.x + r + 1, p.y);
      ctx2d.lineTo(p.x, p.y + r + 1);
      ctx2d.lineTo(p.x - r - 1, p.y);
      ctx2d.closePath();
    }
    const snapped = h.handle.kind === "point" && h.handle.index === 0 && expr.snapFirst;
    ctx2d.fillStyle = snapped ? "#101010" : "#ffd29e";
    ctx2d.strokeStyle = hot ? "#ffffff" : "#ff9f40";
    ctx2d.lineWidth = 2;
    ctx2d.fill();
    ctx2d.stroke();
  }
  ctx2d.restore();
}

function drawExpressionCurve(): void {
  if (!scale || (!selectedNote && !workingNotes.some((n) => n.expr))) return;
  const effs = effectiveNotes().filter((e) => e.offset_s > viewT0 - 1 && e.onset_s < viewT1 + 1);
  if (!effs.length) return;
  const step = 0.005;
  const times: number[] = [];
  for (const e of effs) {
    if (e.offset_s < viewT0 || e.onset_s > viewT1) continue;
    for (let t = Math.max(e.onset_s, viewT0); t < Math.min(e.offset_s, viewT1); t += step) times.push(t);
  }
  const pitch = renderPitch(renderNotes(effs), times);
  ctx2d.save();
  ctx2d.strokeStyle = "#ff9f40";
  ctx2d.lineWidth = 2;
  ctx2d.beginPath();
  let prevT = -Infinity;
  times.forEach((t, i) => {
    const x = scale!.timeToX(t);
    const y = scale!.pitchToY(pitch[i]) + scale!.rowHeight / 2;
    if (t - prevT > step * 1.5) ctx2d.moveTo(x, y);
    else ctx2d.lineTo(x, y);
    prevT = t;
  });
  ctx2d.stroke();
  ctx2d.restore();
}

function fitAll(only?: WorkingNote): void {
  if (!annotation?.f0_contour.length) return;
  const effs = effectiveNotes();
  const fits = fitExpressions(renderNotes(effs), annotation.f0_contour);
  let glides = 0;
  let vibratos = 0;
  effs.forEach((e, i) => {
    if (only && e.raw !== only) return;
    e.raw.expr = fits[i];
    if (fits[i].vibrato) vibratos++;
    if (fits[i].points.length > 2 || fits[i].points[0].x !== NEUTRAL_EXPRESSION.points[0].x) glides++;
  });
  markDirty();
  redraw();
  setStatus(only ? "Pitch fitted" : `Pitch fitted · ${glides} glides · ${vibratos} vibratos`);
}

function exprOf(note: WorkingNote): Expression {
  return note.expr ?? { ...NEUTRAL_EXPRESSION, points: NEUTRAL_EXPRESSION.points.map((p) => ({ ...p })) };
}

function toggleVibrato(note: WorkingNote): void {
  const ex = exprOf(note);
  note.expr = { ...ex, vibrato: ex.vibrato ? null : { ...DEFAULT_VIBRATO } };
  markDirty();
  redraw();
}

let noteShownInExprBar: WorkingNote | null = null;

const exprBarHasFocus = (): boolean => el.exprBar.contains(document.activeElement);

const OPENUTAU_VIBRATO_LIMITS = { depth: [5, 200], period: [5, 500], percent: [0, 100] } as const;
const GLIDE_X_LIMITS_MS = { start: [-2000, 2000], end: [-2000, 5000] } as const;

function syncSelectionActions(): void {
  for (const b of [el.editLyric, el.editVibrato, el.editDelete]) b.disabled = !selectedNote;
}

function syncExprBar(): void {
  syncSelectionActions();
  if (exprBarHasFocus()) return;
  const n = selectedNote;
  noteShownInExprBar = n;
  el.exprBar.hidden = !n || !annotation;
  if (!n) return;
  const ex = exprOf(n);
  el.glideStart.value = String(ex.points[0]?.x ?? 0);
  el.glideEnd.value = String(ex.points[1]?.x ?? 0);
  const v = ex.vibrato;
  el.vibOn.checked = !!v;
  el.exprVib.setAttribute("aria-pressed", String(!!v));
  const shown = v ?? DEFAULT_VIBRATO;
  el.vibDepth.value = String(shown.depth);
  el.vibPeriod.value = String(shown.period);
  el.vibLength.value = String(shown.length);
  el.vibIn.value = String(shown.in);
  el.vibOut.value = String(shown.out);
  for (const input of [el.vibDepth, el.vibPeriod, el.vibLength, el.vibIn, el.vibOut]) input.disabled = !v;
  el.exprFit.disabled = !annotation?.f0_contour.length;
}

const clampNum = (input: HTMLInputElement, lo: number, hi: number, fallback: number): number => {
  const v = Number(input.value);
  return Number.isFinite(v) ? Math.max(lo, Math.min(hi, v)) : fallback;
};

function applyExprBar(): void {
  const n = noteShownInExprBar;
  if (!n) return;
  const ex = exprOf(n);
  const points = ex.points.map((p) => ({ ...p }));
  if (points.length >= 2) {
    points[0].x = clampNum(el.glideStart, ...GLIDE_X_LIMITS_MS.start, points[0].x);
    points[1].x = Math.max(points[0].x + 1, clampNum(el.glideEnd, ...GLIDE_X_LIMITS_MS.end, points[1].x));
  }
  const base = ex.vibrato ?? DEFAULT_VIBRATO;
  const fadeIn = clampNum(el.vibIn, ...OPENUTAU_VIBRATO_LIMITS.percent, base.in);
  const vibrato = el.vibOn.checked
    ? {
        ...base,
        depth: clampNum(el.vibDepth, ...OPENUTAU_VIBRATO_LIMITS.depth, base.depth),
        period: clampNum(el.vibPeriod, ...OPENUTAU_VIBRATO_LIMITS.period, base.period),
        length: clampNum(el.vibLength, ...OPENUTAU_VIBRATO_LIMITS.percent, base.length),
        in: fadeIn,
        out: Math.min(100 - fadeIn, clampNum(el.vibOut, ...OPENUTAU_VIBRATO_LIMITS.percent, base.out)),
      }
    : null;
  n.expr = { ...ex, points, vibrato };
  markDirty();
  redraw();
}

for (const input of [el.glideStart, el.glideEnd, el.vibOn, el.vibDepth, el.vibPeriod, el.vibLength, el.vibIn, el.vibOut]) {
  input.addEventListener("change", applyExprBar);
}
el.fitPitch.addEventListener("click", () => fitAll());
el.showCurve.addEventListener("change", () => redraw());
el.exprVib.addEventListener("click", () => selectedNote && toggleVibrato(selectedNote));
el.exprExactToggle.addEventListener("click", () => {
  el.exprExact.hidden = !el.exprExact.hidden;
  el.exprExactToggle.setAttribute("aria-expanded", String(!el.exprExact.hidden));
});
el.exprFit.addEventListener("click", () => selectedNote && fitAll(selectedNote));
el.exprReset.addEventListener("click", () => {
  if (!selectedNote) return;
  delete selectedNote.expr;
  markDirty();
  redraw();
});

let openMenu: HTMLElement | null = null;

const menuItems = (menu: HTMLElement): HTMLElement[] =>
  Array.from(menu.querySelectorAll<HTMLElement>('[role="menuitem"], [role="menuitemcheckbox"]')).filter(
    (b) => !(b instanceof HTMLButtonElement ? b.disabled : b.querySelector("input")?.disabled),
  );

function closeMenu(focusButton = false): void {
  if (!openMenu) return;
  const btn = openMenu.querySelector<HTMLButtonElement>(".menu-btn");
  openMenu.querySelector<HTMLElement>(".menu-list")!.hidden = true;
  btn?.setAttribute("aria-expanded", "false");
  if (focusButton) btn?.focus();
  openMenu = null;
}

function showMenu(menu: HTMLElement, focusFirst: boolean): void {
  closeMenu();
  menu.querySelector<HTMLElement>(".menu-list")!.hidden = false;
  menu.querySelector(".menu-btn")?.setAttribute("aria-expanded", "true");
  openMenu = menu;
  if (focusFirst) menuItems(menu)[0]?.focus();
}

for (const menu of Array.from(document.querySelectorAll<HTMLElement>(".menu"))) {
  const btn = menu.querySelector<HTMLButtonElement>(".menu-btn")!;
  btn.addEventListener("click", () => (openMenu === menu ? closeMenu() : showMenu(menu, false)));
  btn.addEventListener("pointerenter", () => openMenu && openMenu !== menu && showMenu(menu, false));
  btn.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      showMenu(menu, true);
    }
  });
  menu.querySelector(".menu-list")!.addEventListener("click", (e) => {
    if ((e.target as Element).closest('[role="menuitem"]')) closeMenu();
  });
  menu.addEventListener("keydown", (e) => {
    if (openMenu !== menu) return;
    const items = menuItems(menu);
    const i = items.indexOf(document.activeElement as HTMLElement);
    const active = document.activeElement as HTMLElement | null;
    if ((e.key === "Enter" || e.key === " ") && active?.getAttribute("role") === "menuitemcheckbox") {
      e.preventDefault();
      e.stopPropagation();
      active.querySelector("input")?.click();
      return;
    }
    const go = (k: number) => items[(k + items.length) % items.length]?.focus();
    if (e.key === "ArrowDown") go(i + 1);
    else if (e.key === "ArrowUp") go(i < 0 ? -1 : i - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(-1);
    else if (e.key === "Escape") closeMenu(true);
    else if (e.key === "Tab") closeMenu();
    else return;
    e.preventDefault();
    e.stopPropagation();
  });
}
document.addEventListener("pointerdown", (e) => {
  if (openMenu && !openMenu.contains(e.target as Node)) closeMenu();
});

function deleteSelectedNote(): void {
  if (!selectedNote) return;
  workingNotes = withoutNote(workingNotes, selectedNote);
  selectedNote = null;
  el.tooltip.style.display = "none";
  invalidateEffective();
  markDirty();
  redraw();
}

el.openAnnotation.addEventListener("click", () => el.json.click());

function openDetails(): void {
  const p = annotation?.provenance;
  el.detailsBody.replaceChildren();
  el.detailsDownload.hidden = !p;
  if (!p) {
    const none = document.createElement("p");
    none.textContent = "This annotation doesn't record what produced it.";
    el.detailsBody.append(none);
  } else {
    for (const section of detailSections(p)) {
      const h = document.createElement("h3");
      h.textContent = section.title;
      const dl = document.createElement("dl");
      dl.className = "details-list";
      for (const [k, v] of section.rows) {
        const dt = document.createElement("dt");
        dt.textContent = k;
        const dd = document.createElement("dd");
        dd.textContent = v;
        dl.append(dt, dd);
      }
      el.detailsBody.append(h, dl);
    }
  }
  setModal(el.detailsOverlay, true);
}

el.details.addEventListener("click", openDetails);
el.detailsClose.addEventListener("click", () => setModal(el.detailsOverlay, false));
el.detailsOverlay.addEventListener("click", (e) => {
  if (e.target === el.detailsOverlay) setModal(el.detailsOverlay, false);
});
el.detailsDownload.addEventListener("click", () => {
  if (annotation?.provenance) download(JSON.stringify(annotation.provenance, null, 2), "sona-provenance.json", "application/json");
});
el.openStem.addEventListener("click", () => el.audio.click());
el.editLyric.addEventListener("click", () => selectedNote && openLyricEditor(selectedNote));
el.editVibrato.addEventListener("click", () => selectedNote && toggleVibrato(selectedNote));
el.editDelete.addEventListener("click", deleteSelectedNote);
el.editUndo.addEventListener("click", undoEdit);
el.editRedo.addEventListener("click", redoEdit);
el.startOpen.addEventListener("click", () => el.json.click());
el.startResume.addEventListener("click", () => {
  restoreAutosave();
  el.startResume.hidden = true;
});

let startShown = false;
let startState: StartState = { kind: "idle", error: null };
let serveCaps: Capabilities | null = null;
let songBlob: File | null = null;
let songCounter = 0;
let pollTimer: number | null = null;

function dispatchStart(e: StartEvent): void {
  startState = startStep(startState, e);
  renderStart();
}

function renderStart(): void {
  const s = startState;
  const unavailable = s.kind === "unavailable";
  el.drop.hidden = s.kind === "ready" || s.kind === "running";
  el.drop.classList.toggle("disabled", unavailable);
  el.songFile.disabled = unavailable;
  el.dropTitle.textContent = unavailable ? "Start sona serve to transcribe" : "Drop a song";
  el.dropSub.textContent = unavailable ? "" : s.kind === "idle" && s.error ? s.error : "or click to choose";
  el.songReady.hidden = s.kind !== "ready";
  if (s.kind === "ready") {
    el.songName.textContent = s.name;
    if (document.activeElement !== el.songBpm) el.songBpm.value = s.bpm === null ? "" : String(s.bpm);
    const d = s.detected;
    el.songHintText.textContent = d === "pending" ? "finding the tempo…" : d === null ? "no steady beat found" : typeof d === "number" ? "detected ≈" : `no tempo: ${d.error}`;
    el.songHintFull.hidden = el.songHintHalf.hidden = typeof d !== "number";
    if (typeof d === "number") {
      el.songHintFull.textContent = String(hintBpm(d, 1));
      el.songHintHalf.textContent = `½× ${hintBpm(d, 0.5)}`;
      el.songHintFull.title = `Use ${hintBpm(d, 1)} BPM`;
      el.songHintHalf.title = `Use ${hintBpm(d, 0.5)} BPM`;
    }
    el.songError.hidden = !s.error;
    el.songError.textContent = s.error ?? "";
  }
  el.setupNotes.hidden = !serveCaps?.setup.length || (s.kind !== "idle" && s.kind !== "ready");
  el.songRunning.hidden = s.kind !== "running";
  if (s.kind === "running") {
    el.songRunningName.textContent = s.name;
    el.songPhase.textContent = s.job === null ? "Uploading…" : `${PHASE_LABEL[s.phase]} · ${Math.round(s.elapsedS)} s`;
    el.songBpm.blur();
    el.songBar.style.width = `${Math.max(4, phaseProgress(s.phase, el.songSeparation.value as Separation) * 100)}%`;
    el.songLog.textContent = s.log.join("\n") || "Nothing logged yet.";
  }
}

function showStart(show: boolean): void {
  startShown = show;
  if (show && startState.kind === "loaded") dispatchStart({ type: "start-over" });
  rebuild();
}

function applyCapabilities(caps: Capabilities): void {
  serveCaps = caps;
  const d = defaultJob(caps);
  for (const opt of Array.from(el.songSeparation.options)) opt.disabled = !caps.separation[opt.value as Separation];
  for (const opt of Array.from(el.songEngine.options)) opt.disabled = !caps.engines[opt.value as Engine];
  el.songSeparation.value = d.separation;
  el.songEngine.value = d.engine;
  el.setupNotes.replaceChildren(
    ...caps.setup.map((n) => {
      const li = document.createElement("li");
      li.textContent = n.message;
      li.classList.toggle("missing", n.status === "missing");
      return li;
    }),
  );
  syncBarOneOption();
  dispatchStart({ type: "serve-found" });
}

/** Bar 1 from the music needs the full song beside its vocals, so not an upload that is already a stem. */
function syncBarOneOption(): void {
  el.songBarOne.disabled = el.songSeparation.value === "none";
}
el.songSeparation.addEventListener("change", syncBarOneOption);

async function chooseSong(file: File): Promise<void> {
  if (startState.kind === "running" || startState.kind === "unavailable") return;
  const song = ++songCounter;
  songBlob = file;
  dispatchStart({ type: "choose", song, name: file.name });
  el.songBpm.focus();
  const answer = serveCaps?.tempo ? await estimateTempo(file, file.name) : { error: "this sona serve can't estimate tempo" };
  dispatchStart("error" in answer ? { type: "tempo-failed", song, message: answer.error } : { type: "tempo", song, bpm: answer.bpm });
}

let attempts = 0;

const awaitingJob = (attempt: number): boolean => startState.kind === "running" && startState.attempt === attempt && startState.job === null;
const watching = (job: string): boolean => startState.kind === "running" && startState.job === job;

async function transcribeSong(): Promise<void> {
  const attempt = ++attempts;
  dispatchStart({ type: "transcribe", attempt });
  const s = startState;
  if (!awaitingJob(attempt) || s.kind !== "running" || !songBlob) return;
  const grid = el.songGrid.value === "24" ? 24 : 16;
  try {
    const separation = el.songSeparation.value as Separation;
    const barOneWanted = el.songBarOne.checked && separation !== "none";
    const job = await submitJob({ bpm: s.bpm, grid, separation, engine: el.songEngine.value as Engine, name: s.name, ...(barOneWanted ? { barOne: true } : {}) }, songBlob);
    if (!awaitingJob(attempt)) {
      await cancelJob(job.id).catch(() => undefined);
      return;
    }
    dispatchStart({ type: "submitted", attempt, job: job.id });
    showJobProgress(job);
    pollJob(job.id, s.bpm, grid);
  } catch (err) {
    dispatchStart({ type: "failed-to-submit", attempt, message: (err as Error).message });
  }
}

function showJobProgress(j: JobStatus): void {
  dispatchStart({ type: "progress", job: j.id, phase: j.phase, elapsedS: j.elapsedS, log: j.log, error: j.error });
}

function pollJob(id: string, jobBpm: number, grid: number): void {
  pollTimer = window.setTimeout(async () => {
    if (startState.kind !== "running" || startState.job !== id) return;
    try {
      const j = await fetchJobOrCancel(id);
      if (!watching(id)) return;
      showJobProgress(j);
      if (!isFinal(j.phase)) return pollJob(id, jobBpm, grid);
      if (j.phase === "done") await loadJobResults(j, jobBpm, grid);
    } catch (err) {
      dispatchStart({ type: "progress", job: id, phase: "failed", elapsedS: 0, log: [], error: (err as Error).message });
    }
  }, 1000);
}

async function loadJobResults(j: JobStatus, jobBpm: number, grid: number): Promise<void> {
  if (dirty && !window.confirm(`Replace the current notes with the ones from “${j.name}”? Your unsaved edits stay in the autosave.`)) {
    dispatchStart({ type: "progress", job: j.id, phase: "cancelled", elapsedS: j.elapsedS, log: j.log, error: null });
    return;
  }
  const { annotation: ann, stem } = await fetchResults(j.id);
  if (!watching(j.id)) return;
  if (!loadAnnotationJson(ann, true)) {
    dispatchStart({ type: "progress", job: j.id, phase: "failed", elapsedS: j.elapsedS, log: j.log, error: "the result couldn't be read" });
    return;
  }
  bpm = jobBpm;
  el.bpm.value = String(jobBpm);
  if (Array.from(el.division.options).some((o) => o.value === String(grid))) {
    division = grid;
    el.division.value = String(grid);
  }
  invalidateEffective();
  await loadStemBytes(stem);
  fitAll();
  edits.reset();
  syncUndoMenu();
  dispatchStart({ type: "results-loaded", job: j.id });
  startShown = false;
  rebuild();
  syncBarOne();
  setStatus(`${workingNotes.length} notes${annotation?.bar_one ? ` · bar 1: ${el.barOneReadout.textContent}` : ""}`);
}

el.songFile.addEventListener("change", () => {
  const f = el.songFile.files?.[0];
  el.songFile.value = "";
  if (f) void chooseSong(f);
});
for (const t of ["dragenter", "dragover"] as const) {
  el.drop.addEventListener(t, (e) => {
    e.preventDefault();
    if (startState.kind !== "unavailable") el.drop.classList.add("over");
  });
}
for (const t of ["dragleave", "drop"] as const) el.drop.addEventListener(t, () => el.drop.classList.remove("over"));
el.drop.addEventListener("drop", (e) => {
  e.preventDefault();
  const f = e.dataTransfer?.files[0];
  if (f) void chooseSong(f);
});
el.drop.addEventListener("keydown", (e) => {
  if ((e.key === "Enter" || e.key === " ") && !el.songFile.disabled) {
    e.preventDefault();
    el.songFile.click();
  }
});
el.songChange.addEventListener("click", () => dispatchStart({ type: "start-over" }));
el.songBpm.addEventListener("input", () => dispatchStart({ type: "set-bpm", bpm: parseBpm(el.songBpm.value) }));
el.songHintFull.addEventListener("click", () => dispatchStart({ type: "use-hint", multiple: 1 }));
el.songHintHalf.addEventListener("click", () => dispatchStart({ type: "use-hint", multiple: 0.5 }));
el.songGo.addEventListener("click", () => void transcribeSong());
el.songBpm.addEventListener("keydown", (e) => {
  if (e.key === "Enter") void transcribeSong();
});
el.songCancel.addEventListener("click", async () => {
  const s = startState;
  if (s.kind !== "running") return;
  if (pollTimer !== null) window.clearTimeout(pollTimer);
  dispatchStart({ type: "cancel" });
  if (s.job) await cancelJob(s.job).catch(() => undefined);
});
el.generate.addEventListener("click", () => showStart(true));
el.startBack.addEventListener("click", () => showStart(false));

void fetchCapabilities()
  .then(applyCapabilities)
  .catch(() => dispatchStart({ type: "serve-missing", reason: "no sona serve" }));
renderStart();

// --- Playback ---------------------------------------------------------------

function ensureAudioContext(): AudioContext {
  if (!audioCtx) audioCtx = new AudioCtx();
  return audioCtx;
}

el.play.addEventListener("click", () => togglePlay());

function syncPlayButton(): void {
  el.play.disabled = !(annotation && audioBuffer);
  el.play.textContent = playback ? "⏸" : "▶";
  el.play.setAttribute("aria-label", playback ? "Pause" : "Play");
  el.play.title = playback ? "Pause (Space)" : "Play (Space)";
}

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
  for (const voice of buildSchedule(playbackNotes(effectiveNotes()))) {
    const osc = scheduleVoice(ac, synthGain, voice, startedAt, from);
    if (osc) voices.push(osc);
  }

  const duration = totalDuration();
  playback = { startedAt, duration, from, stemSource, stemGain, synthGain, voices, raf: 0 };
  stemSource.onended = () => {
    // Natural end: reset the playhead to the start so the next Play replays.
    if (playback && playback.stemSource === stemSource) stopPlayback(true);
  };

  syncPlayButton();
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
  syncPlayButton();
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
  el.status.classList.remove("changed");
  void el.status.offsetWidth; // restart the fade for each new message
  el.status.classList.add("changed");
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
  if (raw) offerResume(raw);
  else if (tries < 8) setTimeout(() => attemptRestore(tries + 1), 120);
}
// setTimeout, not requestAnimationFrame: rAF is paused while the tab is
// backgrounded, which would leave the initial paint and autosave restore
// hanging until the tab is focused.
setTimeout(() => {
  rebuild();
  attemptRestore();
}, 0);
