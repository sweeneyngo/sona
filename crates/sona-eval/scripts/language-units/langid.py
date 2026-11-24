"""Text language ID on the dev lyric texts: a script rule vs lingua, per line and per song."""
import collections
import os
import re
import sys
import unicodedata

D = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'data', 'pifuyuu-lyrics') + '/'


def script_counts(s):
    c = collections.Counter()
    for ch in unicodedata.normalize('NFKC', s):
        o = ord(ch)
        if 0x3041 <= o <= 0x30ff:
            c['kana'] += 1
        elif 0x4e00 <= o <= 0x9fff or ch == '々':
            c['han'] += 1
        elif 0xac00 <= o <= 0xd7af:
            c['hangul'] += 1
        elif ch.isascii() and ch.isalpha():
            c['latin'] += 1
    return c


def rule(s):
    c = script_counts(s)
    if not c:
        return None
    if c['kana']:
        return 'ja'
    if c['hangul']:
        return 'ko'
    if c['han']:
        return 'zh?'  # han without kana: Chinese, or a kana-less Japanese line
    return 'latin'


def main():
    from lingua import LanguageDetectorBuilder
    det = LanguageDetectorBuilder.from_all_languages().with_preloaded_language_models().build()
    tot = collections.Counter()
    for sid in sys.argv[1:]:
        t = open(D + sid + '.txt').read()
        lines = [l for l in t.splitlines() if l.strip()]
        song_rule = rule(t)
        song_lingua = det.detect_language_of(t)
        per = collections.Counter()
        disagree = 0
        for l in lines:
            r = rule(l)
            g = det.detect_language_of(l)
            g = g.iso_code_639_1.name.lower() if g else None
            per[(r, g)] += 1
            if r == 'ja' and g != 'ja':
                disagree += 1
            tot[(r, g)] += 1
        print(sid, 'song: rule', song_rule, 'lingua', song_lingua.iso_code_639_1.name.lower() if song_lingua else None,
              '| lines', len(lines), 'rule/lingua pairs', dict(per))
    print('pooled line (rule, lingua) pairs:', dict(tot))


if __name__ == '__main__':
    main()
