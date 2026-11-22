/**
 * Shift-JIS encoding for UST export. Classic UTAU reads USTs as Shift-JIS
 * (CP932). Browsers ship a Shift-JIS *decoder* (`TextDecoder("shift_jis")`)
 * but no encoder, so the table is built once by decoding every one- and
 * two-byte code with the platform's own decoder. That keeps the mapping
 * identical to what a Shift-JIS reader expects, and no table ships.
 */

let table: Map<string, number[]> | null = null;

function buildTable(): Map<string, number[]> {
  const dec = new TextDecoder("shift_jis", { fatal: true });
  const map = new Map<string, number[]>();
  const add = (bytes: number[]) => {
    let ch: string;
    try {
      ch = dec.decode(Uint8Array.from(bytes));
    } catch {
      return;
    }
    // First code wins: NEC/IBM extension duplicates map to the same character.
    if ([...ch].length === 1 && !map.has(ch)) map.set(ch, bytes);
  };
  for (let b = 0x00; b <= 0x7f; b++) add([b]);
  for (let b = 0xa1; b <= 0xdf; b++) add([b]);
  for (let lead = 0x81; lead <= 0xfc; lead++) {
    if (lead > 0x9f && lead < 0xe0) continue;
    for (let trail = 0x40; trail <= 0xfc; trail++) {
      if (trail !== 0x7f) add([lead, trail]);
    }
  }
  return map;
}

/** Encode `text` as Shift-JIS, or return null if any character has no Shift-JIS form. */
export function encodeShiftJis(text: string): Uint8Array | null {
  table ??= buildTable();
  const out: number[] = [];
  for (const ch of text) {
    const bytes = table.get(ch);
    if (!bytes) return null;
    out.push(...bytes);
  }
  return Uint8Array.from(out);
}
