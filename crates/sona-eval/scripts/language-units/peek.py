"""Print the worst units and the commonest error ops for one detail file (scratchpad only)."""
import collections
import json
import sys

d = json.load(open(sys.argv[1]))
sid = sys.argv[2] if len(sys.argv) > 2 else None
kind = sys.argv[3] if len(sys.argv) > 3 else None
c = collections.Counter()
for s, units in d.items():
    if sid and s != sid:
        continue
    for u in units:
        for e in u['errs']:
            if kind is None or e[4] == kind:
                c[(e[0], e[1], e[2], e[3], e[4])] += 1
for k, v in c.most_common(int(sys.argv[4]) if len(sys.argv) > 4 else 40):
    print(v, k)
if sid:
    for u in sorted(d[sid], key=lambda u: -u['edits'])[:4]:
        print(u['edits'], u['text'].replace('\n', ' / '))
        print('   hyp', u['hyp'])
        print('   ref', u['ref'])
