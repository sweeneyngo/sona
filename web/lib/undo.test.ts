import { describe, expect, it } from "vitest";
import { SharedSnapshot, Undo } from "./undo";

function setup(limit = 200) {
  const doc = { items: [1, 2, 3] };
  let t = 0;
  const undo = new Undo<string>({
    capture: () => JSON.stringify(doc.items),
    restore: (s) => {
      doc.items = JSON.parse(s);
    },
    equal: (a, b) => a === b,
    limit,
    now: () => t,
  });
  return { doc, undo, tick: (ms: number) => (t += ms) };
}

describe("undo", () => {
  it("undoes and redoes one edit at a time", () => {
    const { doc, undo } = setup();
    doc.items.push(4);
    undo.changed();
    doc.items[0] = 9;
    undo.changed();
    expect(undo.undo()).toBe(true);
    expect(doc.items).toEqual([1, 2, 3, 4]);
    expect(undo.undo()).toBe(true);
    expect(doc.items).toEqual([1, 2, 3]);
    expect(undo.undo()).toBe(false);
    expect(undo.redo()).toBe(true);
    expect(undo.redo()).toBe(true);
    expect(doc.items).toEqual([9, 2, 3, 4]);
  });

  it("makes a whole drag one step, and a drag that changed nothing no step", () => {
    const { doc, undo } = setup();
    undo.beginGesture();
    for (let i = 0; i < 20; i++) {
      doc.items[1] = 10 + i;
      undo.changed();
    }
    expect(undo.canUndo).toBe(false);
    undo.endGesture();
    undo.beginGesture();
    undo.endGesture();
    undo.undo();
    expect(doc.items).toEqual([1, 2, 3]);
    expect(undo.canUndo).toBe(false);
  });

  it("merges a burst from one source, but not across a pause or another source", () => {
    const { doc, undo, tick } = setup();
    for (const v of [1, 12, 125]) {
      doc.items[0] = v;
      undo.changed("bpm");
      tick(300);
    }
    tick(5000);
    doc.items[0] = 126;
    undo.changed("bpm");
    doc.items[2] = 0;
    undo.changed();
    undo.undo();
    undo.undo();
    expect(doc.items).toEqual([125, 2, 3]);
    undo.undo();
    expect(doc.items).toEqual([1, 2, 3]);
  });

  it("a burst that ends where it started leaves no step", () => {
    const { doc, undo } = setup();
    for (const v of [9, 8, 1]) {
      doc.items[0] = v;
      undo.changed("bpm");
    }
    expect(undo.canUndo).toBe(false);
  });

  it("a new edit after an undo drops the redo branch", () => {
    const { doc, undo } = setup();
    doc.items.push(4);
    undo.changed();
    undo.undo();
    doc.items.push(5);
    undo.changed();
    expect(undo.canRedo).toBe(false);
    expect(undo.redo()).toBe(false);
    expect(doc.items).toEqual([1, 2, 3, 5]);
  });

  it("an unreported change made before Undo is kept as its own step, not lost", () => {
    const { doc, undo } = setup();
    doc.items.push(4);
    undo.changed();
    doc.items.push(5);
    undo.undo();
    expect(doc.items).toEqual([1, 2, 3, 4]);
    undo.redo();
    expect(doc.items).toEqual([1, 2, 3, 4, 5]);
  });

  it("forgets everything on reset, and keeps at most `limit` steps", () => {
    const { doc, undo } = setup(3);
    for (let i = 0; i < 5; i++) {
      doc.items.push(i);
      undo.changed();
    }
    let n = 0;
    while (undo.undo() && n < 100) n++;
    expect(n).toBe(3);
    undo.reset();
    expect([undo.canUndo, undo.canRedo]).toEqual([false, false]);
  });

  it("walking back through every step and forward again replays the exact history, whatever the edits", () => {
    let seed = 11;
    const rnd = (n: number) => (seed = (seed * 1103515245 + 12345) % 2 ** 31) % n;
    for (let trial = 0; trial < 300; trial++) {
      const { doc, undo, tick } = setup();
      const seen = [JSON.stringify(doc.items)];
      for (let k = 0; k < 15; k++) {
        const op = rnd(4);
        if (op === 0) {
          undo.beginGesture();
          for (let m = 0; m < 1 + rnd(4); m++) {
            doc.items[rnd(doc.items.length)] = rnd(50);
            undo.changed();
          }
          undo.endGesture();
        } else if (op === 1) {
          doc.items.push(rnd(50));
          undo.changed();
        } else if (op === 2 && doc.items.length > 1) {
          doc.items.splice(rnd(doc.items.length), 1);
          undo.changed();
        } else {
          doc.items[0] = rnd(50);
          undo.changed("typing");
          tick(rnd(3000));
        }
        const s = JSON.stringify(doc.items);
        if (s !== seen[seen.length - 1] && !(op === 3 && seen.length > 1 && undo.canUndo)) seen.push(s);
      }
      const end = JSON.stringify(doc.items);
      const back: string[] = [end];
      for (let guard = 0; undo.undo(); guard++) {
        expect(guard).toBeLessThan(100);
        back.push(JSON.stringify(doc.items));
      }
      expect(back[back.length - 1]).toBe(seen[0]);
      for (let i = back.length - 2; i >= 0; i--) {
        expect(undo.redo()).toBe(true);
        expect(JSON.stringify(doc.items)).toBe(back[i]);
      }
      expect(undo.redo()).toBe(false);
      for (let i = 1; i < back.length; i++) expect(back[i]).not.toBe(back[i - 1]);
    }
  });
});

describe("SharedSnapshot", () => {
  it("shares one copy until an element changes, and a restore doesn't look like a change", () => {
    const share = new SharedSnapshot<{ v: number }>();
    let live = [{ v: 1 }, { v: 2 }];
    const a = share.capture(live);
    expect(share.capture(live)).toBe(a);
    live[1] = { v: 3 };
    const b = share.capture(live);
    expect(b).not.toBe(a);
    expect(a[1].v).toBe(2);
    live = share.restore(a);
    expect(share.capture(live)).toBe(a);
    live[0] = { v: 9 };
    expect(a[0].v).toBe(1);
  });
});
