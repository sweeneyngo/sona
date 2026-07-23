// sona review surface — browser layer (ticket 08).
//
// Loads an annotation JSON + the vocal stem, renders the piano-roll on a
// <canvas> (notes positioned by raw seconds, coloured by confidence, with the
// f0 contour and energy envelope drawn underneath as guides), and plays the
// stem against synthesized notes in sync via Web Audio. Grid/quantization,
// correction, and MIDI export are out of scope here — tickets 09–10.
//
// The pure geometry/colour/schedule maths live in ./lib/*.js so they can be
// unit-tested under `node --test`; this file is the DOM + Web Audio plumbing
// that can only run in a browser.

import {
  midiToNoteName,
  confidenceColor,
  weakestSubScore,
  pitchRange,
  makeScale,
} from "./lib/view.js";
import { buildSchedule } from "./lib/synth.js";
import { clamp01 } from "./lib/math.js";

const PAD = { left: 44, top: 12, bottom: 22 };
const NOTE_RADIUS = 3;

const el = {
  json: document.getElementById("json-input"),
  audio: document.getElementById("audio-input"),
  play: document.getElementById("play"),
  stop: document.getElementById("stop"),
  muteStem: document.getElementById("mute-stem"),
  muteSynth: document.getElementById("mute-synth"),
  status: document.getElementById("status"),
  canvas: document.getElementById("roll"),
  tooltip: document.getElementById("tooltip"),
};
const ctx2d = el.canvas.getContext("2d");

// --- State -----------------------------------------------------------------

/** @type {import("./annotation.d.ts").Annotation | null} */
let annotation = null;
/** Decoded stem, for playback and for the total roll duration. */
let audioBuffer = null;
/** Rendering geometry, rebuilt on load and resize. Null until data arrives. */
let scale = null;
/** Screen rects of drawn notes, for hover hit-testing. */
let noteHitboxes = [];
/** The note the user clicked to pin its reason open; null when nothing is selected. */
let selectedNote = null;

let audioCtx = null;
let playback = null; // { startedAt, duration, nodes, raf } while playing.

// --- Loading ---------------------------------------------------------------

el.json.addEventListener("change", async () => {
  const file = el.json.files?.[0];
  if (!file) return;
  try {
    const parsed = JSON.parse(await file.text());
    annotation = normalizeAnnotation(parsed);
    selectedNote = null; // the old selection points at a note that's gone now.
    setStatus(
      `Loaded ${annotation.notes.length} notes from “${file.name}” (${annotation.producer}).`,
    );
    rebuild();
  } catch (err) {
    annotation = null;
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
    bpm: raw.bpm,
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

// --- Layout / geometry ------------------------------------------------------

/** Total seconds the roll spans: the longer of the annotation and the stem. */
function totalDuration() {
  const annotEnd = annotation
    ? Math.max(annotation.duration_s, ...annotation.notes.map((n) => n.offset_s), 0)
    : 0;
  const audioEnd = audioBuffer ? audioBuffer.duration : 0;
  return Math.max(annotEnd, audioEnd, 1);
}

/** Rebuild the scale, redraw, and refresh which controls are usable. */
function rebuild() {
  resizeCanvas();
  if (annotation) {
    const { min, max } = pitchRange(annotation.notes);
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
  // Playback needs both a stem to play and notes to synth against.
  el.play.disabled = !(annotation && audioBuffer) || !!playback;
  el.stop.disabled = !playback;
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
    // Shade the black keys faintly so the roll reads like a keyboard.
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
    // A gap in time means an unvoiced stretch — don't connect across it.
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

/** The note bars, coloured by confidence; records hitboxes for hover. */
function drawNotes() {
  noteHitboxes = [];
  const barH = Math.max(3, scale.rowHeight - 2);
  for (const note of annotation.notes) {
    const x = scale.timeToX(note.onset_s);
    const y = scale.pitchToY(note.midi_pitch) + (scale.rowHeight - barH) / 2;
    const wNote = Math.max(3, scale.timeToX(note.offset_s) - x);
    ctx2d.fillStyle = confidenceColor(note.confidence);
    roundRect(x, y, wNote, barH, NOTE_RADIUS);
    ctx2d.fill();
    // The selected note gets a bright ring so the pinned reason has an anchor.
    const isSelected = note === selectedNote;
    ctx2d.strokeStyle = isSelected ? "#ffffff" : "rgba(0,0,0,0.4)";
    ctx2d.lineWidth = isSelected ? 2 : 1;
    ctx2d.stroke();
    ctx2d.lineWidth = 1;
    noteHitboxes.push({ x, y, w: wNote, h: barH, note });
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

// --- Hover / select / tooltip -----------------------------------------------
//
// Hover previews a note's reason transiently; clicking a note *pins* it
// (`selectedNote`) so the reason stays open with a highlight ring — the
// "hover/select" the ticket asks for. Clicking empty space clears the pin.

el.canvas.addEventListener("mousemove", (e) => {
  const { x, y } = canvasPoint(e);
  const hit = hitTest(x, y);
  if (hit) showTooltip(hit.note, x, y);
  else if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
});
el.canvas.addEventListener("mouseleave", () => {
  if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
});
el.canvas.addEventListener("click", (e) => {
  const { x, y } = canvasPoint(e);
  const hit = hitTest(x, y);
  // Clicking the pinned note again unpins it; clicking empty space clears it.
  selectedNote = hit && hit.note !== selectedNote ? hit.note : null;
  draw(playback ? currentPlayheadS() : null);
  if (selectedNote) showSelectedTooltip();
  else el.tooltip.style.display = "none";
});

function canvasPoint(e) {
  const rect = el.canvas.getBoundingClientRect();
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

/** Pin the tooltip to the selected note's own bar, so it survives mouse-out. */
function showSelectedTooltip() {
  const box = noteHitboxes.find((b) => b.note === selectedNote);
  if (box) showTooltip(selectedNote, box.x + box.w / 2, box.y);
}

function hitTest(px, py) {
  // Last drawn wins (topmost), so search in reverse.
  for (let i = noteHitboxes.length - 1; i >= 0; i--) {
    const b = noteHitboxes[i];
    if (px >= b.x && px <= b.x + b.w && py >= b.y && py <= b.y + b.h) return b;
  }
  return null;
}

function showTooltip(note, px, py) {
  const weakest = weakestSubScore(note.sub_scores);
  const rows = Object.entries(note.sub_scores)
    .map(([k, v]) => `<dt>${k.replace(/_/g, " ")}</dt><dd>${fmt(v)}</dd>`)
    .join("");
  el.tooltip.innerHTML = `
    <div class="pitch">${midiToNoteName(note.midi_pitch)} · conf ${fmt(note.confidence)}</div>
    <div>${note.onset_s.toFixed(3)}s – ${note.offset_s.toFixed(3)}s</div>
    ${weakest ? `<div class="reason">⚠ ${weakest.label} (${fmt(weakest.value)})</div>` : ""}
    <dl>${rows}</dl>`;
  el.tooltip.style.display = "block";
  // Keep the tooltip inside the wrap; flip left near the right edge.
  const wrapW = el.canvas.getBoundingClientRect().width;
  const tipW = el.tooltip.offsetWidth;
  const left = px + tipW + 16 > wrapW ? px - tipW - 12 : px + 12;
  el.tooltip.style.left = `${Math.max(4, left)}px`;
  el.tooltip.style.top = `${py + 12}px`;
}

function fmt(v) {
  return Number.isFinite(v) ? v.toFixed(2) : "–";
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
  const startedAt = ac.currentTime + 0.05; // brief lead-in so both sources align.
  const nodes = [];

  // Both the stem and the synth start at the same audio-clock instant, which is
  // what keeps them in sync — the whole point of the review surface.
  const stemGain = ac.createGain();
  stemGain.gain.value = el.muteStem.checked ? 0 : 1;
  stemGain.connect(ac.destination);
  const stemSource = ac.createBufferSource();
  stemSource.buffer = audioBuffer;
  stemSource.connect(stemGain);
  stemSource.start(startedAt);
  nodes.push(stemSource);

  const synthGain = ac.createGain();
  synthGain.gain.value = el.muteSynth.checked ? 0 : 1;
  synthGain.connect(ac.destination);
  for (const voice of buildSchedule(annotation.notes)) {
    scheduleVoice(ac, synthGain, voice, startedAt);
  }

  const duration = totalDuration();
  playback = { startedAt, duration, stemSource, stemGain, synthGain, raf: 0 };
  stemSource.onended = () => {
    // Fires on natural end and on stop(); only auto-stop if still current.
    if (playback && playback.stemSource === stemSource) stopPlayback();
  };

  el.play.disabled = true;
  el.stop.disabled = false;
  tickPlayhead();
}

/** One oscillator + envelope per note, started/stopped on the shared clock. */
function scheduleVoice(ac, out, voice, startedAt) {
  const osc = ac.createOscillator();
  osc.type = "triangle"; // softer than a saw; reads as a reference tone.
  osc.frequency.value = voice.freq;
  const g = ac.createGain();
  const t0 = startedAt + voice.start;
  const t1 = t0 + voice.duration;
  // Tiny attack/release ramps to avoid clicks at note edges.
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
  playback = null; // clear first so onended doesn't recurse.
  try {
    p.stemSource.onended = null;
    p.stemSource.stop();
  } catch {
    // Already stopped/ended — fine.
  }
  cancelAnimationFrame(p.raf);
  el.play.disabled = !(annotation && audioBuffer);
  el.stop.disabled = true;
  draw(); // clear the playhead.
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
