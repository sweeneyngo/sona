"""Every touching pair of output notes a rule could merge, with transcription-time features and its truth
label (chopped: both pieces under one UST note; boundary: the UST has a note boundary there)."""
import wob
L = wob.L


def candidates(s, g, tr=None, lab=None, kinds=None):
    """Rows for cuts i (between g[i-1] and g[i]) with 0 < |step| <= 2 semitones or a one-cell first piece."""
    tr = tr or wob.Track(s)
    if lab is None:
        lab, _ = wob.truth(s, g)
    if kinds is None:
        kinds = wob.chopped_cuts(s, g)
    rows = []
    for i in range(1, len(g)):
        a, b = g[i - 1], g[i]
        if a[1] != b[0]:
            continue
        iv = b[2] - a[2]
        la, lb = a[1] - a[0], b[1] - b[0]
        after_rest = i - 1 == 0 or g[i - 2][1] < a[0]
        before_rest = i + 1 >= len(g) or g[i + 1][0] > b[1]
        if iv == 0:
            continue
        ta0, ta1, tb1 = L.t_of(s, a[0]), L.t_of(s, a[1]), L.t_of(s, b[1])
        fa, fb = tr.frames(ta0, ta1), tr.frames(ta1, tb1)
        ma, mb = wob.median(fa), wob.median(fb)
        rows.append(dict(
            song=s["id"], i=i, iv=iv, la=la, lb=lb, after_rest=after_rest, before_rest=before_rest,
            label=lab.get(i, "other"), kind=kinds.get(i), pa=a[2], pb=b[2],
            ma=ma, mb=mb, va=len(fa) / max(1, (ta1 - ta0) / 0.0058), vb=len(fb) / max(1, (tb1 - ta1) / 0.0058),
            sep=None if ma is None or mb is None else (mb - ma) * (1 if iv > 0 else -1),
        ))
    return rows


if __name__ == "__main__":
    import collections
    songs = wob.load()
    for out in ("lead", "stage"):
        R = []
        for s in songs:
            R += candidates(s, wob.grid(s, out))
        print(f"== {out}: {len(R)} touching pitch-change pairs (0 < |step|)")

        def tab(name, rows):
            c = collections.Counter(r["label"] for r in rows)
            k = collections.Counter(r["kind"] for r in rows if r["label"] == "chopped")
            n = len(rows)
            print(f"  {name:44} n {n:5d}  chopped {c['chopped']:4d} ({100 * c['chopped'] / max(1, n):5.1f}%)  boundary {c['boundary']:4d}  other {c['other']:3d}   kinds {dict(k)}")

        step = [r for r in R if abs(r["iv"]) <= 2]
        tab("|step| 1-2", step)
        tab("|step| 3+", [r for r in R if abs(r["iv"]) > 2])
        for lmin in (1, 2, 3, 4):
            tab(f"|step| 1-2, shorter piece = {lmin}", [r for r in step if min(r["la"], r["lb"]) == lmin])
        tab("|step| 1-2, shorter piece >= 5", [r for r in step if min(r["la"], r["lb"]) >= 5])
        head = [r for r in R if r["la"] == 1 and r["after_rest"]]
        tab("1-cell first piece after a rest (any step)", head)
        tab("  ... |step| 1-2", [r for r in head if abs(r["iv"]) <= 2])
        tab("  ... |step| 3+", [r for r in head if abs(r["iv"]) > 2])
        tab("1-cell piece, not after a rest", [r for r in R if r["la"] == 1 and not r["after_rest"]])
        tab("1-cell last piece before a rest", [r for r in R if r["lb"] == 1 and r["before_rest"]])
        print("  sep (piece medians' distance along the step, semitones) by label, |step| 1-2:")
        for lab in ("chopped", "boundary"):
            v = sorted(r["sep"] for r in step if r["label"] == lab and r["sep"] is not None)
            q = [v[int(p * (len(v) - 1))] for p in (0.1, 0.25, 0.5, 0.75, 0.9)] if v else []
            print(f"    {lab:9} n {len(v):4d} (no f0 {sum(1 for r in step if r['label'] == lab and r['sep'] is None)})  q10/25/50/75/90 " + " ".join(f"{x:+.2f}" for x in q))
