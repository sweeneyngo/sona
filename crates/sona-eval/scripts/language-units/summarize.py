"""Print pooled tables from a summary JSON: MER with song-bootstrap CI, per-class rates, kinds, per-song rates."""
import collections
import json
import random
import sys


def boot(per, fe, fn, B=10000, seed=0):
    rnd = random.Random(seed)
    ids = list(per)
    vals = []
    for _ in range(B):
        s = [rnd.choice(ids) for _ in ids]
        vals.append(sum(fe(per[i]) for i in s) / max(1, sum(fn(per[i]) for i in s)))
    vals.sort()
    return vals[int(0.025 * B)], vals[int(0.975 * B)]


def paired(per_a, per_b, B=10000, seed=1):
    rnd = random.Random(seed)
    ids = list(per_a)
    vals = []
    for _ in range(B):
        s = [rnd.choice(ids) for _ in ids]
        ra = sum(per_a[i].get('e_ja', 0) for i in s) / sum(per_a[i].get('n_ja', 0) for i in s)
        rb = sum(per_b[i].get('e_ja', 0) for i in s) / sum(per_b[i].get('n_ja', 0) for i in s)
        vals.append(ra - rb)
    vals.sort()
    return vals[int(0.025 * B)], vals[int(0.975 * B)]


def main():
    summ = json.load(open(sys.argv[1]))
    base = sys.argv[2] if len(sys.argv) > 2 else None
    print(f"{'reader/mode':30s} {'MER':>6s} {'CI':>14s} {'ja-only':>7s} {'CI':>14s} {'kanji':>6s} {'kana':>6s} {'E':>5s} {'N':>5s}")
    for key, per in summ.items():
        E = sum(p.get('e', 0) for p in per.values())
        N = max(1, sum(p.get('n', 0) for p in per.values()))
        Ej = sum(p.get('e_ja', 0) for p in per.values())
        Nj = sum(p.get('n_ja', 0) for p in per.values())
        lo, hi = boot(per, lambda p: p.get('e', 0), lambda p: p.get('n', 0))
        loj, hij = boot(per, lambda p: p.get('e_ja', 0), lambda p: p.get('n_ja', 0))
        cr = {}
        for c in ('kanji', 'kana'):
            h = sum(p.get('h_' + c, 0) for p in per.values())
            cr[c] = sum(p.get('e_' + c, 0) for p in per.values()) / h if h else float('nan')
        print(f'{key:30s} {E / max(N, 1):6.3f} [{lo:.3f},{hi:.3f}] {Ej / max(Nj, 1):7.3f} [{loj:.3f},{hij:.3f}] {cr["kanji"]:6.3f} {cr["kana"]:6.3f} {E:5d} {N:5d}')
    print()
    for key, per in summ.items():
        kinds = collections.Counter()
        kja = collections.Counter()
        for p in per.values():
            for k, v in p['kinds'].items():
                (kja if k.startswith('ja:') else kinds)[k.replace('ja:', '')] += v
        print(f'{key:30s} all: {dict(kinds.most_common())}')
        print(f'{"":30s} ja : {dict(kja.most_common())}')
    print()
    ids = list(next(iter(summ.values())))
    print('per-song MER (ja-only lines):')
    print(f"{'reader/mode':30s} " + ' '.join(f'{i[:8]:>8s}' for i in ids))
    for key, per in summ.items():
        print(f'{key:30s} ' + ' '.join(f"{per[i].get('e_ja', 0) / max(1, per[i].get('n_ja', 0)):8.3f}" for i in ids))
    if base:
        print('\npaired difference in ja-only MER vs', base, '(song bootstrap 95% CI):')
        for key, per in summ.items():
            if key != base:
                lo, hi = paired(per, summ[base])
                print(f'{key:30s} [{lo:+.3f},{hi:+.3f}]')


if __name__ == '__main__':
    main()
