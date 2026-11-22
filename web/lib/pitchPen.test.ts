import { describe, expect, it } from "vitest";
import { penErase, penStroke } from "./pitchPen";
import { hzToCents, type F0Frame } from "./sungPitch";

const contour = (): F0Frame[] => Array.from({ length: 101 }, (_, i) => ({ time_s: i * 0.01, frequency_hz: 220, voicing: 0.2 }));

describe("pitch pen", () => {
  it("draws a straight line in cents over exactly the frames it crosses, in either direction", () => {
    for (const [t0, c0, t1, c1] of [[0.2, 6000, 0.4, 6200], [0.4, 6200, 0.2, 6000]]) {
      const c = contour();
      expect(penStroke(c, t0, c0, t1, c1)).toBe(21);
      expect(hzToCents(c[30].frequency_hz)).toBeCloseTo(6100, 6);
      expect([c[19].voicing, c[20].voicing, c[40].voicing, c[41].voicing]).toEqual([0.2, 1, 1, 0.2]);
    }
  });

  it("erases to unvoiced so the notes sing there", () => {
    const c = contour();
    expect(penErase(c, 0.5, 0.52)).toBe(3);
    expect(c.slice(49, 54).map((f) => f.voicing)).toEqual([0.2, 0, 0, 0, 0.2]);
  });

  it("marks the nearest frame on a click between frames", () => {
    const c = contour();
    expect(penStroke(c, 0.204, 6000, 0.204, 6000)).toBe(1);
    expect(c[20].voicing).toBe(1);
    expect(penErase(c, 0.207, 0.207)).toBe(1);
    expect(c[21].voicing).toBe(0);
  });
});
