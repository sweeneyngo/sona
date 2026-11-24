"""Today's web/lib/lyrics.ts semantics (っ and ー are their own tokens) on the hiragana text, ja-only lines,
vs the same split with the UST conventions (っ dropped, ー -> vowel). Pooled MER with song-bootstrap CI."""
import collections
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import evalread as ev  # noqa: E402
from readers import clean_text  # noqa: E402

SMALL = set('ぁぃぅぇぉゃゅょゎ')


def lyrics_ts(kana):
    out = []
    for ch in ev.kata2hira(kana):
        ch = ev.NORM.get(ch, ch)
        if not ('ぁ' <= ch <= 'ゖ' or ch == 'ー'):
            continue
        if ch in SMALL and out and out[-1] not in ('ー', 'っ', 'ん'):
            out[-1] += ch
        else:
            out.append(ch)
    return out


def main():
    ids = sys.argv[1:]
    per = {k: {} for k in ('lyrics.ts', 'ust-conventions')}
    for sid in ids:
        ref = ev.ust_moras(sid)
        text = ev.to_hiragana_text(clean_text(open(ev.LYR + sid + '.txt').read()))
        c = {k: collections.Counter() for k in per}
        for para in text.split('\n\n'):
            lines = [l for l in para.splitlines() if l.strip()]
            groups, cur = [], []
            for l in lines:
                cur.append(l)
                if len(lyrics_ts(''.join(cur))) >= 12:
                    groups.append(cur)
                    cur = []
            if cur:
                (groups[-1].extend(cur) if groups else groups.append(cur))
            for g in groups:
                s = ''.join(g)
                if any(ch.isascii() and ch.isalpha() for ch in s):
                    continue
                for k, hyp in (('lyrics.ts', lyrics_ts(s)), ('ust-conventions', [m for m, _ in ev.to_moras(s, {})])):
                    if not hyp:
                        continue
                    e, span, _, _ = ev.sellers(hyp, ref)
                    c[k]['e_ja'] += e
                    c[k]['n_ja'] += span
        for k in per:
            per[k][sid] = c[k]
    import summarize
    for k, p in per.items():
        E = sum(x['e_ja'] for x in p.values())
        N = sum(x['n_ja'] for x in p.values())
        lo, hi = summarize.boot(p, lambda x: x['e_ja'], lambda x: x['n_ja'])
        print(f'{k:16s} {E / N:.3f} [{lo:.3f},{hi:.3f}] E={E} N={N}')
    lo, hi = summarize.paired(per['lyrics.ts'], per['ust-conventions'])
    print(f'paired lyrics.ts - ust-conventions [{lo:+.3f},{hi:+.3f}]')


if __name__ == '__main__':
    main()
