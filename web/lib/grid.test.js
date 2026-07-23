import { test } from "node:test";
import assert from "node:assert/strict";

import {
  gridSpacingSeconds,
  snapTime,
  gridLines,
  quantizeNote,
  noteFlags,
} from "./grid.js";

test("gridSpacingSeconds: a 16th at 120 BPM is 0.125 s", () => {
  assert.ok(Math.abs(gridSpacingSeconds(120, 16) - 0.125) < 1e-12);
});

test("gridSpacingSeconds: a quarter (÷4) at 60 BPM is 1 s", () => {
  assert.ok(Math.abs(gridSpacingSeconds(60, 4) - 1) < 1e-12);
});

test("gridSpacingSeconds: finer divisions are proportionally shorter", () => {
  assert.ok(Math.abs(gridSpacingSeconds(120, 8) - 0.25) < 1e-12);
  assert.ok(Math.abs(gridSpacingSeconds(120, 32) - 0.0625) < 1e-12);
});

test("snapTime rounds to the nearest grid line relative to the anchor", () => {
  // spacing 0.1, anchor 0.05 → lines at .05,.15,.25,...
  assert.ok(Math.abs(snapTime(0.17, 0.1, 0.05) - 0.15) < 1e-12);
  assert.ok(Math.abs(snapTime(0.21, 0.1, 0.05) - 0.25) < 1e-12);
  assert.ok(Math.abs(snapTime(0.05, 0.1, 0.05) - 0.05) < 1e-12);
});

test("gridLines covers [0, duration] stepping from the anchor", () => {
  const lines = gridLines(0.05, 0.1, 0.32);
  // Lines at .05,.15,.25 land inside; the anchor itself is included.
  assert.deepEqual(
    lines.map((t) => Number(t.toFixed(3))),
    [0.05, 0.15, 0.25],
  );
});

test("gridLines includes lines before the anchor when the anchor is > 0", () => {
  const lines = gridLines(0.25, 0.1, 0.4).map((t) => Number(t.toFixed(3)));
  // From anchor .25 back to 0: .05,.15 also appear.
  assert.ok(lines.includes(0.05));
  assert.ok(lines.includes(0.35));
});

test("quantizeNote hard-snaps onset and offset to the grid", () => {
  const q = quantizeNote(
    { onset_s: 0.09, offset_s: 0.31, sub_scores: { onset_clarity: 0.5 } },
    { spacing: 0.1, anchor: 0 },
  );
  assert.ok(Math.abs(q.onset_s - 0.1) < 1e-12);
  assert.ok(Math.abs(q.offset_s - 0.3) < 1e-12);
  assert.equal(q.escaped, false);
});

test("quantizeNote guarantees at least a one-cell note", () => {
  // Onset and offset would snap to the same line.
  const q = quantizeNote(
    { onset_s: 0.11, offset_s: 0.13, sub_scores: { onset_clarity: 0.2 } },
    { spacing: 0.1, anchor: 0 },
  );
  assert.ok(q.offset_s - q.onset_s >= 0.1 - 1e-12);
});

test("escape hatch: far-from-grid + high onset confidence stays put", () => {
  // 0.15 is half a cell off the nearest line (0.1 or 0.2) — maximally far.
  const q = quantizeNote(
    { onset_s: 0.15, offset_s: 0.35, sub_scores: { onset_clarity: 0.95 } },
    { spacing: 0.1, anchor: 0, escapeDistanceCells: 0.25, onsetConfidenceThreshold: 0.8 },
  );
  assert.equal(q.escaped, true);
  assert.equal(q.onset_s, 0.15, "raw onset preserved");
  assert.equal(q.offset_s, 0.35, "raw offset preserved");
});

test("escape hatch does NOT fire for a far note with low onset confidence", () => {
  const q = quantizeNote(
    { onset_s: 0.15, offset_s: 0.35, sub_scores: { onset_clarity: 0.3 } },
    { spacing: 0.1, anchor: 0, escapeDistanceCells: 0.25, onsetConfidenceThreshold: 0.8 },
  );
  assert.equal(q.escaped, false);
  // It gets snapped instead — onto one of the two adjacent lines (0.15 is
  // equidistant, so float rounding may land on either).
  assert.ok(Math.abs(q.onset_s - 0.1) < 1e-9 || Math.abs(q.onset_s - 0.2) < 1e-9);
});

test("escape hatch does NOT fire for a near-grid note even at high confidence", () => {
  const q = quantizeNote(
    { onset_s: 0.11, offset_s: 0.29, sub_scores: { onset_clarity: 0.99 } },
    { spacing: 0.1, anchor: 0, escapeDistanceCells: 0.25, onsetConfidenceThreshold: 0.8 },
  );
  assert.equal(q.escaped, false);
});

test("noteFlags flags a note below the confidence threshold", () => {
  const f = noteFlags({ confidence: 0.4 }, { escaped: false }, 0.5);
  assert.equal(f.flagged, true);
  assert.equal(f.lowConfidence, true);
  assert.equal(f.escaped, false);
});

test("noteFlags does not flag a confident, snapped note", () => {
  const f = noteFlags({ confidence: 0.9 }, { escaped: false }, 0.5);
  assert.equal(f.flagged, false);
});

test("noteFlags flags an escaped note regardless of its scalar", () => {
  const f = noteFlags({ confidence: 0.95 }, { escaped: true }, 0.5);
  assert.equal(f.flagged, true);
  assert.equal(f.escaped, true);
});
