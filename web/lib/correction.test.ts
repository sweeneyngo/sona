import { test } from "vitest";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { alignNotes, correctionCost, correctionChanges, type NoteLike } from "./correction";

/** A note from (onset, offset, midi). */
function n(onset_s: number, offset_s: number, midi_pitch: number): NoteLike {
  return { onset_s, offset_s, midi_pitch };
}

test("identical sets cost nothing", () => {
  const notes = [n(0, 0.5, 60), n(0.6, 1.0, 62)];
  const c = correctionCost(notes, notes);
  assert.deepEqual(c, {
    added: 0,
    deleted: 0,
    pitchFixed: 0,
    onsetMoved: 0,
    offsetMoved: 0,
    total: 0,
  });
});

test("a baseline note with no final match is a deletion", () => {
  const baseline = [n(0, 0.5, 60), n(2.0, 2.5, 72)];
  const final = [n(0, 0.5, 60)];
  const c = correctionCost(baseline, final);
  assert.equal(c.deleted, 1);
  assert.equal(c.added, 0);
  assert.equal(c.total, 1);
});

test("a final note with no baseline match is an addition", () => {
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0, 0.5, 60), n(2.0, 2.5, 72)];
  const c = correctionCost(baseline, final);
  assert.equal(c.added, 1);
  assert.equal(c.deleted, 0);
  assert.equal(c.total, 1);
});

test("a re-pitched note is one pitch fix, not a delete + add", () => {
  // Same timing, pitch nudged up a semitone: the note is paired (onset ignored
  // for matching), so it reads as one correction, not two.
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0, 0.5, 61)];
  const c = correctionCost(baseline, final);
  assert.equal(c.pitchFixed, 1);
  assert.equal(c.added, 0);
  assert.equal(c.deleted, 0);
  assert.equal(c.total, 1);
});

test("an onset nudged past the tolerance counts as onset-moved", () => {
  // 80 ms later > 50 ms tol, but within the 150 ms match window → still paired.
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0.08, 0.5, 60)];
  const c = correctionCost(baseline, final);
  assert.equal(c.onsetMoved, 1);
  assert.equal(c.offsetMoved, 0);
  assert.equal(c.pitchFixed, 0);
  assert.equal(c.total, 1);
});

test("an onset nudged within tolerance is not a correction", () => {
  // 30 ms < 50 ms tol → the note is considered unchanged.
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0.03, 0.5, 60)];
  const c = correctionCost(baseline, final);
  assert.equal(c.total, 0);
});

test("an offset dragged past the tolerance counts as offset-moved", () => {
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0, 0.7, 60)];
  const c = correctionCost(baseline, final);
  assert.equal(c.offsetMoved, 1);
  assert.equal(c.onsetMoved, 0);
  assert.equal(c.total, 1);
});

test("a note both re-pitched and re-timed counts in both categories", () => {
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0.08, 0.5, 62)];
  const c = correctionCost(baseline, final);
  assert.equal(c.pitchFixed, 1);
  assert.equal(c.onsetMoved, 1);
  assert.equal(c.total, 2);
});

test("matching is one-to-one under contention", () => {
  // Two final notes near one baseline note: only one pairs; the other is an add.
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0, 0.5, 60), n(0.03, 0.5, 60)];
  const { matched, added, deleted } = alignNotes(baseline, final);
  assert.equal(matched.length, 1);
  assert.equal(added.length, 1);
  assert.equal(deleted.length, 0);
  // The nearer onset wins the pairing.
  assert.equal(matched[0].final.onset_s, 0);
});

test("a timing move larger than the match window reads as delete + add", () => {
  // 300 ms > 150 ms window: the notes don't pair, so it's a deletion and an
  // addition (2 units) rather than one itemised move — documented behaviour.
  const baseline = [n(0, 0.5, 60)];
  const final = [n(0.3, 0.8, 60)];
  const c = correctionCost(baseline, final);
  assert.equal(c.deleted, 1);
  assert.equal(c.added, 1);
  assert.equal(c.onsetMoved, 0);
  assert.equal(c.total, 2);
});

test("weights scale the roll-up but leave raw counts intact", () => {
  const baseline = [n(0, 0.5, 60), n(2.0, 2.5, 72)]; // second note will be deleted
  const final = [n(0, 0.5, 61)]; // first re-pitched
  const weights = {
    added: 1,
    deleted: 3,
    pitchFixed: 2,
    onsetMoved: 1,
    offsetMoved: 1,
  };
  const c = correctionCost(baseline, final, undefined, weights);
  assert.equal(c.deleted, 1);
  assert.equal(c.pitchFixed, 1);
  // 1 deletion × 3 + 1 pitch fix × 2 = 5.
  assert.equal(c.total, 5);
});

test("empty sides are handled", () => {
  assert.equal(correctionCost([], []).total, 0);
  assert.equal(correctionCost([], [n(0, 0.5, 60)]).added, 1);
  assert.equal(correctionCost([n(0, 0.5, 60)], []).deleted, 1);
});

test("correctionChanges itemises each correction", () => {
  const baseline = [n(0, 0.5, 60), n(0.6, 1.0, 62), n(1.2, 1.6, 64)];
  const final = [n(0, 0.5, 61), n(0.68, 1.0, 62)]; // re-pitch first, nudge second, drop third
  const changes = correctionChanges(baseline, final);
  const kinds = changes.map((c) => c.kind).sort();
  assert.deepEqual(kinds, ["deleted", "onset", "pitch"]);
  // The list length matches the unit-weighted total.
  assert.equal(changes.length, correctionCost(baseline, final).total);
  // A pitch change carries both endpoints.
  const pitch = changes.find((c) => c.kind === "pitch")!;
  assert.equal(pitch.from!.midi_pitch, 60);
  assert.equal(pitch.to!.midi_pitch, 61);
});

test("a note both re-pitched and re-timed yields two change entries", () => {
  const changes = correctionChanges([n(0, 0.5, 60)], [n(0.08, 0.5, 62)]);
  assert.deepEqual(changes.map((c) => c.kind).sort(), ["onset", "pitch"]);
});

interface SharedCase {
  name: string;
  ust: [number, number, number][];
  output: [number, number, number][];
  correction: { added: number; deleted: number; pitchFixed: number; onsetMoved: number; offsetMoved: number };
}

const sharedCases: SharedCase[] = JSON.parse(
  readFileSync(new URL("../../crates/sona-eval/fixtures/correction/cases.json", import.meta.url), "utf8"),
);

test("correctionCost agrees with the Rust scorers on every shared case", () => {
  assert.ok(sharedCases.length >= 6);
  const notes = (v: [number, number, number][]) => v.map(([on, off, p]) => n(on, off, p));
  for (const c of sharedCases) {
    const got = correctionCost(notes(c.output), notes(c.ust));
    const { added, deleted, pitchFixed, onsetMoved, offsetMoved } = got;
    assert.deepEqual({ added, deleted, pitchFixed, onsetMoved, offsetMoved }, c.correction, c.name);
  }
});
