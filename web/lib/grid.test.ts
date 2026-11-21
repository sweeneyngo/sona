import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  gridSpacingSeconds,
  snapTime,
  gridLines,
  quantizeNote,
  resolveOverlaps,
  noteFlags,
} from "./grid";

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

// Drift guard (ADR-0014): the harness carries a Rust port of quantizeNote
// (crates/sona-eval/src/quantize.rs). Both implementations assert this one shared
// case file, so a change to either that the other doesn't mirror fails a test.
interface SharedCase {
  name: string;
  onset_s: number;
  offset_s: number;
  onset_clarity: number | null;
  spacing: number;
  anchor: number;
  escapeDistanceCells?: number;
  onsetConfidenceThreshold?: number;
  expected: { onset_s: number; offset_s: number; escaped: boolean };
}

const sharedCases: SharedCase[] = JSON.parse(
  readFileSync(new URL("../../crates/sona-eval/fixtures/quantize/cases.json", import.meta.url), "utf8"),
);

test("quantizeNote agrees with the Rust port on every shared case", () => {
  assert.ok(sharedCases.length >= 10);
  for (const c of sharedCases) {
    const q = quantizeNote(
      {
        onset_s: c.onset_s,
        offset_s: c.offset_s,
        sub_scores: c.onset_clarity === null ? undefined : { onset_clarity: c.onset_clarity },
      },
      {
        spacing: c.spacing,
        anchor: c.anchor,
        escapeDistanceCells: c.escapeDistanceCells,
        onsetConfidenceThreshold: c.onsetConfidenceThreshold,
      },
    );
    assert.equal(q.escaped, c.expected.escaped, `${c.name}: escaped`);
    assert.ok(Math.abs(q.onset_s - c.expected.onset_s) < 1e-9, `${c.name}: onset ${q.onset_s}`);
    assert.ok(Math.abs(q.offset_s - c.expected.offset_s) < 1e-9, `${c.name}: offset ${q.offset_s}`);
  }
});

interface SequenceCase {
  name: string;
  spacing: number;
  notes: { onset_s: number; offset_s: number }[];
  expected: { onset_s: number; offset_s: number }[];
}

const sequenceCases: SequenceCase[] = JSON.parse(
  readFileSync(new URL("../../crates/sona-eval/fixtures/quantize/sequence_cases.json", import.meta.url), "utf8"),
);

test("resolveOverlaps agrees with the Rust port on every shared sequence case", () => {
  assert.ok(sequenceCases.length >= 5);
  for (const c of sequenceCases) {
    const got = resolveOverlaps(c.notes, c.spacing);
    assert.equal(got.length, c.expected.length, c.name);
    got.forEach((n, i) => {
      assert.ok(Math.abs(n.onset_s - c.expected[i].onset_s) < 1e-9, `${c.name}: onset ${i}`);
      assert.ok(Math.abs(n.offset_s - c.expected[i].offset_s) < 1e-9, `${c.name}: offset ${i}`);
    });
  }
});

test("resolveOverlaps leaves a monophonic line with every note kept", () => {
  let seed = 12345;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31);
  for (let trial = 0; trial < 2000; trial++) {
    const notes = Array.from({ length: 1 + Math.floor(rnd() * 6) }, () => {
      const on = Math.round(rnd() * 8) / 10 + (rnd() < 0.3 ? 0.05 : 0);
      return { onset_s: on, offset_s: on + (1 + Math.floor(rnd() * 3)) / 10 };
    });
    const out = resolveOverlaps(notes, 0.1);
    assert.equal(out.length, notes.length);
    const sorted = [...out].sort((a, b) => a.onset_s - b.onset_s);
    for (let i = 1; i < sorted.length; i++) assert.ok(sorted[i].onset_s >= sorted[i - 1].offset_s - 1e-9, JSON.stringify(notes));
    for (const n of out) assert.ok(n.offset_s > n.onset_s + 1e-9, JSON.stringify(notes));
  }
});
