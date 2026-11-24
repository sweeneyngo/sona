"""Japanese text -> hiragana moras, the unit the fallback lyric is scored in (lyrics-first 05).

Mora rules (both sides, reference and hypothesis):
- katakana -> hiragana; a small kana (ゃ, ぃ, …) joins the kana before it, unless that is ー, っ or ん;
- っ and ん are moras of their own; ー is a mora, written as the vowel of the mora before it, because the
  UST writes a held vowel as that vowel (Pifuyuu's USTs have no ー lyric);
- spelling-only variants of one sound fold together: を=お, づ=ず, ぢ=じ;
- a VCV lyric "a か" or "- か" keeps its CV part (か); `R`, breaths and other non-kana are not moras.
  (Pifuyuu's dev USTs are all CV hiragana, so the VCV rule never fires on dev.)

Reading (kanji -> kana) is fugashi with unidic-lite: a word's `kana` field (its written reading, so 東京
is とうきょう, not とーきょー), except particles, which take `pron` (は -> わ, へ -> え), since that is
what is sung. Kana surfaces are kept as written, again except particles. Latin words and digits that
UniDic has no reading for are dropped and counted (`unread`)."""
import re
import unicodedata

SMALL = set("ぁぃぅぇぉゃゅょゎゕゖ")
OWN = set("ーっん")
FOLD = {"を": "お", "づ": "ず", "ぢ": "じ"}
VOWEL_OF = {}
for row, v in (("あかさたなはまやらわがざだばぱぁゃゎ", "あ"), ("いきしちにひみりぎじぢびぴぃ", "い"),
               ("うくすつぬふむゆるぐずづぶぷぅゅゔ", "う"), ("えけせてねへめれげぜでべぺぇ", "え"),
               ("おこそとのほもよろをごぞどぼぽぉょ", "お")):
    for c in row:
        VOWEL_OF[c] = v


def hira(text):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in text)


def is_kana(c):
    return "ぁ" <= c <= "ゖ" or c == "ー"


def kana_moras(text):
    """Moras of a kana string; anything that is not kana is skipped."""
    return tagged_moras([(text, None)])[0]


def tagged_moras(pieces):
    """Moras of a sequence of (kana, tag) pieces read as one string, and each mora's tag (the tag of the
    piece its first kana came from)."""
    out, tags = [], []
    for text, tag in pieces:
        for c in hira(unicodedata.normalize("NFKC", text)):
            if c in "〜～":
                c = "ー"
            if not is_kana(c):
                continue
            c = FOLD.get(c, c)
            if c in SMALL and out and out[-1][-1] not in OWN:
                out[-1] += c
                continue
            if c == "ー":
                prev = out[-1][-1] if out else None
                c = VOWEL_OF.get(prev, "ー") if prev else "ー"
            out.append(c)
            tags.append(tag)
    return out, tags


def ust_lyric_moras(lyric):
    """Moras of one UST note lyric: VCV prefix dropped, rests and breaths give none."""
    lyric = lyric.strip()
    if " " in lyric:
        lyric = lyric.split(" ")[-1]
    return kana_moras(lyric)


_tagger = None


def _tag():
    global _tagger
    if _tagger is None:
        import fugashi
        import unidic_lite
        _tagger = fugashi.Tagger("-r /dev/null -d " + unidic_lite.DICDIR)
    return _tagger


def read_tokens(text):
    """[(start, kana, unread)] per fugashi token of `text` (after NFKC): the token's character offset,
    its reading, and how many of its characters had no reading (Latin letters, numerals; dropped)."""
    text = unicodedata.normalize("NFKC", text)
    out, pos = [], 0
    for w in _tag()(text):
        surf = w.surface
        start = text.find(surf, pos)
        start = pos if start < 0 else start
        pos = start + len(surf)
        f = w.feature
        particle = getattr(f, "pos1", None) == "助詞"
        reading = (f.pron if particle else f.kana) if len(f) > 10 else None
        if reading in (None, "*", ""):
            reading = None
        unread = 0
        if all(is_kana(c) or c in "〜～" for c in hira(surf)) and not particle:
            kana = surf
        elif reading:
            kana = reading
        else:
            kana = surf
            unread = sum(1 for c in surf if c.isalnum() and not is_kana(hira(c)))
        out.append((start, kana, unread))
    return out


def read_moras(text):
    """(moras, unread): kanji/kana text read to moras by fugashi + unidic-lite, and the count of
    non-kana characters with no reading (Latin letters, numerals) that were dropped."""
    toks = read_tokens(text)
    return tagged_moras([(k, None) for _, k, _ in toks])[0], sum(u for _, _, u in toks)


def kakasi_moras(text):
    """The same text read by pykakasi instead (no particle rule): a second reading step to compare."""
    import pykakasi
    k = pykakasi.kakasi()
    kana, unread = [], 0
    # line by line: pykakasi 2.3 garbles kana runs when one call spans line breaks
    for item in (it for line in unicodedata.normalize("NFKC", text).splitlines() for it in k.convert(line)):
        h = item["hira"]
        kana.append(h)
        unread += sum(1 for c in h if c.isalnum() and not is_kana(hira(c)))
    return kana_moras("".join(kana)), unread


def openjtalk_moras(text):
    """The same text read by pyopenjtalk (its pronunciation, so long vowels come out as ー and fold to the
    vowel: 東京 -> と お きょ お): a third reading step to compare."""
    import pyopenjtalk
    kana = [pyopenjtalk.g2p(line, kana=True) for line in unicodedata.normalize("NFKC", text).splitlines()
            if line.strip()]
    unread = sum(1 for c in text if c.isascii() and c.isalnum())
    return kana_moras("".join(kana)), unread


if __name__ == "__main__":
    assert kana_moras("きゃっと") == ["きゃ", "っ", "と"]
    assert kana_moras("ルーム") == ["る", "う", "む"]
    assert kana_moras("ティー") == ["てぃ", "い"]
    assert kana_moras("をづ") == ["お", "ず"]
    assert ust_lyric_moras("a か") == ["か"] and ust_lyric_moras("R") == []
    assert ust_lyric_moras("- ん") == ["ん"]
    m, _ = read_moras("東京へ行くのは")
    assert m == ["と", "う", "きょ", "う", "え", "い", "く", "の", "わ"], m
    assert read_moras("ラララ〜")[0] == ["ら", "ら", "ら", "あ"]
    print("ok")
