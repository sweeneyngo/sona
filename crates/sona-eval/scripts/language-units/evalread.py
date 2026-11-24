"""Score Japanese readers: kanji-mixed lyric text -> hiragana moras vs the lead UST's moras (dev only).

Each unit (a group of lines with >= MIN_MORAS moras, or a paragraph) is matched to its best span anywhere
in the UST's mora sequence (approximate substring match), so unexpanded repeats cost nothing; the edits
on that match are the errors. Per-song outputs stay in the scratchpad.
"""
import argparse
import collections
import json
import os
import random
import re
import unicodedata
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from readers import all_readers, clean_text, is_kana  # noqa: E402

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..'))
LYR = ROOT + '/data/pifuyuu-lyrics/'
# 03's dev dumps (UST notes with lyrics); per-song outputs go outside the repo.
DUMP = os.environ.get('SONA_LANG_DUMP', '') + '/'
OUT = os.environ.get('SONA_LANG_OUT', os.path.expanduser('~/.cache/sona/lang-units/out')) + '/'
SMALL = set('ぁぃぅぇぉゃゅょゎゕゖ')
VOWELS = {}
for row in ['あかさたなはまやらわがざだばぱぁゃゎ', 'いきしちにひみりぎじぢびぴぃ', 'うくすつぬふむゆるぐずづぶぷぅゅゔ',
            'えけせてねへめれげぜでべぺぇ', 'おこそとのほもよろをごぞどぼぽぉょ']:
    for ch in row:
        VOWELS[ch] = 'あいうえお'[['あ', 'い', 'う', 'え', 'お'].index(row[0])] if row[0] in 'あいうえお' else None
for v, row in zip('あいうえお', ['あかさたなはまやらわがざだばぱぁゃゎ', 'いきしちにひみりぎじぢびぴぃ', 'うくすつぬふむゆるぐずづぶぷぅゅゔ',
                                'えけせてねへめれげぜでべぺぇ', 'おこそとのほもよろをごぞどぼぽぉょ']):
    for ch in row:
        VOWELS[ch] = v
NORM = {'を': 'お', 'ぢ': 'じ', 'づ': 'ず'}


def kata2hira(s):
    return ''.join(chr(ord(c) - 0x60) if 0x30a1 <= ord(c) <= 0x30f6 else c for c in s)


def to_moras(kana, tag, choon='vowel'):
    """Kana -> UST-style moras: small kana join the mora before, っ is not a note, ー repeats the vowel."""
    out = []
    for ch in kata2hira(kana):
        ch = NORM.get(ch, ch)
        if ch == 'っ':
            continue
        if ch == 'ー':
            if out and choon != 'drop':
                v = VOWELS.get(out[-1][0][-1])
                if v:
                    out.append((v, dict(tag, choon=True)))
            continue
        if not ('\u3041' <= ch <= '\u3096' or '\u30a1' <= ch <= '\u30fa'):
            continue
        if ch in SMALL and out and out[-1][0] not in ('ん',):
            out[-1] = (out[-1][0] + ch, out[-1][1])
        else:
            out.append((ch, dict(tag)))
    return out


def hyp_moras(words, mode):
    ms = []
    for wi, (s, read, pron, part, cls) in enumerate(words):
        if mode.startswith('pron'):
            k = pron
        else:
            k = read
            if mode == 'read' and part:
                k = part
        ms += to_moras(k, {'w': wi, 'surface': s, 'cls': cls, 'particle': part is not None},
                       choon='drop' if mode == 'pron-merge' else 'vowel')
    return ms


def sellers(hyp, ref, icost=None):
    """Best approximate match of hyp inside ref. icost[i]: cost of leaving hyp[i] unmatched (default 1)."""
    n, m = len(hyp), len(ref)
    icost = icost or [1] * n
    D = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(1, n + 1):
        D[i][0] = D[i - 1][0] + icost[i - 1]
        hi = hyp[i - 1]
        ic = icost[i - 1]
        Di, Dp = D[i], D[i - 1]
        for j in range(1, m + 1):
            a = Dp[j - 1] + (0 if hi == ref[j - 1] else 1)
            b = Dp[j] + ic
            c = Di[j - 1] + 1
            Di[j] = a if a <= b and a <= c else (b if b <= c else c)
    jbest = min(range(m + 1), key=lambda j: (D[n][j], j))
    edits = D[n][jbest]
    ops = []
    i, j = n, jbest
    while i > 0:
        if j > 0 and D[i][j] == D[i - 1][j - 1] + (0 if hyp[i - 1] == ref[j - 1] else 1):
            ops.append(('=' if hyp[i - 1] == ref[j - 1] else 'S', i - 1, j - 1))
            i, j = i - 1, j - 1
        elif D[i][j] == D[i - 1][j] + icost[i - 1]:
            ops.append(('I' if icost[i - 1] else 'i', i - 1, None))
            i -= 1
        else:
            ops.append(('D', None, j - 1))
            j -= 1
    ops.reverse()
    return edits, jbest - j, ops, j


def kind_of(op, hm, rm, prev_h, words_tag):
    """Error kind for an edit, from the hyp mora's word."""
    t = words_tag
    h = hm[0] if hm else None
    r = rm
    if t.get('particle') and t['surface'] in ('は', 'へ') and op == 'S' and {h, r} in ({'は', 'わ'}, {'へ', 'え'}):
        return 'particle'
    vow = set('あいうえお')
    if op == 'S' and h in vow and r in vow:
        return 'long vowel: other vowel'
    if op in ('I', 'D'):
        x = h if op == 'I' else r
        if x in vow and prev_h is not None:
            pv = VOWELS.get(prev_h[-1])
            if pv == x or (pv, x) in (('お', 'う'), ('え', 'い')):
                return 'long vowel: reader extra' if op == 'I' else 'long vowel: UST extra'
    if t.get('choon') and op != 'D':
        return 'long vowel: reader extra'
    return {'kanji': 'kanji reading', 'latin': 'English/latin', 'number': 'number', 'kana': 'kana (text vs sung)'}.get(
        t['cls'], 'other')


def units_of(text, reader, mode, min_moras):
    """Line groups (>= min_moras) or paragraphs (min_moras=None) -> list of (hyp_moras, has_latin, text)."""
    units = []
    for para in re.split(r'\n\s*\n', text):
        lines = [l for l in para.splitlines() if l.strip()]
        if not lines:
            continue
        if min_moras is None:
            groups = [lines]
        else:
            groups, cur, cnt = [], [], 0
            for l in lines:
                cur.append(l)
                cnt += len(hyp_moras(reader.words(l), mode))
                if cnt >= min_moras:
                    groups.append(cur)
                    cur, cnt = [], 0
            if cur:
                if groups:
                    groups[-1] += cur
                else:
                    groups.append(cur)
        for g in groups:
            ms = []
            for l in g:
                ms += hyp_moras(reader.words(l), mode)
            latin = any(c.isascii() and c.isalpha() for c in ''.join(g))
            units.append((ms, latin, '\n'.join(g)))
    return units


_HIRA = {}


def to_hiragana_text(text):
    """What a user would paste under the hiragana rule: pyopenjtalk-plus's orthographic reading, line by line,
    particles as written (は, へ, を), long vowels as spelled, Latin words left as they are."""
    if text in _HIRA:
        return _HIRA[text]
    from readers import PyOpenJTalk
    pj = PyOpenJTalk()
    lines = []
    for l in text.split('\n'):
        parts = []
        for s, read, pron, part, cls in pj.words(l) if l.strip() else []:
            parts.append(unicodedata.normalize('NFKC', s) if cls == 'latin' else kata2hira(read))
        lines.append(''.join(parts))
    _HIRA[text] = '\n'.join(lines)
    return _HIRA[text]


def ust_moras(sid):
    d = json.load(open(DUMP + sid + '.json'))
    notes = sorted(d['stage_detail']['ust'], key=lambda n: n['on'])
    out = []
    for n in notes:
        out += [m for m, _ in to_moras(n['lyric'], {})]
    return out


def boot(per_song, key_e, key_n, B=10000, seed=0):
    rnd = random.Random(seed)
    ids = list(per_song)
    vals = []
    for _ in range(B):
        s = [rnd.choice(ids) for _ in ids]
        e = sum(per_song[i].get(key_e, 0) for i in s)
        n = sum(per_song[i].get(key_n, 0) for i in s)
        vals.append(e / max(n, 1))
    vals.sort()
    return vals[int(0.025 * B)], vals[int(0.975 * B)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--ids', nargs='+', required=True)
    ap.add_argument('--unit', default='lines12')
    ap.add_argument('--input', default='txt', choices=['txt', 'readings', 'hira'])
    ap.add_argument('--modes', nargs='+', default=['read', 'read-raw', 'pron'])
    ap.add_argument('--readers', nargs='*', default=None)
    ap.add_argument('--latin2kana', action='store_true', help='also wrap each reader with e2k and alkana')
    ap.add_argument('--only-latin', action='store_true', help='score only units with Latin letters')
    ap.add_argument('--tag', default='')
    a = ap.parse_args()
    os.makedirs(OUT, exist_ok=True)
    min_moras = None if a.unit == 'para' else int(a.unit.replace('lines', ''))
    readers = [r for r in all_readers() if a.readers is None or r.name in a.readers]
    if a.latin2kana:
        from readers import Latin2Kana
        readers = readers + [Latin2Kana(r, h) for r in readers for h in ('e2k', 'alkana')]
    refs = {i: ust_moras(i) for i in a.ids}
    summary = {}
    for rd in readers:
        for mode in a.modes:
            if rd.name.split('+e2k')[0].split('+alkana')[0] in ('pykakasi', 'sudachipy+core', 'kana-split') and mode.startswith('pron'):
                continue
            key = f'{rd.name}/{mode}'
            per = {}
            detail = {}
            for sid in a.ids:
                p = LYR + sid + ('.readings.txt' if a.input == 'readings' and os.path.exists(LYR + sid + '.readings.txt') else '.txt')
                text = clean_text(open(p).read())
                if a.input == 'hira':
                    text = to_hiragana_text(text)
                ref = [m for m in refs[sid]]
                rec = collections.Counter()
                kinds = collections.Counter()
                det = []
                for ms, latin, utext in units_of(text, rd, mode, min_moras):
                    if not ms or (a.only_latin and not latin):
                        continue
                    hyp = [m for m, _ in ms]
                    icost = [0 if (mode == 'pron-opt' and t.get('choon')) else 1 for _, t in ms]
                    e, span, ops, j0 = sellers(hyp, ref, icost)
                    rec['e'] += e
                    rec['n'] += span
                    rec['h'] += len(hyp)
                    if not latin:
                        rec['e_ja'] += e
                        rec['n_ja'] += span
                    ulog = []
                    for _, t in ms:
                        rec['h_' + t['cls']] += 1
                    last_hi = 0
                    for op, hi, rj in ops:
                        if hi is not None:
                            last_hi = hi
                        if op in ('=', 'i'):
                            continue
                        if hi is not None:
                            tag = ms[hi][1]
                            prev_h = hyp[hi - 1] if hi > 0 else None
                        else:
                            tag = ms[last_hi][1]
                            prev_h = ref[rj - 1] if rj > 0 else None
                        rec['e_' + tag['cls']] += 1
                        kd = kind_of(op, hyp[hi] if hi is not None else None, ref[rj] if rj is not None else None, prev_h, tag)
                        kinds[kd] += 1
                        if not latin:
                            kinds['ja:' + kd] += 1
                        ulog.append((op, hyp[hi] if hi is not None else '', ref[rj] if rj is not None else '', tag['surface'], kd))
                    det.append({'text': utext, 'hyp': ''.join(hyp), 'ref': ''.join(ref[j0:j0 + span]), 'edits': e, 'errs': ulog})
                per[sid] = dict(rec, kinds=dict(kinds))
                detail[sid] = det
            # moras by word class (hyp side), for per-class rates
            summary[key] = per
            with open(OUT + f'{a.tag}{a.unit}-{a.input}-{rd.name}-{mode}.json', 'w') as f:
                json.dump(detail, f, ensure_ascii=False, indent=1)
    with open(OUT + f'summary-{a.tag}{a.unit}-{a.input}.json', 'w') as f:
        json.dump(summary, f, ensure_ascii=False, indent=1)
    print(f'unit={a.unit} input={a.input}  songs={len(a.ids)}')
    print(f"{'reader/mode':34s} {'MER':>6s} {'95% CI':>15s} {'MER ja-only':>11s} {'95% CI':>15s}  kinds (all)")
    for key, per in summary.items():
        E = sum(p.get('e', 0) for p in per.values())
        N = sum(p.get('n', 0) for p in per.values())
        Ej = sum(p.get('e_ja', 0) for p in per.values())
        Nj = sum(p.get('n_ja', 0) for p in per.values())
        lo, hi = boot(per, 'e', 'n')
        loj, hij = boot(per, 'e_ja', 'n_ja')
        kinds = collections.Counter()
        for p in per.values():
            kinds.update({k: v for k, v in p['kinds'].items() if not k.startswith('ja:')})
        cls_rate = {}
        for c in ('kanji', 'kana', 'latin', 'number', 'other'):
            h = sum(p.get('h_' + c, 0) for p in per.values())
            if h:
                cls_rate[c] = f"{sum(p.get('e_' + c, 0) for p in per.values()) / h:.3f}/{h}"
        print(f'{key:34s} {E / max(N, 1):6.3f} [{lo:.3f},{hi:.3f}] {Ej / max(Nj, 1):11.3f} [{loj:.3f},{hij:.3f}]  E={E} N={N} cls={cls_rate}  {dict(kinds.most_common())}')


if __name__ == '__main__':
    main()
