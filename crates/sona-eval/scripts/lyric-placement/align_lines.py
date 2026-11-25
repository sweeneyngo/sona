"""Place a lyric text's line (and space) breaks on the lead UST's mora stream (lyrics-first 01).

The text's reading (fugashi + unidic-lite for kanji, kana kept, Latin words as one wildcard mora per vowel
group) is aligned to the UST's moras by edit distance; each line's first mora maps to the UST note it aligns
with. Writes $L01_WORK/lines/<id>.json (UST note indices of line and space starts, alignment quality).
Lyric-derived: the output stays in the scratchpad. Run with a Python that has fugashi:
    L01_DUMP=… L01_WORK=… python3 -I align_lines.py <ids>
"""
import json, os, re, sys, unicodedata
import fugashi, unidic_lite

LY = os.environ.get("SONA_LYRICS_DIR") or os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "data", "pifuyuu-lyrics")
DUMP, WORK = os.environ["L01_DUMP"], os.environ["L01_WORK"]
SMALL = set("ぁぃぅぇぉゃゅょゎゕゖ")
tag = fugashi.Tagger("-r /dev/null -d " + unidic_lite.DICDIR)
SAME = [set("はわ"), set("へえ"), set("をお"), set("づず"), set("ぢじ")]
PURE = set("あいうえお")


def hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


def is_kana(c):
    return "ぁ" <= c <= "ゖ" or "ァ" <= c <= "ヺ" or c == "ー"


def kana_moras(text):
    out = []
    for c in hira(text):
        if not is_kana(c):
            continue
        if c in SMALL and out and out[-1] not in ("ー", "っ", "ん"):
            out[-1] += c
        else:
            out.append(c)
    return out


def chunk_moras(chunk):
    out = []
    for w in tag(chunk):
        surf = w.surface
        if surf and all(is_kana(c) for c in surf):
            out += kana_moras(surf)
        elif any("一" <= c <= "鿿" or c == "々" for c in surf):
            k = w.feature.kana if w.feature.kana and w.feature.kana != "*" else ""
            out += kana_moras(k)
        elif re.search("[A-Za-z]", surf):
            for word in re.findall("[A-Za-z']+", surf):
                out += ["?"] * max(1, len(re.findall("[aeiouy]+", word.lower())))
    return out


def text_stream(sid):
    p = os.path.join(LY, sid + ".readings.txt")
    if not os.path.exists(p):
        p = os.path.join(LY, sid + ".txt")
    moras, line_start, space_start = [], [], []
    for line in open(p, encoding="utf-8").read().splitlines():
        line = unicodedata.normalize("NFKC", line)
        first_in_line, prev_latin = True, True
        for chunk in re.split(r"\s+", line.strip()):
            ms = chunk_moras(chunk)
            latin = bool(ms) and all(m == "?" for m in ms)
            for j, m in enumerate(ms):
                moras.append(m)
                line_start.append(first_in_line and j == 0)
                space_start.append(j == 0 and (first_in_line or not (latin or prev_latin)))
            if ms:
                first_in_line, prev_latin = False, latin
    return moras, line_start, space_start, os.path.basename(p)


def sub_cost(a, b):
    if a == b:
        return 0.0
    if a == "?" or b == "?":
        return 0.5
    if any(a in s and b in s for s in SAME):
        return 0.0
    if (a == "ー" and b in PURE) or (b == "ー" and a in PURE):
        return 0.0
    return 1.0


def align(t, u):
    """Edit-distance alignment; for each text mora the UST index it lands on (match or substitution) or None."""
    n, m = len(t), len(u)
    D = [[0.0] * (m + 1) for _ in range(n + 1)]
    B = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n + 1):
        D[i][0], B[i][0] = float(i), 1
    for j in range(m + 1):
        D[0][j], B[0][j] = float(j), 2
    for i in range(1, n + 1):
        Di, Dp, ti = D[i], D[i - 1], t[i - 1]
        for j in range(1, m + 1):
            best, arg = Dp[j - 1] + sub_cost(ti, u[j - 1]), 0
            if Dp[j] + 1 < best:
                best, arg = Dp[j] + 1, 1
            if Di[j - 1] + 1 < best:
                best, arg = Di[j - 1] + 1, 2
            Di[j], B[i][j] = best, arg
    at = [None] * n
    i, j, matches = n, m, 0
    while i > 0 or j > 0:
        a = B[i][j] if i > 0 and j > 0 else (1 if j == 0 else 2)
        if a == 0:
            at[i - 1] = j - 1
            matches += sub_cost(t[i - 1], u[j - 1]) == 0
            i, j = i - 1, j - 1
        elif a == 1:
            i -= 1
        else:
            j -= 1
    return at, D[n][m], matches


def realign_repeats(t, u, at, min_run=8):
    """A UST run of at least `min_run` moras that no text mora reached is a repeat the text writes once: align
    it alone (all of it, against any stretch of the text) and give it that stretch's breaks. Returns
    {UST index: text index} for the runs."""
    cov = {a for a in at if a is not None}
    runs, j = [], 0
    while j < len(u):
        if j in cov:
            j += 1
            continue
        k = j
        while k < len(u) and k not in cov:
            k += 1
        if k - j >= min_run:
            runs.append((j, k))
        j = k
    extra = {}
    for a, b in runs:
        r = u[a:b]
        n, m = len(r), len(t)
        D = [[0.0] * (m + 1) for _ in range(n + 1)]
        B = [[2] * (m + 1) for _ in range(n + 1)]
        for i in range(1, n + 1):
            D[i][0], B[i][0] = float(i), 1
            for jj in range(1, m + 1):
                best, arg = D[i - 1][jj - 1] + sub_cost(t[jj - 1], r[i - 1]), 0
                if D[i - 1][jj] + 1 < best:
                    best, arg = D[i - 1][jj] + 1, 1
                if D[i][jj - 1] + 1 < best:
                    best, arg = D[i][jj - 1] + 1, 2
                D[i][jj], B[i][jj] = best, arg
        jj = min(range(m + 1), key=lambda x: D[n][x])
        i = n
        while i > 0 and jj > 0:
            if B[i][jj] == 0:
                extra[a + i - 1] = jj - 1
                i, jj = i - 1, jj - 1
            elif B[i][jj] == 1:
                i -= 1
            else:
                jj -= 1
    return extra


def starts(at, flags, m, extra=None):
    """UST indices where a flagged text mora lands (a deleted text mora takes the next aligned one's)."""
    out = set()
    for i, f in enumerate(flags):
        if not f:
            continue
        k = i
        while k < len(at) and at[k] is None:
            k += 1
        if k < len(at):
            out.add(at[k])
    for ui, ti in (extra or {}).items():
        if flags[ti]:
            out.add(ui)
    return sorted(x for x in out if 0 < x < m)


def ust_moras(sid):
    s = json.load(open(os.path.join(DUMP, sid + ".json")))
    u = sorted((x for x in s["stage_detail"]["ust"] if x["kind"] == "Note"), key=lambda x: (x["on"], x["off"]))
    return [hira(x["lyric"].split()[-1]) for x in u]


if __name__ == "__main__":
    os.makedirs(os.path.join(WORK, "lines"), exist_ok=True)
    for sid in sys.argv[1:]:
        u = ust_moras(sid)
        t, ls, ss, src = text_stream(sid)
        at, dist, matches = align(t, u)
        covered = len({a for a in at if a is not None})
        extra = realign_repeats(t, u, at)
        out = dict(src=src, ust_moras=len(u), text_moras=len(t), matches=matches, dist=dist, ust_covered=covered,
                   repeats_realigned=len(extra),
                   line_starts=starts(at, ls, len(u), extra), space_starts=starts(at, ss, len(u), extra))
        json.dump(out, open(os.path.join(WORK, "lines", sid + ".json"), "w"))
        print(f"{sid:28} {src:30} UST {len(u):4} text {len(t):4} exact {matches:4} ({100 * matches / len(u):5.1f}% of UST) "
              f"covered {100 * covered / len(u):5.1f}% +rep {len(extra):3} dist {dist:6.1f} lines {len(out['line_starts']) + 1:3} "
              f"chunks {len(out['space_starts']) + 1:3}")
