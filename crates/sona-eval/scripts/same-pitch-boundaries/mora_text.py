"""Read each dev song's wiki lyric text (readings.txt when present) into a mora stream as a user pasting kana
would give it: kanji read by fugashi/unidic-lite, kana kept, Latin words counted by vowel groups.
Writes texts/<id>.json: [[kind, line_start], ...]. Lyric-derived: stays in the scratchpad."""
import json, os, re, sys, unicodedata
import fugashi, unidic_lite

LY = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "data", "pifuyuu-lyrics") + os.sep
HERE = os.environ.get("S07_WORK", ".")
SMALL = set("ぁぃぅぇぉゃゅょゎゕゖァィゥェォャュョヮヵヶ")
VOW = set("あいうえおぁぃぅぇぉアイウエオァィゥェォ")
tag = fugashi.Tagger("-r /dev/null -d " + unidic_lite.DICDIR)


def is_kana(c):
    return "ぁ" <= c <= "ヿ"


def kana_moras(text):
    out = []
    for c in text:
        if not is_kana(c) or c in "・":
            continue
        if c in SMALL and out and out[-1] not in "ーっッんン":
            out[-1] += c
        else:
            out.append(c)
    return out


def kind(m):
    h = "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in m)
    if h in ("ー",):
        return "x"
    if h == "ん":
        return "n"
    if h == "っ":
        return "x"
    if len(h) == 1 and h in "あいうえおぁぃぅぇぉ":
        return "v"
    return "c"


def line_moras(line):
    line = unicodedata.normalize("NFKC", line)
    out = []
    for w in tag(line):
        surf = w.surface
        if any(is_kana(c) for c in surf) and all(is_kana(c) or not c.isalnum() for c in surf):
            out += kana_moras(surf)
        elif any("一" <= c <= "鿿" or c == "々" for c in surf):
            k = w.feature.kana if w.feature.kana and w.feature.kana != "*" else ""
            out += kana_moras(k)
        elif re.search("[A-Za-z]", surf):
            for word in re.findall("[A-Za-z']+", surf):
                n = max(1, len(re.findall("[aeiouy]+", word.lower())) - (1 if word.lower().endswith("e") and len(word) > 3 else 0))
                out += ["c"] * n  # placeholder consonant moras
    return out


for sid in sys.argv[1:]:
    p = LY + sid + ".readings.txt"
    if not os.path.exists(p):
        p = LY + sid + ".txt"
    stream = []
    for line in open(p, encoding="utf-8").read().splitlines():
        if line.startswith("#"):
            continue
        ms = line_moras(line)
        for j, m in enumerate(ms):
            stream.append([kind(m) if m != "c" else "c", j == 0])
    os.makedirs(os.path.join(HERE, "texts"), exist_ok=True)
    json.dump(stream, open(os.path.join(HERE, "texts", sid + ".json"), "w"))
    print(sid, os.path.basename(p), "moras", len(stream), "lines", sum(1 for _, b in stream if b))
