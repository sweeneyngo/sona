"""Lyrics-first 01: the lyric placement metric and the fit with moras, in Python, for the oracles on a
`sona-eval-cellstage --dump` of dev. The Rust versions are `sona_eval::correction::lyric_placement` and
`sona_core::lyric_fit::fit_phrase_lyric`; `oracles.py --check` compares them with this file.

L01_DUMP: the dump directory (holds UST content); L01_WORK: lines/<id>.json from align_lines.py. Both stay
out of the repo.
"""
import bisect, json, math, os, random, sys

TAXO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "edit-cost-taxonomy")
sys.path.insert(0, TAXO)
import ec  # noqa: E402

DUMP = os.environ.get("L01_DUMP", "")
WORK = os.environ.get("L01_WORK", ".")

# ---- sona_core::lyric_fit, ported line by line, with the mora each note gets ----

TAIL_CELLS = 2
MAX_MORAS_PER_NOTE = 8
DURATION_WEIGHT = 0.5
NOTE_WITHOUT_MORA = 2.0
MORA_WITHOUT_NOTE = 3.0
ATTACH = {"c": -math.log(0.15), "o": -math.log(0.55)}


def hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def norm(lyric):
    l = lyric.strip()
    if " " in l:
        l = l.split()[-1]
    return hira(l)


def kind(lyric):
    l = norm(lyric)
    one = lambda st: len(l) == 1 and l in st
    return "o" if l in ("ー", "-", "～", "~", "ん", "n", "N") or one("あいうえおぁぃぅぇぉ") or one("aiueoAIUEO") else "c"


def mora_blocks(lens, kinds, rests=None, rest_cost=0.0):
    """(first mora, count) per note: the DP of `mora_counts`. With `rests` (cells of rest before each note,
    and one more after the last), a rest may take up to its cells in moras at `rest_cost` each: moras sung
    where the notes have a gap (07's 1b ceiling)."""
    if rests is not None:
        return _blocks_with_rests(lens, kinds, rests, rest_cost)
    n, m = len(lens), len(kinds)
    cpm = sum(lens) / max(m, 1)
    INF = float("inf")
    cost = [[INF] * (m + 1) for _ in range(n + 1)]
    step = [[None] * (m + 1) for _ in range(n + 1)]
    cost[0][0] = 0.0
    for j in range(1, m + 1):
        cost[0][j] = cost[0][j - 1] + MORA_WITHOUT_NOTE
        step[0][j] = "skip"
    for i in range(1, n + 1):
        ln = lens[i - 1]
        for j in range(m + 1):
            best = (cost[i - 1][j] + NOTE_WITHOUT_MORA, 0)
            if j > 0 and cost[i][j - 1] + MORA_WITHOUT_NOTE < best[0]:
                best = (cost[i][j - 1] + MORA_WITHOUT_NOTE, "skip")
            attached = 0.0
            for k in range(1, min(max(ln, 0), MAX_MORAS_PER_NOTE, j) + 1):
                if k > 1:
                    attached += ATTACH[kinds[j - k + 1]]
                duration = DURATION_WEIGHT * abs(math.log((ln / k) / cpm))
                c = cost[i - 1][j - k] + attached + duration
                if c < best[0]:
                    best = (c, k)
            cost[i][j], step[i][j] = best
    blocks = [(0, 0)] * n
    i, j = n, m
    while i > 0 or j > 0:
        s = step[i][j]
        if s == "skip":
            j -= 1
        else:
            blocks[i - 1] = (j - s, s)
            j -= s
            i -= 1
    return blocks


def _blocks_with_rests(lens, kinds, rests, rest_cost):
    items = []
    for i, ln in enumerate(lens):
        if rests[i] > 0:
            items.append((False, rests[i], None))
        items.append((True, ln, i))
    if rests[len(lens)] > 0:
        items.append((False, rests[len(lens)], None))
    n, m = len(items), len(kinds)
    cpm = sum(lens) / max(m, 1)
    INF = float("inf")
    cost = [[INF] * (m + 1) for _ in range(n + 1)]
    step = [[None] * (m + 1) for _ in range(n + 1)]
    cost[0][0] = 0.0
    for j in range(1, m + 1):
        cost[0][j] = cost[0][j - 1] + MORA_WITHOUT_NOTE
        step[0][j] = "skip"
    for i in range(1, n + 1):
        is_note, ln, _ = items[i - 1]
        for j in range(m + 1):
            best = (cost[i - 1][j] + (NOTE_WITHOUT_MORA if is_note else 0.0), 0)
            if j > 0 and cost[i][j - 1] + MORA_WITHOUT_NOTE < best[0]:
                best = (cost[i][j - 1] + MORA_WITHOUT_NOTE, "skip")
            attached = 0.0
            for k in range(1, min(max(ln, 0), MAX_MORAS_PER_NOTE, j) + 1):
                if is_note:
                    if k > 1:
                        attached += ATTACH[kinds[j - k + 1]]
                    c = cost[i - 1][j - k] + attached + DURATION_WEIGHT * abs(math.log((ln / k) / cpm))
                else:
                    c = cost[i - 1][j - k] + rest_cost * k
                if c < best[0]:
                    best = (c, k)
            cost[i][j], step[i][j] = best
    blocks = [(0, 0)] * len(lens)
    i, j = n, m
    while i > 0 or j > 0:
        s_ = step[i][j]
        if s_ == "skip":
            j -= 1
        else:
            if items[i - 1][0]:
                blocks[items[i - 1][2]] = (j - s_, s_)
            j -= s_
            i -= 1
    return blocks


def cuts(note, k):
    a, b = note[0], note[1]
    ln = b - a
    if ln < (k - 1) * TAIL_CELLS + 1:
        k = max(min(k, ln), 1)
        return [a + math.floor(ln * j / k + 0.5) for j in range(1, k)]
    return [b - (k - j) * TAIL_CELLS for j in range(1, k)]


def fit_phrase_lyric(notes, lyrics, span=None, rest_cost=0.0):
    """notes: [(start, end, pitch)] cells; lyrics: the phrase's moras. Returns [(start, end, pitch, mora or None)].
    With `span` (start, end cells of the phrase), rests inside it can hold moras (see mora_blocks)."""
    if not lyrics:
        return [(a, b, p, None) for a, b, p in notes]
    rests = None
    if span is not None:
        ends = [span[0]] + [b for _, b, _ in notes]
        rests = [max(0, a - e) for (a, _, _), e in zip(notes, ends)] + [max(0, span[1] - max(ends))]
    blocks = mora_blocks([b - a for a, b, _ in notes], [kind(l) for l in lyrics], rests, rest_cost)
    out = []
    for i, (note, (first, k)) in enumerate(zip(notes, blocks)):
        continues = i > 0 and notes[i - 1][1] == note[0] and notes[i - 1][2] == note[2]
        if out and k == 0 and continues and out[-1][1] == note[0]:
            out[-1] = (out[-1][0], note[1], out[-1][2], out[-1][3])
        elif k > 1:
            start, pieces = note[0], []
            for c in [c for c in cuts(note, k) if note[0] < c < note[1]] + [note[1]]:
                if c > start:
                    pieces.append((start, c))
                    start = c
            for q, (x, y) in enumerate(pieces):
                out.append((x, y, note[2], lyrics[first + q]))
        else:
            out.append((note[0], note[1], note[2], lyrics[first] if k == 1 else None))
    return out


# ---- sona_eval::correction::lyric_placement ----

def to_cells_l(notes, spacing, anchor):
    """ec.to_cells keeping each note's lyric: [start, end, pitch, lyric]."""
    v = []
    for n in notes:
        s, e = ec.rround((n[0] - anchor) / spacing), ec.rround((n[1] - anchor) / spacing)
        if e > s:
            v.append([s, e, ec.rround(n[2]), n[3]])
    v.sort(key=lambda c: (c[0], c[1]))
    for i in range(1, len(v)):
        if v[i - 1][1] > v[i][0]:
            v[i - 1][1] = v[i][0]
    return [c for c in v if c[1] > c[0]]


def placement(ref, out, spacing, anchor):
    """ref: [(on, off, midi, lyric)]; out: [(on, off, midi, lyric or None)]. Returns dict(notes, placed,
    phrases, landed, span_s)."""
    refs = to_cells_l(ref, spacing, anchor)
    if not refs:
        return dict(notes=0, placed=0, phrases=0, landed=0, span_s=0.0)
    s0, s1 = refs[0][0], refs[-1][1]
    outs = [o for o in to_cells_l(out, spacing, anchor) if o[1] > s0 and o[0] < s1]
    best_o = [None] * len(refs)
    best_r = [None] * len(outs)
    for ri, r in enumerate(refs):
        lo = bisect.bisect_right([o[1] for o in outs], r[0])
        for oi in range(lo, len(outs)):
            o = outs[oi]
            if o[0] >= r[1]:
                break
            sh = ec.shared(r, o)
            if sh <= 0:
                continue
            if best_o[ri] is None or sh > best_o[ri][0]:
                best_o[ri] = (sh, oi)
            if best_r[oi] is None or sh > best_r[oi][0]:
                best_r[oi] = (sh, ri)
    placed_r = [False] * len(refs)
    matched_o = [False] * len(outs)
    for ri, b in enumerate(best_o):
        if b is not None and best_r[b[1]][1] == ri:
            matched_o[b[1]] = True
            o = outs[b[1]]
            placed_r[ri] = o[3] is not None and norm(o[3]) == norm(refs[ri][3])
    phrases = []
    for ri, r in enumerate(refs):
        if phrases and refs[phrases[-1][-1]][1] == r[0]:
            phrases[-1].append(ri)
        else:
            phrases.append([ri])
    mids = sorted((o[0] + o[1], oi) for oi, o in enumerate(outs))
    keys = [m for m, _ in mids]
    landed = 0
    for ph in phrases:
        a, b = refs[ph[0]][0], refs[ph[-1]][1]
        stray = any(outs[oi][3] is not None and not matched_o[oi] for _, oi in mids[bisect.bisect_left(keys, 2 * a):bisect.bisect_left(keys, 2 * b)])
        landed += all(placed_r[ri] for ri in ph) and not stray
    return dict(notes=len(refs), placed=sum(placed_r), phrases=len(phrases), landed=landed, span_s=(s1 - s0) * spacing)


def pool(rows):
    p = dict(notes=0, placed=0, phrases=0, landed=0, span_s=0.0)
    for r in rows:
        for k in p:
            p[k] += r[k]
    return p


def wrong_per_min(p):
    return (p["notes"] - p["placed"]) / (p["span_s"] / 60)


def placed_pct(p):
    return 100 * p["placed"] / p["notes"]


def landed_pct(p):
    return 100 * p["landed"] / p["phrases"]


def boot(rows, stat, iters=10000, seed=1):
    rng = random.Random(seed)
    v = sorted(stat(pool([rows[rng.randrange(len(rows))] for _ in rows])) for _ in range(iters))
    return v[int(0.025 * (iters - 1) + 0.5)], v[int(0.975 * (iters - 1) + 0.5)]


def paired(rows_a, rows_b, stat, pool_fn=pool, iters=10000, seed=1):
    rng = random.Random(seed)
    n = len(rows_a)
    d = []
    for _ in range(iters):
        idx = [rng.randrange(n) for _ in range(n)]
        d.append(stat(pool_fn([rows_b[i] for i in idx])) - stat(pool_fn([rows_a[i] for i in idx])))
    d.sort()
    return stat(pool_fn(rows_b)) - stat(pool_fn(rows_a)), d[int(0.025 * (iters - 1) + 0.5)], d[int(0.975 * (iters - 1) + 0.5)]


# ---- songs ----

def load(ids=None):
    songs = []
    for f in sorted(os.listdir(DUMP)):
        if f.endswith(".json") and (ids is None or f[:-5] in ids):
            s = json.load(open(os.path.join(DUMP, f)))
            u = sorted((x for x in s["stage_detail"]["ust"] if x["kind"] == "Note"), key=lambda x: (x["on"], x["off"]))
            s["ust_l"] = [(x["on"], x["off"], x["midi"], x["lyric"]) for x in u]
            lp = os.path.join(WORK, "lines", s["id"] + ".json")
            s["lines"] = json.load(open(lp)) if os.path.exists(lp) else None
            songs.append(s)
    return songs
