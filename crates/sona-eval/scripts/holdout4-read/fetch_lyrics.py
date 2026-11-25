"""utau-edit-cost 17: a song's lyric text from the Vocaloid Lyrics Wiki (vocaloidlyrics.miraheze.org), as the dev texts.

Fetches `https://vocaloidlyrics.miraheze.org/wiki/<title>` (the article paths robots.txt allows; never `/w/`,
`?action=` or `Special:`), one page at a time with a pause between requests, under a generic user agent. From the
page's first `lyrics-table` it writes, as `data/pifuyuu-lyrics` holds them:
  <id>.txt           the original column, one sung line per line, a blank line between stanzas (ruby as its base)
  <id>.readings.txt  the same with each ruby base replaced by its reading (only when the page has ruby)
  <id>.romaji.txt    the romaji column
and prints the index entry (source_urls, files, lines) for the caller to complete. It reads no UST.

Usage: python3 -I fetch_lyrics.py page TITLE --id ID --out DIR      # one song
       python3 -I fetch_lyrics.py parse FILE.html --id ID --out DIR # a page already fetched
Lyric texts are copyrighted: they stay in the git-ignored data folder, never in the repo.
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import html, json, os, re, time, urllib.parse, urllib.request  # noqa: E402

WIKI = "https://vocaloidlyrics.miraheze.org/wiki/"
AGENT = "sona-lyrics-fetch/1.0 (local research script)"
PAUSE_S = 3.0


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def fetch(title):
    """The rendered article, or None when the wiki has no such page (404)."""
    url = WIKI + urllib.parse.quote(title.replace(" ", "_"), safe="()/_-,.!'")
    req = urllib.request.Request(url, headers={"User-Agent": AGENT})
    time.sleep(PAUSE_S)
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            return r.geturl(), r.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return url, None
        raise


def cell_text(cell, reading, br=" "):
    """A table cell as text: ruby as its base (or its reading), tags dropped, entities decoded; `<br>` becomes
    `br`. ASCII whitespace collapses; the ideographic space is kept, as the dev texts keep it."""
    cell = re.sub(r"<rp>.*?</rp>", "", cell, flags=re.S)
    cell = re.sub(r"<ruby>\s*<rb>(.*?)</rb>\s*<rt>(.*?)</rt>\s*</ruby>", (lambda m: m.group(2)) if reading else (lambda m: m.group(1)), cell, flags=re.S)
    cell = re.sub(r"<br\s*/?>", "\x00", cell)
    cell = re.sub(r"<[^>]+>", "", cell)
    parts = [re.sub(r"[ \t\r\n]+", " ", p).strip(" ") for p in html.unescape(cell).split("\x00")]
    return br.join(p for p in parts if p)


def lyrics(page):
    """[(original, reading, romaji) per row; None for a stanza break] from the page's first lyrics table, and
    the number of lyrics tables."""
    tables = re.findall(r'<table class="lyrics-table".*?</table>', page, flags=re.S)
    if not tables:
        return None, 0
    rows = []
    for tr in re.findall(r"<tr[^>]*>(.*?)</tr>", tables[0], flags=re.S):
        if "<th" in tr:
            continue
        tds = re.findall(r"<td([^>]*)>(.*?)</td>", tr, flags=re.S)
        if not tds:
            continue
        if "colspan" in tds[0][0]:
            # A line across all columns (spoken parts, a poem): each of its lines as sung, in every file.
            for o, r in zip(cell_text(tds[0][1], False, "\n").split("\n"), cell_text(tds[0][1], True, "\n").split("\n")):
                if o:
                    rows.append((o, r, o))
            continue
        orig = cell_text(tds[0][1], False)
        if not orig:
            rows.append(None)
            continue
        rows.append((orig, cell_text(tds[0][1], True), cell_text(tds[1][1], False) if len(tds) > 1 else ""))
    while rows and rows[-1] is None:
        rows.pop()
    return rows, len(tables)


def write(rows, sid, out, has_ruby):
    def lines(k):
        out_lines, prev_blank = [], True
        for r in rows:
            if r is None:
                if not prev_blank:
                    out_lines.append("")
                prev_blank = True
            else:
                out_lines.append(r[k])
                prev_blank = False
        return "\n".join(out_lines).rstrip("\n") + "\n"

    files = [(f"{sid}.txt", 0), (f"{sid}.romaji.txt", 2)] + ([(f"{sid}.readings.txt", 1)] if has_ruby else [])
    os.makedirs(out, exist_ok=True)
    for name, k in files:
        head = f"# Romaji from Vocaloid Lyrics Wiki (fan romanization; follows the wiki line structure, which may differ from {sid}.txt)\n" if k == 2 else ""
        open(os.path.join(out, name), "w", encoding="utf-8").write(head + lines(k))
    return sorted(name for name, _ in files)


def main():
    cmd, sid, out = sys.argv[1], arg("--id"), arg("--out")
    if cmd == "page":
        url, page = fetch(sys.argv[2])
        if page is None:
            print(json.dumps({"id": sid, "found": False, "tried": url}, ensure_ascii=False))
            return
        pages = arg("--pages", out)
        os.makedirs(pages, exist_ok=True)
        open(os.path.join(pages, f"{sid}.html"), "w", encoding="utf-8").write(page)
    elif cmd == "parse":
        url, page = arg("--url", ""), open(sys.argv[2], encoding="utf-8").read()
    else:
        sys.exit(__doc__)
    rows, tables = lyrics(page)
    if not rows:
        print(json.dumps({"id": sid, "found": True, "lyrics": False, "url": url}, ensure_ascii=False))
        return
    files = write(rows, sid, out, "<ruby>" in page)
    print(json.dumps({"id": sid, "found": True, "url": url, "files": files, "lines": sum(r is not None for r in rows), "tables": tables}, ensure_ascii=False))


if __name__ == "__main__":
    main()
