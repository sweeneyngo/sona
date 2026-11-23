/**
 * Undo/redo by snapshots (ADR-0025). The editor reports every change; the controller compares the
 * document with the last state it recorded and keeps the difference as one step. A gesture (a drag, a
 * pen stroke) reports many changes but becomes one step when it ends, and repeated changes from one
 * source within a short window (typing a tempo) merge into one step.
 */

export interface UndoOptions<S> {
  capture: () => S;
  restore: (s: S) => void;
  equal: (a: S, b: S) => boolean;
  limit?: number;
  mergeWindowMs?: number;
  now?: () => number;
}

interface Step<S> {
  before: S;
  after: S;
  key?: string;
  at: number;
}

export class Undo<S> {
  private undoStack: Step<S>[] = [];
  private redoStack: Step<S>[] = [];
  private committed: S;
  private inGesture = false;
  private readonly o: Required<UndoOptions<S>>;

  constructor(opts: UndoOptions<S>) {
    this.o = { limit: 200, mergeWindowMs: 1500, now: () => Date.now(), ...opts };
    this.committed = this.o.capture();
  }

  /** Forget all history: the current document becomes the starting point (a new song or session). */
  reset(): void {
    this.undoStack = [];
    this.redoStack = [];
    this.inGesture = false;
    this.committed = this.o.capture();
  }

  /** Idempotent, so a lost mouseup can't leave the history stuck mid-gesture. */
  beginGesture(): void {
    this.inGesture = true;
  }

  endGesture(): void {
    if (!this.inGesture) return;
    this.inGesture = false;
    this.changed();
  }

  /** Something may have changed; during a gesture this waits for the gesture to end. */
  changed(key?: string): void {
    if (this.inGesture) return;
    const now = this.o.capture();
    if (this.o.equal(now, this.committed)) return;
    const t = this.o.now();
    const last = this.undoStack[this.undoStack.length - 1];
    if (key !== undefined && last && last.key === key && this.redoStack.length === 0 && t - last.at <= this.o.mergeWindowMs) {
      last.after = now;
      last.at = t;
      if (this.o.equal(last.before, last.after)) this.undoStack.pop();
    } else {
      this.undoStack.push({ before: this.committed, after: now, key, at: t });
      if (this.undoStack.length > this.o.limit) this.undoStack.shift();
    }
    this.redoStack = [];
    this.committed = now;
  }

  get canUndo(): boolean {
    return this.undoStack.length > 0 && !this.inGesture;
  }

  get canRedo(): boolean {
    return this.redoStack.length > 0 && !this.inGesture;
  }

  undo(): boolean {
    if (!this.canUndo) return false;
    this.changed();
    const step = this.undoStack.pop()!;
    this.redoStack.push(step);
    this.committed = step.before;
    this.o.restore(step.before);
    return true;
  }

  redo(): boolean {
    if (!this.canRedo) return false;
    const step = this.redoStack.pop()!;
    this.undoStack.push(step);
    this.committed = step.after;
    this.o.restore(step.after);
    return true;
  }
}

/**
 * Snapshots of a large array edited in place (the pitch line): one copy is shared by every step until
 * an element changes, so history doesn't hold a copy per edit, and identity stands for equality.
 */
export class SharedSnapshot<T> {
  private last: T[] | null = null;

  /** The snapshot for `current`: the previous one when no element changed, else a new copy. */
  capture(current: T[]): T[] {
    const last = this.last;
    if (!last || last.length !== current.length || last.some((x, i) => x !== current[i])) this.last = current.slice();
    return this.last!;
  }

  /** A working copy of `snapshot` to edit, remembering it so the next capture returns `snapshot` itself. */
  restore(snapshot: T[]): T[] {
    this.last = snapshot;
    return snapshot.slice();
  }
}
