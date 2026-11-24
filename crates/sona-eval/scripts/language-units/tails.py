"""How often does the UST give a long-vowel tail (pron ー) its own note? And how many っ are in texts vs USTs.
pyopenjtalk-plus pron, ja-only units, long-vowel tails optional in the match."""
import collections
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import evalread as ev  # noqa: E402
from readers import PyOpenJTalk, clean_text  # noqa: E402

IDS = sys.argv[1:]
pj = PyOpenJTalk()
tot = collections.Counter()
for sid in IDS:
    ref = ev.ust_moras(sid)
    text = clean_text(open(ev.LYR + sid + '.txt').read())
    c = collections.Counter()
    c['sokuon_text'] = text.count('っ') + text.count('ッ')
    c['sokuon_ust'] = sum(1 for n in json.load(open(ev.DUMP + sid + '.json'))['stage_detail']['ust'] if 'っ' in n['lyric'])
    for ms, latin, _ in ev.units_of(text, pj, 'pron', 12):
        if latin or not ms:
            continue
        hyp = [m for m, _ in ms]
        icost = [0 if t.get('choon') else 1 for _, t in ms]
        e, span, ops, j0 = ev.sellers(hyp, ref, icost)
        for op, hi, rj in ops:
            if hi is not None and ms[hi][1].get('choon'):
                c['tail_' + {'=': 'own_note', 'i': 'merged', 'S': 'sub', 'I': 'ins'}[op]] += 1
    print(sid, dict(c))
    tot.update(c)
print('pooled', dict(tot))
t = tot['tail_own_note'] + tot['tail_merged']
print(f"long-vowel tails with their own UST note: {tot['tail_own_note']}/{t} = {tot['tail_own_note'] / max(t, 1):.3f}")
