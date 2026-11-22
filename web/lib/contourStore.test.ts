import { describe, expect, it } from "vitest";
import { packContour, unpackContour } from "./contourStore";
import { hzToCents } from "./sungPitch";

describe("packed contour", () => {
  it("round-trips within a cent and a frame's time, and is much smaller", () => {
    const frames = Array.from({ length: 2000 }, (_, i) => ({ time_s: i * 0.005804988662131519, frequency_hz: 110 + i * 0.3137, voicing: (i % 97) / 96 }));
    const back = unpackContour(JSON.parse(JSON.stringify(packContour(frames))));
    back.forEach((f, i) => {
      expect(Math.abs(f.time_s - frames[i].time_s)).toBeLessThan(1e-4);
      expect(Math.abs(hzToCents(f.frequency_hz) - hzToCents(frames[i].frequency_hz))).toBeLessThan(1);
      expect(Math.abs(f.voicing - frames[i].voicing)).toBeLessThan(1e-3);
    });
    expect(JSON.stringify(packContour(frames)).length).toBeLessThan(JSON.stringify(frames).length / 2);
  });

  it("passes the annotation's own frames through, and anything else as empty", () => {
    const f = [{ time_s: 1, frequency_hz: 220, voicing: 1 }];
    expect(unpackContour(f)).toEqual(f);
    expect(unpackContour(undefined)).toEqual([]);
  });
});
