import { describe, expect, it } from "vitest";
import { NEUTRAL_EXPRESSION, isExpression, type Expression } from "./expression";
import { addPoint, dragHandle, handlesFor, removePoint, type Handle, type NoteSpan } from "./pitchHandles";

const span: NoteSpan = { onset_s: 10, offset_s: 11, tone: 64, touchingPrevTone: 60 };
const expr: Expression = {
  points: [
    { x: -80, y: 0, shape: "io" },
    { x: 40, y: 0, shape: "io" },
    { x: 600, y: -5, shape: "io" },
  ],
  snapFirst: true,
  vibrato: { length: 50, period: 200, depth: 30, in: 20, out: 10, shift: 0, drift: 0 },
};

describe("handlesFor", () => {
  it("puts points at note time and their pitch, the first one snapped to the previous note", () => {
    const h = handlesFor(expr, span);
    expect(h.slice(0, 3).map((x) => [x.t, x.pitch])).toEqual([
      [9.92, 60],
      [10.04, 64],
      [10.6, 63.5],
    ]);
  });

  it("puts the vibrato's start, fade ends and one period where OpenUtau draws them", () => {
    const v = Object.fromEntries(handlesFor(expr, span).slice(3).map((x) => [x.handle.kind, [x.t, x.pitch]]));
    expect(v["vibrato-start"]).toEqual([10.5, 64]);
    expect(v["vibrato-fade-in"][0]).toBeCloseTo(10.6, 9);
    expect(v["vibrato-fade-out"][0]).toBeCloseTo(10.95, 9);
    expect(v["vibrato-fade-in"][1]).toBeCloseTo(64.3, 9);
    expect(v["vibrato-period"][0]).toBeCloseTo(10.7, 9);
  });

  it("shows no vibrato handles without a vibrato", () => {
    expect(handlesFor(NEUTRAL_EXPRESSION, span).every((h) => h.handle.kind === "point")).toBe(true);
  });
});

describe("dragHandle", () => {
  it("moves a point in time and cents, but never past its neighbours", () => {
    const e = dragHandle(expr, span, { kind: "point", index: 1 }, 0.05, 25);
    expect(e.points[1]).toEqual({ x: 90, y: 2.5, shape: "io" });
    expect(dragHandle(expr, span, { kind: "point", index: 1 }, 5, 0).points[1].x).toBe(599);
    expect(dragHandle(expr, span, { kind: "point", index: 1 }, -5, 0).points[1].x).toBe(-79);
    expect(expr.points[1].x).toBe(40);
  });

  it("keeps the first point's height while it snaps to the previous note", () => {
    expect(dragHandle(expr, span, { kind: "point", index: 0 }, 0.02, 300).points[0]).toEqual({ x: -60, y: 0, shape: "io" });
    const free = dragHandle({ ...expr, snapFirst: false }, span, { kind: "point", index: 0 }, 0, 300);
    expect(free.points[0].y).toBe(30);
  });

  it("turns vibrato drags into OpenUtau's parameters, within its limits", () => {
    expect(dragHandle(expr, span, { kind: "vibrato-start" }, -0.2, 0).vibrato!.length).toBe(70);
    const fade = dragHandle(expr, span, { kind: "vibrato-fade-in" }, 0.1, 500).vibrato!;
    expect([fade.in, fade.depth]).toEqual([40, 200]);
    expect(dragHandle(expr, span, { kind: "vibrato-fade-out" }, -0.05, -100).vibrato!.depth).toBe(5);
    expect(dragHandle(expr, span, { kind: "vibrato-fade-out" }, -0.05, 0).vibrato!.out).toBe(20);
    expect(dragHandle(expr, span, { kind: "vibrato-period" }, 1, 0).vibrato!.period).toBe(500);
  });
});

describe("addPoint / removePoint", () => {
  it("inserts in time order and refuses to drop below two points", () => {
    const { expr: e, index } = addPoint(expr, 300, 12.34);
    expect(index).toBe(2);
    expect(e.points.map((p) => p.x)).toEqual([-80, 40, 300, 600]);
    expect(e.points[2].y).toBe(12.3);
    expect(removePoint(e, 2).points.map((p) => p.x)).toEqual([-80, 40, 600]);
    expect(removePoint(NEUTRAL_EXPRESSION, 0)).toBe(NEUTRAL_EXPRESSION);
  });
});

describe("any sequence of drags", () => {
  it("leaves points in time order and the vibrato inside OpenUtau's limits", () => {
    const handles: Handle[] = [
      { kind: "point", index: 0 },
      { kind: "point", index: 1 },
      { kind: "point", index: 2 },
      { kind: "vibrato-start" },
      { kind: "vibrato-fade-in" },
      { kind: "vibrato-fade-out" },
      { kind: "vibrato-period" },
    ];
    let seed = 7;
    const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2 ** 31) / 2 ** 31) * 2 - 1;
    let e = expr;
    for (let step = 0; step < 5000; step++) {
      e = dragHandle(e, span, handles[Math.floor(((rnd() + 1) / 2) * handles.length)], rnd() * 2, rnd() * 400);
      expect(isExpression(e)).toBe(true);
      for (let i = 1; i < e.points.length; i++) expect(e.points[i].x).toBeGreaterThan(e.points[i - 1].x);
      const v = e.vibrato!;
      expect(v.in + v.out).toBeLessThanOrEqual(100);
      expect([v.length >= 0 && v.length <= 100, v.depth >= 5 && v.depth <= 200, v.period >= 5 && v.period <= 500]).toEqual([true, true, true]);
    }
  });
});
