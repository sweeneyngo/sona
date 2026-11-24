"""Japanese readers: text -> list of words (surface, reading_kata, pron_kata, particle_kana_or_None, cls)."""
import re
import unicodedata

PARTICLE_SOUND = {'ハ': 'ワ', 'ヘ': 'エ', 'ヲ': 'オ'}


def is_kana(ch):
    c = ord(ch)
    return 0x3041 <= c <= 0x30ff or 0x31f0 <= c <= 0x31ff


def is_kanji(ch):
    c = ord(ch)
    return 0x4e00 <= c <= 0x9fff or 0x3400 <= c <= 0x4dbf or ch == '々' or 0xf900 <= c <= 0xfaff


def hira2kata(s):
    return ''.join(chr(ord(c) + 0x60) if 0x3041 <= ord(c) <= 0x3096 else c for c in s)


def word_class(surface):
    surface = unicodedata.normalize("NFKC", surface)
    if any(is_kanji(c) for c in surface):
        return 'kanji'
    if any(c.isascii() and c.isalpha() for c in surface):
        return 'latin'
    if any(c.isdigit() for c in surface):
        return 'number'
    if surface and all(is_kana(c) for c in surface):
        return 'kana'
    return 'other'


def spoken(surface):
    s = unicodedata.normalize('NFKC', surface)
    return any(is_kana(c) or is_kanji(c) or c.isalnum() for c in s if c != '・')


def drop_symbols(words):
    """A word with nothing speakable in it (space, punctuation) reads as nothing."""
    return [w if spoken(w[0]) else (w[0], '', '', None, w[4]) for w in words]


class KanaSplit:
    """No reader: the text's own kana, as a user pastes it (lyrics.ts, with the UST conventions)."""
    name = 'kana-split'

    def words(self, text):
        return [(text, fallback(text), fallback(text), None, 'kana')]


def fallback(surface):
    """Reading for a word the reader gives none: its kana, if it is kana."""
    return hira2kata(''.join(c for c in surface if is_kana(c)))


class PyOpenJTalk:
    def __init__(self, vanilla=False):
        import pyopenjtalk
        self.p = pyopenjtalk
        self.vanilla = vanilla
        self.name = 'openjtalk-vanilla' if vanilla else 'pyopenjtalk-plus'

    def words(self, text):
        out = []
        for f in self.p.run_frontend(text, use_vanilla=self.vanilla):
            s = f['string']
            read, pron = f.get('read', ''), f.get('pron', '')
            read = read.replace('’', '')
            pron = pron.replace('’', '')
            if read in ('、', '。', '？', '！', '') and not any(is_kana(c) for c in read):
                read, pron = fallback(s), fallback(s)
            part = None
            if f['pos'] == '助詞' and s in ('は', 'へ', 'を'):
                part = PARTICLE_SOUND[hira2kata(s)]
            out.append((s, read, pron, part, word_class(s)))
        return drop_symbols(out)


class Fugashi:
    def __init__(self, dic):
        import fugashi
        self.dic = dic
        if dic == 'unidic-lite':
            self.t = fugashi.Tagger()
            self.name = 'fugashi+unidic-lite'
        else:
            import ipadic
            self.t = fugashi.GenericTagger(ipadic.MECAB_ARGS)
            self.name = 'fugashi+ipadic'

    def words(self, text):
        out = []
        for w in self.t(text):
            s = w.surface
            if self.dic == 'unidic-lite':
                f = w.feature
                read, pron, pos = f.kana, f.pron, f.pos1
            else:
                f = w.feature
                pos = f[0]
                read = f[7] if len(f) > 7 else None
                pron = f[8] if len(f) > 8 else None
            if not read or read == '*':
                read = fallback(s)
            if not pron or pron == '*':
                pron = read
            part = PARTICLE_SOUND[hira2kata(s)] if pos == '助詞' and s in ('は', 'へ', 'を') else None
            out.append((s, read, pron, part, word_class(s)))
        return drop_symbols(out)


class Sudachi:
    name = 'sudachipy+core'

    def __init__(self):
        from sudachipy import dictionary, tokenizer
        self.t = dictionary.Dictionary(dict='core').create()
        self.mode = tokenizer.Tokenizer.SplitMode.C

    def words(self, text):
        out = []
        for m in self.t.tokenize(text, self.mode):
            s = m.surface()
            read = m.reading_form()
            if not read or not any(is_kana(c) for c in read):
                read = fallback(s)
            pos = m.part_of_speech()[0]
            part = PARTICLE_SOUND[hira2kata(s)] if pos == '助詞' and s in ('は', 'へ', 'を') else None
            out.append((s, read, read, part, word_class(s)))
        return drop_symbols(out)


class Kakasi:
    name = 'pykakasi'

    def __init__(self):
        import pykakasi
        self.k = pykakasi.kakasi()

    def words(self, text):
        out = []
        for w in self.k.convert(text):
            s = w['orig']
            read = w['kana']
            if not any(is_kana(c) for c in read):
                read = fallback(s)
            out.append((s, read, read, None, word_class(s)))
        return drop_symbols(out)


def all_readers():
    rs = []
    for ctor in (KanaSplit, lambda: PyOpenJTalk(True), PyOpenJTalk, lambda: Fugashi('unidic-lite'), lambda: Fugashi('ipadic'), Sudachi, Kakasi):
        try:
            rs.append(ctor())
        except Exception as e:  # noqa: BLE001
            print('reader unavailable:', ctor, e)
    return rs


def clean_text(t):
    t = unicodedata.normalize('NFKC', t)
    t = re.sub(r'※注', '', t)
    return t


class Latin2Kana:
    """Replace each Latin word with katakana (e2k C2K or alkana) before the base reader runs."""

    def __init__(self, base, how):
        self.base, self.how = base, how
        self.name = f'{base.name}+{how}'
        if how == 'e2k':
            from e2k import C2K
            self.c2k = C2K()
        else:
            import alkana
            self.alk = alkana

    def kana(self, w):
        if self.how == 'e2k':
            return self.c2k(w.lower())
        k = self.alk.get_kana(w.lower())
        return k if k else w

    def words(self, text):
        text = unicodedata.normalize('NFKC', text)
        text = re.sub(r"[A-Za-z][A-Za-z']*", lambda m: self.kana(m.group(0)), text)
        return self.base.words(text)
