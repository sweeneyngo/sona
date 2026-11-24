"""Can a dictionary-free rule find long-vowel tails in hiragana? A vowel kana whose vowel continues the mora
before (おう, えい, or the same vowel), or ー, is a tail. Scored on the hiragana text, ja-only lines:
'rule-merge' drops tails, 'rule-opt' makes them free either way. Same units and matcher as evalread."""
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import evalread as ev  # noqa: E402
import summarize  # noqa: E402
from readers import clean_text  # noqa: E402


def tails(ms):
    out = []
    for i, m in enumerate(ms):
        t = False
        if i > 0 and m in 'あいうえお':
            pv = ev.VOWELS.get(ms[i - 1][-1])
            t = pv == m or (pv, m) in (('お', 'う'), ('え', 'い'))
        out.append(t)
    return out


def main():
    ids = sys.argv[1:]
    per = {k: {} for k in ('split', 'rule-merge', 'rule-opt')}
    for sid in ids:
        ref = ev.ust_moras(sid)
        text = ev.to_hiragana_text(clean_text(open(ev.LYR + sid + '.txt').read()))
        c = {k: collections.Counter() for k in per}
        for para in text.split('\n\n'):
            lines = [l for l in para.splitlines() if l.strip()]
            groups, cur = [], []
            for l in lines:
                cur.append(l)
                if len(ev.to_moras(''.join(cur), {})) >= 12:
                    groups.append(cur)
                    cur = []
            if cur:
                (groups[-1].extend(cur) if groups else groups.append(cur))
            for g in groups:
                s = ''.join(g)
                if any(ch.isascii() and ch.isalpha() for ch in s):
                    continue
                ms = [m for m, _ in ev.to_moras(s, {})]
                if not ms:
                    continue
                tl = tails(ms)
                for k in per:
                    if k == 'split':
                        hyp, ic = ms, None
                    elif k == 'rule-merge':
                        hyp, ic = [m for m, t in zip(ms, tl) if not t], None
                    else:
                        hyp, ic = ms, [0 if t else 1 for t in tl]
                    e, span, _, _ = ev.sellers(hyp, ref, ic)
                    c[k]['e_ja'] += e
                    c[k]['n_ja'] += span
        for k in per:
            per[k][sid] = c[k]
    for k, p in per.items():
        E = sum(x['e_ja'] for x in p.values())
        N = sum(x['n_ja'] for x in p.values())
        lo, hi = summarize.boot(p, lambda x: x['e_ja'], lambda x: x['n_ja'])
        print(f'{k:12s} {E / N:.3f} [{lo:.3f},{hi:.3f}] E={E} N={N}')
    for k in ('rule-merge', 'rule-opt'):
        lo, hi = summarize.paired(per[k], per['split'])
        print(f'paired {k} - split [{lo:+.3f},{hi:+.3f}]')


if __name__ == '__main__':
    main()
