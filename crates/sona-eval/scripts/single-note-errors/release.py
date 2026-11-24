"""Releases: GAME lead notes that run through a rest the UST leaves before the next note.

For every pair whose output ends later than the UST note, report the UST's rest length, how far the output
runs, and what the vocals' pitch track and energy do over the overrun (evidence available at transcription
time). Compares against pairs that end on time.

usage: S12_DUMP=DIR python3 -I release.py [lead|stage]
"""
import bisect, collections, os, statistics as st, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib12 as L

OUT = sys.argv[1] if len(sys.argv) > 1 else "lead"
songs = L.load()


def track(song):
    t = [x[0] for x in song["f0"]]
    hz = [x[1] for x in song["f0"]]
    rt = [x[0] for x in song["rms"]]
    rv = [x[1] for x in song["rms"]]
    return t, hz, rt, rv


def span_stats(tr, a, b):
    t, hz, rt, rv = tr
    i, j = bisect.bisect_left(t, a), bisect.bisect_left(t, b)
    voiced = sum(1 for h in hz[i:j] if h > 0) / max(1, j - i)
    k, l = bisect.bisect_left(rt, a), bisect.bisect_left(rt, b)
    e = rv[k:l]
    return voiced, (st.mean(e) if e else 0.0)


rows = []
for s in songs:
    A = L.analyse(s, s["outputs"][OUT])
    sp, an = s["spacing"], s["ust_anchor"]
    tr = track(s)
    for p in L.pairs(A):
        r, o = p["r"], p["o"]
        sec = lambda c: an + c * sp
        # body: the note's last two cells before the UST end; tail: the overrun (or the cell after the end)
        v_body, e_body = span_stats(tr, sec(max(r[0], r[1] - 2)), sec(r[1]))
        v_tail, e_tail = span_stats(tr, sec(r[1]), sec(max(o[1], r[1] + 1)))
        rows.append(dict(song=s["id"], de=p["de"], ds=p["ds"], rgap=p["r_next_gap"], ogap=p["o_next_gap"], len=r[1] - r[0],
                         v_body=v_body, v_tail=v_tail, e_ratio=(e_tail / e_body if e_body > 0 else 0.0), cost=p["cost"]))

print(f"{OUT}: pairs {len(rows)}")
late = [r for r in rows if r["ds"] == 0 and r["de"] > 0 and r["rgap"] and r["rgap"] > 0]
print(f"end late into a UST rest (ds 0): {len(late)}; overrun == whole rest: {sum(r['de'] == r['rgap'] for r in late)}; "
      f"overrun < rest: {sum(r['de'] < r['rgap'] for r in late)}")
print(" UST rest length (cells) of those:", sorted(collections.Counter(r["rgap"] for r in late).items())[:12])
print(" overrun (cells):", sorted(collections.Counter(r["de"] for r in late).items())[:12])
ontime_rest = [r for r in rows if r["ds"] == 0 and r["de"] == 0 and r["rgap"] and r["rgap"] > 0]
print(f"on-time ends before a UST rest: {len(ontime_rest)}")


def summ(label, rs):
    if not rs:
        return
    print(f"  {label:44} n {len(rs):4d}  voiced over the tail {st.mean(r['v_tail'] for r in rs):.2f} "
          f"(body {st.mean(r['v_body'] for r in rs):.2f})  energy tail/body median {st.median(r['e_ratio'] for r in rs):.2f}")


print("\nevidence over the cells after the UST note's end")
summ("late into a UST rest", late)
summ("on time before a UST rest (cell after end)", ontime_rest)
summ("late, UST legato (cell after end)", [r for r in rows if r["ds"] == 0 and r["de"] > 0 and r["rgap"] == 0])
summ("early (de < 0), UST rest", [r for r in rows if r["ds"] == 0 and r["de"] < 0 and r["rgap"] and r["rgap"] > 0])

print("\nper song: late-into-rest count / pairs ending before a UST rest")
for s in songs:
    a = [r for r in late if r["song"] == s["id"]]
    b = [r for r in rows if r["song"] == s["id"] and r["rgap"] and r["rgap"] > 0]
    print(f"  {s['id']:28} {len(a):3d} / {len(b):3d}  overrun==rest {sum(r['de'] == r['rgap'] for r in a)}")
