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
// The pure geometry/colour/grid/schedule/MIDI maths live in ./lib/*.js so they
// can be unit-tested under `node --test`; this file is the DOM + Web Audio +
// canvas plumbing that can only run in a browser.

import {
  midiToNoteName,
  confidenceColor,
  weakestSubScore,
  pitchRange,
  makeScale,
} from "./lib/view.js";
import { buildSchedule } from "./lib/synth.js";
import { clamp01 } from "./lib/math.js";
import { gridSpacingSeconds, snapTime, gridLines, quantizeNote, noteFlags } from "./lib/grid.js";
import { encodeMidiFile } from "./lib/midi.js";

const RULER_H = 18; // top strip you drag to shift the grid phase.
const PAD = { left: 44, top: RULER_H, bottom: 22 };
const NOTE_RADIUS = 3;
const RESIZE_GRAB_PX = 7; // right-edge zone that starts a resize instead of a move.
const FLAG_COLOR = "#ff5c5c";
const ESCAPE_COLOR = "#5cd0ff";

const el = {
  json: document.getElementById("json-input"),
  audio: document.getElementById("audio-input"),
  play: document.getElementById("play"),
  stop: document.getElementById("stop"),
  muteStem: document.getElementById("mute-stem"),
  muteSynth: document.getElementById("mute-synth"),
  bpm: document.getElementById("bpm"),
  division: document.getElementById("division"),
  flag: document.getElementById("flag"),
  flagVal: document.getElementById("flag-val"),
  export: document.getElementById("export"),
  status: document.getElementById("status"),
  canvas: document.getElementById("roll"),
  tooltip: document.getElementById("tooltip"),
};
const ctx2d = el.canvas.getContext("2d");

// --- State -----------------------------------------------------------------

/** @type {import("./annotation.d.ts").Annotation | null} */
let annotation = null;
/** Editable working copy of the notes (raw seconds); the source of truth for edits. */
let workingNotes = [];
/** Decoded stem, for playback and for the total roll duration. */
let audioBuffer = null;
/** Rendering geometry, rebuilt on load and resize. Null until data arrives. */
let scale = null;
/** Screen rects of drawn notes, for hover/hit-testing. Each carries its working note. */
let noteHitboxes = [];
/** The working note the user selected; null when nothing is selected. */
let selectedNote = null;
/** In-progress drag (move / resize / grid-offset), or null. */
let drag = null;

// Musical interpretation (ticket 09). Seeded on load; edited via the toolbar.
let bpm = 120;
let division = 16;
let gridAnchor = 0; // grid phase in seconds (auto = first onset, draggable).
let flagThreshold = 0.5;

let audioCtx = null;
let playback = null;

// --- Loading ---------------------------------------------------------------

el.json.addEventListener("change", async () => {
  const file = el.json.files?.[0];
  if (!file) return;
  try {
    const parsed = JSON.parse(await file.text());
    annotation = normalizeAnnotation(parsed);
    workingNotes = annotation.notes.map((n) => ({ ...n, sub_scores: { ...n.sub_scores } }));
    selectedNote = null;
    // Seed the grid from the annotation: BPM as carried, phase on the first onset.
    bpm = annotation.bpm > 0 ? annotation.bpm : 120;
    el.bpm.value = String(bpm);
    gridAnchor = workingNotes.length ? Math.min(...workingNotes.map((n) => n.onset_s)) : 0;
    setStatus(
      `Loaded ${workingNotes.length} notes from “${file.name}” (${annotation.producer}).`,
    );
    rebuild();
  } catch (err) {
    annotation = null;
    workingNotes = [];
    setStatus(`Could not read annotation JSON: ${err.message}`, true);
    rebuild();
  }
});

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
    setStatus(`Could not decode audio: ${err.message}`, true);
    rebuild();
  }
});

/**
 * Validate the shape we depend on and coerce the fields the roll reads. Throws
 * a human-readable error if the file isn't an annotation.
 */
function normalizeAnnotation(raw) {
  if (!raw || typeof raw !== "object" || !Array.isArray(raw.notes)) {
    throw new Error("expected an object with a `notes` array");
  }
  return {
    version: raw.version,
    sample_rate: raw.sample_rate,
    bpm: Number(raw.bpm) || 0,
    duration_s: Number(raw.duration_s) || 0,
    producer: raw.producer ?? "unknown",
    notes: raw.notes.map((n) => ({
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
function spacing() {
  return gridSpacingSeconds(bpm, division);
}

/**
 * The notes as interpreted for display, playback, and export: each working note
 * hard-snapped to the grid (or escaped) plus its flag state. Manual notes never
 * escape — the user placed them deliberately, so they always snap.
 */
function effectiveNotes() {
  const s = spacing();
  return workingNotes.map((n) => {
    // Manual notes never escape — the user placed them, so they always snap.
    // Everything else uses grid.js's documented escape-hatch defaults (ADR-0003).
    const opts = { spacing: s, anchor: gridAnchor };
    if (n.manual) opts.escapeDistanceCells = Infinity;
    const q = quantizeNote(n, opts);
    const flags = noteFlags(n, q, flagThreshold);
    return { raw: n, onset_s: q.onset_s, offset_s: q.offset_s, pitch: n.midi_pitch, ...flags };
  });
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

// --- Toolbar controls -------------------------------------------------------

el.bpm.addEventListener("input", () => {
  const v = Number(el.bpm.value);
  if (v > 0 && Number.isFinite(v)) {
    bpm = v;
    redraw();
  }
});
el.division.addEventListener("change", () => {
  division = Number(el.division.value);
  redraw();
});
el.flag.addEventListener("input", () => {
  flagThreshold = Number(el.flag.value);
  el.flagVal.textContent = flagThreshold.toFixed(2);
  redraw();
});
el.export.addEventListener("click", exportMidi);

// --- Layout / geometry ------------------------------------------------------

/** Total seconds the roll spans: the longer of the notes and the stem. */
function totalDuration() {
  const noteEnd = workingNotes.length
    ? Math.max(annotation?.duration_s ?? 0, ...workingNotes.map((n) => n.offset_s))
    : (annotation?.duration_s ?? 0);
  const audioEnd = audioBuffer ? audioBuffer.duration : 0;
  return Math.max(noteEnd, audioEnd, 1);
}

/** Rebuild the scale, redraw, and refresh which controls are usable. */
function rebuild() {
  resizeCanvas();
  if (annotation) {
    const { min, max } = pitchRange(workingNotes);
    scale = makeScale({
      durationS: totalDuration(),
      minPitch: min,
      maxPitch: max,
      width: el.canvas.width / devicePixelRatioSafe(),
      height: el.canvas.height / devicePixelRatioSafe(),
      padLeft: PAD.left,
      padTop: PAD.top,
      padBottom: PAD.bottom,
    });
  } else {
    scale = null;
  }
  draw();
  el.play.disabled = !(annotation && audioBuffer) || !!playback;
  el.stop.disabled = !playback;
  el.export.disabled = !annotation;
}

/** Redraw without recomputing the scale (grid/edit changes, not resize). */
function redraw() {
  draw(playback ? currentPlayheadS() : null);
}

function devicePixelRatioSafe() {
  return window.devicePixelRatio || 1;
}

/** Size the backing store to the CSS box × DPR so lines stay crisp. */
function resizeCanvas() {
  const dpr = devicePixelRatioSafe();
  const rect = el.canvas.getBoundingClientRect();
  el.canvas.width = Math.max(1, Math.round(rect.width * dpr));
  el.canvas.height = Math.max(1, Math.round(rect.height * dpr));
  ctx2d.setTransform(dpr, 0, 0, dpr, 0, 0);
}

window.addEventListener("resize", rebuild);

// --- Drawing ----------------------------------------------------------------

function cssSize() {
  const dpr = devicePixelRatioSafe();
  return { w: el.canvas.width / dpr, h: el.canvas.height / dpr };
}

function draw(playheadS = null) {
  const { w, h } = cssSize();
  ctx2d.clearRect(0, 0, w, h);
  ctx2d.fillStyle = "#0d0f14";
  ctx2d.fillRect(0, 0, w, h);

  if (!annotation || !scale) {
    drawPlaceholder(w, h);
    return;
  }

  drawPitchGrid(w, h);
  drawTimeGrid(w, h);
  drawRuler(w);
  drawEnergyEnvelope(w, h);
  drawF0Contour();
  drawNotes();
  if (playheadS != null) drawPlayhead(playheadS, h);
}

function drawPlaceholder(w, h) {
  ctx2d.fillStyle = "#5b6072";
  ctx2d.font = "13px system-ui, sans-serif";
  ctx2d.textAlign = "center";
  ctx2d.fillText("Load an annotation JSON to render the roll.", w / 2, h / 2);
  ctx2d.textAlign = "left";
}

/** Horizontal rows per semitone, with note names on the octave lines (C). */
function drawPitchGrid(w, h) {
  ctx2d.font = "10px system-ui, sans-serif";
  ctx2d.textBaseline = "middle";
  for (let pitch = scale.minPitch; pitch <= scale.maxPitch; pitch++) {
    const y = scale.pitchToY(pitch);
    const isC = pitch % 12 === 0;
    if (isBlackKey(pitch)) {
      ctx2d.fillStyle = "#171a22";
      ctx2d.fillRect(PAD.left, y, w - PAD.left, scale.rowHeight);
    }
    ctx2d.strokeStyle = isC ? "#333846" : "#20232c";
    ctx2d.beginPath();
    ctx2d.moveTo(PAD.left, y);
    ctx2d.lineTo(w, y);
    ctx2d.stroke();
    if (isC) {
      ctx2d.fillStyle = "#7a8090";
      ctx2d.fillText(midiToNoteName(pitch), 6, y + scale.rowHeight / 2);
    }
  }
  ctx2d.textBaseline = "alphabetic";
}

function isBlackKey(midi) {
  return [1, 3, 6, 8, 10].includes(((midi % 12) + 12) % 12);
}

/** Each visible grid line's screen x and whether it lands on a beat (quarter). */
function gridLineInfo() {
  const s = spacing();
  const cellsPerBeat = division / 4; // a beat is a quarter note.
  return gridLines(gridAnchor, s, totalDuration()).map((t) => {
    const k = Math.round((t - gridAnchor) / s);
    return { x: scale.timeToX(t), isBeat: ((k % cellsPerBeat) + cellsPerBeat) % cellsPerBeat === 0 };
  });
}

/** Vertical grid lines from BPM/division/phase; beat lines drawn stronger. */
function drawTimeGrid(w, h) {
  for (const { x, isBeat } of gridLineInfo()) {
    ctx2d.strokeStyle = isBeat ? "#2f3646" : "#1c2029";
    ctx2d.beginPath();
    ctx2d.moveTo(x, PAD.top);
    ctx2d.lineTo(x, h);
    ctx2d.stroke();
  }
}

/** The draggable phase ruler across the top, with a tick on each grid line. */
function drawRuler(w) {
  ctx2d.fillStyle = "#171a22";
  ctx2d.fillRect(PAD.left, 0, w - PAD.left, RULER_H);
  ctx2d.strokeStyle = "#2b2f3a";
  ctx2d.beginPath();
  ctx2d.moveTo(PAD.left, RULER_H - 0.5);
  ctx2d.lineTo(w, RULER_H - 0.5);
  ctx2d.stroke();

  for (const { x, isBeat } of gridLineInfo()) {
    ctx2d.strokeStyle = isBeat ? "#6c7488" : "#3a4050";
    ctx2d.beginPath();
    ctx2d.moveTo(x, isBeat ? 3 : RULER_H - 7);
    ctx2d.lineTo(x, RULER_H - 1);
    ctx2d.stroke();
  }
  ctx2d.fillStyle = "#5b6072";
  ctx2d.font = "9px system-ui, sans-serif";
  ctx2d.fillText("drag ↔ grid", 6, RULER_H - 6);
}

/** Energy/RMS envelope as a filled area along the bottom — a loudness guide. */
function drawEnergyEnvelope(w, h) {
  const env = annotation.energy_envelope;
  if (env.length === 0) return;
  const maxRms = Math.max(...env.map((f) => f.rms), 1e-6);
  const band = Math.min(70, h * 0.18);
  const base = h - PAD.bottom;
  ctx2d.beginPath();
  ctx2d.moveTo(scale.timeToX(env[0].time_s), base);
  for (const f of env) {
    ctx2d.lineTo(scale.timeToX(f.time_s), base - (f.rms / maxRms) * band);
  }
  ctx2d.lineTo(scale.timeToX(env[env.length - 1].time_s), base);
  ctx2d.closePath();
  ctx2d.fillStyle = "rgba(110, 168, 254, 0.12)";
  ctx2d.fill();
}

/** f0 contour as a polyline, dimmed where the frame is unvoiced. */
function drawF0Contour() {
  const contour = annotation.f0_contour.filter((f) => f.frequency_hz > 0);
  if (contour.length === 0) return;
  ctx2d.lineWidth = 1.5;
  for (let i = 1; i < contour.length; i++) {
    const a = contour[i - 1];
    const b = contour[i];
    if (b.time_s - a.time_s > 0.12) continue;
    const voicing = (a.voicing + b.voicing) / 2;
    ctx2d.strokeStyle = `rgba(255, 255, 255, ${0.15 + 0.55 * clamp01(voicing)})`;
    ctx2d.beginPath();
    ctx2d.moveTo(scale.timeToX(a.time_s), freqToY(a.frequency_hz));
    ctx2d.lineTo(scale.timeToX(b.time_s), freqToY(b.frequency_hz));
    ctx2d.stroke();
  }
  ctx2d.lineWidth = 1;
}

/** Map a raw f0 (Hz) to the same pitch axis as the notes, via fractional MIDI. */
function freqToY(hz) {
  const midi = 69 + 12 * Math.log2(hz / 440);
  return scale.pitchToY(midi) + scale.rowHeight / 2;
}

/**
 * The (snapped/escaped) note bars, coloured by confidence. Flagged notes get a
 * red outline, escaped notes a dashed cyan outline, the selected note a white
 * ring. Hitboxes carry the working note so edits mutate the source of truth.
 */
function drawNotes() {
  noteHitboxes = [];
  const barH = Math.max(3, scale.rowHeight - 2);
  for (const eff of effectiveNotes()) {
    const x = scale.timeToX(eff.onset_s);
    const y = scale.pitchToY(eff.pitch) + (scale.rowHeight - barH) / 2;
    const wNote = Math.max(3, scale.timeToX(eff.offset_s) - x);
    ctx2d.fillStyle = confidenceColor(eff.raw.confidence);
    roundRect(x, y, wNote, barH, NOTE_RADIUS);
    ctx2d.fill();

    // Outline: escaped (dashed cyan) and flagged (red) are the review cues.
    ctx2d.save();
    if (eff.escaped) {
      ctx2d.setLineDash([4, 3]);
      ctx2d.strokeStyle = ESCAPE_COLOR;
      ctx2d.lineWidth = 2;
    } else if (eff.flagged) {
      ctx2d.strokeStyle = FLAG_COLOR;
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

function roundRect(x, y, w, h, r) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx2d.beginPath();
  ctx2d.moveTo(x + radius, y);
  ctx2d.arcTo(x + w, y, x + w, y + h, radius);
  ctx2d.arcTo(x + w, y + h, x, y + h, radius);
  ctx2d.arcTo(x, y + h, x, y, radius);
  ctx2d.arcTo(x, y, x + w, y, radius);
  ctx2d.closePath();
}

function drawPlayhead(t, h) {
  const x = scale.timeToX(t);
  ctx2d.strokeStyle = "#ffd166";
  ctx2d.lineWidth = 1.5;
  ctx2d.beginPath();
  ctx2d.moveTo(x, 0);
  ctx2d.lineTo(x, h);
  ctx2d.stroke();
  ctx2d.lineWidth = 1;
}

// --- Interaction: select, drag (move/resize), grid-offset, add, delete ------

el.canvas.addEventListener("mousedown", (e) => {
  if (!scale) return;
  const { x, y } = canvasPoint(e);

  // The top ruler drags the grid phase.
  if (y < RULER_H) {
    drag = { mode: "offset", startX: x, origAnchor: gridAnchor, moved: false };
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

window.addEventListener("mousemove", (e) => {
  if (!scale) return;
  if (!drag) return updateHover(e);
  const { x, y } = canvasPoint(e);
  drag.moved = drag.moved || Math.abs(x - drag.startX) > 2 || Math.abs(y - (drag.startY ?? 0)) > 2;
  const dt = scale.xToTime(x) - scale.xToTime(drag.startX);

  if (drag.mode === "offset") {
    gridAnchor = drag.origAnchor + dt;
    redraw();
    return;
  }

  const n = drag.note;
  if (drag.mode === "move") {
    n.onset_s = Math.max(0, drag.origOnset + dt);
    n.offset_s = n.onset_s + (drag.origOffset - drag.origOnset);
    const dRows = scale.yToPitch(y) - scale.yToPitch(drag.startY);
    n.midi_pitch = clampPitch(drag.origPitch + dRows);
    n.manual = true;
  } else if (drag.mode === "resize") {
    // Keep at least a hair of length; the grid snap will round it to a cell.
    n.offset_s = Math.max(n.onset_s + spacing() / 2, drag.origOffset + dt);
    n.manual = true;
  }
  redraw();
  showSelectedTooltip();
});

window.addEventListener("mouseup", () => {
  drag = null;
});

el.canvas.addEventListener("dblclick", (e) => {
  if (!scale || !annotation) return;
  const { x, y } = canvasPoint(e);
  if (y < RULER_H) return;
  const s = spacing();
  const onset = Math.max(0, snapTime(scale.xToTime(x), s, gridAnchor));
  const note = {
    onset_s: onset,
    offset_s: onset + s,
    midi_pitch: clampPitch(scale.yToPitch(y)),
    confidence: 1,
    sub_scores: { pitch_stability: 1, voicing: 1, onset_clarity: 1, offset_clarity: 1 },
    manual: true,
  };
  workingNotes.push(note);
  selectedNote = note;
  redraw();
  showSelectedTooltip();
});

window.addEventListener("keydown", (e) => {
  if (e.key !== "Delete" && e.key !== "Backspace") return;
  // Don't hijack the key while a form control is focused.
  const tag = document.activeElement?.tagName;
  if (tag === "INPUT" || tag === "SELECT" || tag === "TEXTAREA") return;
  if (!selectedNote) return;
  e.preventDefault();
  workingNotes = workingNotes.filter((n) => n !== selectedNote);
  selectedNote = null;
  el.tooltip.style.display = "none";
  redraw();
});

function updateHover(e) {
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

el.canvas.addEventListener("mouseleave", () => {
  if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
});

function canvasPoint(e) {
  const rect = el.canvas.getBoundingClientRect();
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function showSelectedTooltip() {
  const box = noteHitboxes.find((b) => b.note === selectedNote);
  if (box) showTooltip(box.note, box.effective, box.x + box.w / 2, box.y);
}

function hitTest(px, py) {
  for (let i = noteHitboxes.length - 1; i >= 0; i--) {
    const b = noteHitboxes[i];
    if (px >= b.x && px <= b.x + b.w && py >= b.y && py <= b.y + b.h) return b;
  }
  return null;
}

function showTooltip(note, effective, px, py) {
  const weakest = weakestSubScore(note.sub_scores);
  const rows = Object.entries(note.sub_scores)
    .map(([k, v]) => `<dt>${k.replace(/_/g, " ")}</dt><dd>${fmt(v)}</dd>`)
    .join("");
  const cue = effective?.escaped
    ? `<div class="reason" style="color:${ESCAPE_COLOR}">↷ off-grid — kept (escaped)</div>`
    : weakest
      ? `<div class="reason">⚠ ${weakest.label} (${fmt(weakest.value)})</div>`
      : "";
  el.tooltip.innerHTML = `
    <div class="pitch">${midiToNoteName(note.midi_pitch)} · conf ${fmt(note.confidence)}${note.manual ? " · manual" : ""}</div>
    <div>${fmt3(effective?.onset_s ?? note.onset_s)}s – ${fmt3(effective?.offset_s ?? note.offset_s)}s</div>
    ${cue}
    <dl>${rows}</dl>`;
  el.tooltip.style.display = "block";
  const wrapW = el.canvas.getBoundingClientRect().width;
  const tipW = el.tooltip.offsetWidth;
  const left = px + tipW + 16 > wrapW ? px - tipW - 12 : px + 12;
  el.tooltip.style.left = `${Math.max(4, left)}px`;
  el.tooltip.style.top = `${py + 12}px`;
}

function fmt(v) {
  return Number.isFinite(v) ? v.toFixed(2) : "–";
}
function fmt3(v) {
  return Number.isFinite(v) ? v.toFixed(3) : "–";
}
function clampPitch(p) {
  return Math.min(127, Math.max(0, Math.round(p)));
}

// --- Export -----------------------------------------------------------------

/**
 * Export the authoritative MIDI (ADR-0004): the current snapped + corrected
 * state, seconds→ticks at the chosen BPM. Escaped notes export at their raw
 * position, as shown.
 */
function exportMidi() {
  if (!annotation) return;
  const notes = midiNotes();
  const bytes = encodeMidiFile(notes, { bpm });
  const blob = new Blob([bytes], { type: "audio/midi" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = "sona.mid";
  a.click();
  URL.revokeObjectURL(url);
  setStatus(`Exported ${notes.length} notes to sona.mid at ${bpm} BPM.`);
}

// --- Playback ---------------------------------------------------------------

function ensureAudioContext() {
  if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  return audioCtx;
}

el.play.addEventListener("click", () => {
  if (!annotation || !audioBuffer || playback) return;
  startPlayback();
});
el.stop.addEventListener("click", stopPlayback);

el.muteStem.addEventListener("change", () => {
  if (playback) playback.stemGain.gain.value = el.muteStem.checked ? 0 : 1;
});
el.muteSynth.addEventListener("change", () => {
  if (playback) playback.synthGain.gain.value = el.muteSynth.checked ? 0 : 1;
});

function startPlayback() {
  const ac = ensureAudioContext();
  ac.resume();
  const startedAt = ac.currentTime + 0.05;

  const stemGain = ac.createGain();
  stemGain.gain.value = el.muteStem.checked ? 0 : 1;
  stemGain.connect(ac.destination);
  const stemSource = ac.createBufferSource();
  stemSource.buffer = audioBuffer;
  stemSource.connect(stemGain);
  stemSource.start(startedAt);

  // Synth the snapped/corrected notes, so you hear the interpretation, not the raw.
  const synthGain = ac.createGain();
  synthGain.gain.value = el.muteSynth.checked ? 0 : 1;
  synthGain.connect(ac.destination);
  for (const voice of buildSchedule(midiNotes())) {
    scheduleVoice(ac, synthGain, voice, startedAt);
  }

  const duration = totalDuration();
  playback = { startedAt, duration, stemSource, stemGain, synthGain, raf: 0 };
  stemSource.onended = () => {
    if (playback && playback.stemSource === stemSource) stopPlayback();
  };

  el.play.disabled = true;
  el.stop.disabled = false;
  tickPlayhead();
}

/** One oscillator + envelope per note, started/stopped on the shared clock. */
function scheduleVoice(ac, out, voice, startedAt) {
  const osc = ac.createOscillator();
  osc.type = "triangle";
  osc.frequency.value = voice.freq;
  const g = ac.createGain();
  const t0 = startedAt + voice.start;
  const t1 = t0 + voice.duration;
  g.gain.setValueAtTime(0, t0);
  g.gain.linearRampToValueAtTime(voice.gain, t0 + 0.005);
  g.gain.setValueAtTime(voice.gain, Math.max(t0 + 0.005, t1 - 0.01));
  g.gain.linearRampToValueAtTime(0, t1);
  osc.connect(g);
  g.connect(out);
  osc.start(t0);
  osc.stop(t1 + 0.02);
}

function stopPlayback() {
  if (!playback) return;
  const p = playback;
  playback = null;
  try {
    p.stemSource.onended = null;
    p.stemSource.stop();
  } catch {
    // Already stopped/ended — fine.
  }
  cancelAnimationFrame(p.raf);
  el.play.disabled = !(annotation && audioBuffer);
  el.stop.disabled = true;
  draw();
}

/** Seconds elapsed since playback started, on the audio clock. */
function currentPlayheadS() {
  return Math.max(0, ensureAudioContext().currentTime - playback.startedAt);
}

function tickPlayhead() {
  if (!playback) return;
  if (currentPlayheadS() >= playback.duration) {
    stopPlayback();
    return;
  }
  draw(currentPlayheadS());
  playback.raf = requestAnimationFrame(tickPlayhead);
}

// --- Misc -------------------------------------------------------------------

function setStatus(msg, isError = false) {
  el.status.textContent = msg;
  el.status.classList.toggle("error", isError);
}

// First paint (placeholder) once the layout has a size.
requestAnimationFrame(rebuild);
