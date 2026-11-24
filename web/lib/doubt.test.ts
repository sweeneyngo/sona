import { describe, expect, it } from "vitest";
import type { Annotation, StageDoubt, WorkingNote } from "./annotation";
import { DEFAULT_FLAG_SHARE, doubtReason, flagReading, isStageResult } from "./doubt";
import type { Frame } from "./frame";
import { effectiveNotes } from "./interpret";
import { lcg } from "./exportCases";
import { docText, normalizeAnnotation, parseDoc, sessionFile, settingsOnLoad, workingNotesFrom, type SessionState } from "./session";

// 120 BPM, 1/16: a cell is 0.125 s. Every note below starts on a cell line, so none escapes the snap.
const frame: Frame = { bpm: 120, division: 16, anchor: 0, barOne: null };
const CELL = 0.125;
const SUB = { pitch_stability: 0.9, voicing: 0.9, onset_clarity: 0.9, offset_clarity: 0.5 };

/** A note `cells` long starting at cell `at`, with the stage's confidence and doubt. */
function stageNote(at: number, cells: number, confidence: number, doubt: StageDoubt = { length: 0.5, unsure: 0.5 }): WorkingNote {
  return { onset_s: at * CELL, offset_s: (at + cells) * CELL, midi_pitch: 64, confidence, sub_scores: { ...SUB }, stage_doubt: doubt };
}

/** Ten stage notes in a row, two cells each, with these confidences. */
function song(confidences: number[]): WorkingNote[] {
  return confidences.map((c, i) => stageNote(2 * i, 2, c));
}

const settings: SessionState = { bpm: 120, division: 16, gridAnchor: 0, barOne: null, flagThreshold: 0.5, flagShare: DEFAULT_FLAG_SHARE, snapKey: false };

function annotation(engine: string | null, notes: WorkingNote[], producer = engine === "pyin" ? "pyin" : "game"): Annotation {
  const provenance = engine
    ? { sona_version: "0.1.0", sona_commit: null, created_unix_s: 0, input: { role: "input", name: "lead.wav", bytes: 1, sha256: "a".repeat(64) }, settings: { engine }, models: [] }
    : undefined;
  return { version: 1, sample_rate: 44100, bpm: 120, duration_s: 4, producer, notes, f0_contour: [], energy_envelope: [], ...(provenance ? { provenance } : {}) };
}

/** Which notes the roll outlines for review, the way the editor reads them. */
function outlined(a: Annotation, notes: WorkingNote[], s: SessionState = settings): boolean[] {
  return effectiveNotes(notes, frame, { ...flagReading(a, s), snapKey: false, key: null }).map((e) => e.flagged);
}

const flaggedAt = (flags: boolean[]) => flags.flatMap((f, i) => (f ? [i] : []));

describe("flagging a learned-stage result by share", () => {
  const confidences = [0.81, 0.34, 0.77, 0.9, 0.62, 0.55, 0.88, 0.71, 0.45, 0.83];

  it("outlines the least confident 20% of a song's notes by default, and only them", () => {
    const notes = song(confidences);
    expect(DEFAULT_FLAG_SHARE).toBe(0.2);
    expect(flaggedAt(outlined(annotation("game+cells", notes), notes))).toEqual([1, 8]);
  });

  it("outlines the share asked for, rounding to whole notes", () => {
    const notes = song(confidences);
    const at = (share: number) => flaggedAt(outlined(annotation("game+cells", notes), notes, { ...settings, flagShare: share }));
    expect(at(0)).toEqual([]);
    expect(at(0.3)).toEqual([1, 5, 8]);
    expect(at(0.5)).toEqual([1, 4, 5, 7, 8]);
    expect(at(1)).toEqual(confidences.map((_, i) => i));
  });

  it("ranks by the stage's confidence, not by the weakest sub-score", () => {
    // The sub-scores say the opposite of the stage: the surest stage notes have the weakest sub-scores.
    const notes = song(confidences).map((n) => ({ ...n, sub_scores: { ...SUB, offset_clarity: (1 - n.confidence) / 2 } }));
    expect(flaggedAt(outlined(annotation("game+cells", notes), notes))).toEqual([1, 8]);
  });

  it("puts the longer of two equally confident notes first", () => {
    const notes = [stageNote(0, 1, 0.5), stageNote(1, 2, 0.9), stageNote(3, 6, 0.5), stageNote(9, 2, 0.9), stageNote(11, 2, 0.9)];
    expect(flaggedAt(outlined(annotation("game+cells", notes), notes))).toEqual([2]);
  });

  it("ignores the threshold on a stage result", () => {
    const notes = song(confidences);
    for (const flagThreshold of [0, 0.5, 0.85, 1]) {
      expect(flaggedAt(outlined(annotation("game+cells", notes), notes, { ...settings, flagThreshold }))).toEqual([1, 8]);
    }
  });

  it("stops outlining a note once the user has fixed it, without outlining another in its place", () => {
    const notes = song(confidences);
    notes[1] = { ...notes[1], midi_pitch: 65, manual: true };
    expect(flaggedAt(outlined(annotation("game+cells", notes), notes))).toEqual([8]);
  });

  it("treats an older stage annotation without provenance as a stage result when its notes carry the doubt", () => {
    const notes = song(confidences);
    expect(isStageResult(annotation(null, notes))).toBe(true);
    expect(flaggedAt(outlined(annotation(null, notes), notes))).toEqual([1, 8]);
  });

  it("outlines a share of every song, whatever its notes (property)", () => {
    const rand = lcg(14);
    for (let trial = 0; trial < 300; trial++) {
      const n = Math.floor(rand() * 40);
      const notes = Array.from({ length: n }, (_, i) => stageNote(3 * i, 1 + Math.floor(rand() * 2), Math.round(rand() * 20) / 20));
      const share = Math.round(rand() * 20) / 20;
      const flags = outlined(annotation("game+cells", notes), notes, { ...settings, flagShare: share });
      const out = flaggedAt(flags);
      expect(out.length).toBe(Math.round(share * n));
      const worstKept = Math.max(-Infinity, ...out.map((i) => notes[i].confidence));
      const bestLeft = Math.min(Infinity, ...notes.filter((_, i) => !flags[i]).map((x) => x.confidence));
      expect(worstKept).toBeLessThanOrEqual(bestLeft);
    }
  });
});

describe("pYIN and GAME-only results", () => {
  const notes = (): WorkingNote[] => song([0.81, 0.34, 0.77, 0.9, 0.62, 0.55, 0.88, 0.71, 0.45, 0.83]).map(({ stage_doubt: _, ...n }) => n);

  it("are not stage results", () => {
    expect(isStageResult(annotation("pyin", notes()))).toBe(false);
    expect(isStageResult(annotation("game", notes()))).toBe(false);
    expect(isStageResult(annotation(null, notes(), "pyin"))).toBe(false);
  });

  it("are outlined by the threshold exactly as before, whatever the share", () => {
    for (const engine of ["pyin", "game", null]) {
      const a = annotation(engine, notes());
      for (const flagThreshold of [0, 0.5, 0.7, 1]) {
        const before = effectiveNotes(notes(), frame, { flagThreshold, snapKey: false, key: null });
        for (const flagShare of [0, 0.2, 1]) {
          expect(effectiveNotes(notes(), frame, { ...flagReading(a, { ...settings, flagThreshold, flagShare }), snapKey: false, key: null })).toEqual(before);
        }
      }
    }
    expect(flaggedAt(outlined(annotation("pyin", notes()), notes()))).toEqual([1, 8]);
    expect(flaggedAt(outlined(annotation("game", notes()), notes(), { ...settings, flagThreshold: 0.7 }))).toEqual([1, 4, 5, 8]);
  });
});

describe("why a stage note is doubtful", () => {
  it("names a long note when its length weighs most", () => {
    expect(doubtReason(stageNote(0, 8, 0.4, { length: 1.21, unsure: 0.3 }))).toBe("long note, may hide several syllables");
  });

  it("names the model's doubt when its uncertainty weighs most", () => {
    expect(doubtReason(stageNote(0, 2, 0.4, { length: 0.4, unsure: 1.8 }))).toBe("the model was unsure here");
  });

  it("says nothing for a note the stage didn't make", () => {
    const { stage_doubt: _, ...plain } = stageNote(0, 2, 0.4);
    expect(doubtReason(plain)).toBeNull();
  });
});

describe("the share is part of the song's session", () => {
  const stageFile = () => JSON.parse(JSON.stringify(annotation("game+cells", song([0.3, 0.9]))));

  it("keeps each note's doubt through loading and saving", () => {
    const raw = stageFile();
    expect(normalizeAnnotation(raw).notes[0].stage_doubt).toEqual({ length: 0.5, unsure: 0.5 });
    expect(workingNotesFrom(raw.notes)[1].stage_doubt).toEqual({ length: 0.5, unsure: 0.5 });
    expect(workingNotesFrom(annotation("pyin", [{ ...stageNote(0, 1, 0.5), stage_doubt: undefined }]).notes)[0]).not.toHaveProperty("stage_doubt");
  });

  it("reopens with the share it was saved with, and an older session keeps the current one", () => {
    const raw = stageFile();
    const a = normalizeAnnotation(raw);
    const notes = workingNotesFrom(raw.notes);
    const saved = JSON.parse(JSON.stringify(sessionFile(a, notes, a.notes, { ...settings, flagShare: 0.35 })));
    expect(settingsOnLoad(saved, a, notes, settings).flagShare).toBe(0.35);
    const older = { ...saved, session: { ...saved.session, flagShare: undefined } };
    expect(settingsOnLoad(older, a, notes, { ...settings, flagShare: 0.1 }).flagShare).toBe(0.1);
    expect(settingsOnLoad({ ...saved, session: { ...saved.session, flagShare: 3 } }, a, notes, settings).flagShare).toBe(1);
  });

  it("is restored by undo", () => {
    const back = parseDoc(docText(song([0.3]), { ...settings, flagShare: 0.4 }));
    expect(back.settings.flagShare).toBe(0.4);
  });
});
