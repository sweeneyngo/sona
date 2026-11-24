"""The lyric on the start screen (lyrics-first 03): Transcribe needs one; a katakana line is refused, naming its
characters; the lyric goes with the job, lands on the notes a mora each, and a re-transcription and a session
reload keep it. The lyric is made up. Needs `sona serve`; the song is sent as a stem (no separation), so the
synthetic fixture's stem is enough (`SONA_E2E_SONG` overrides it)."""
import json, os, time, urllib.parse
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []
S = OUT / "shots"
LOADED = "document.querySelector('#start').hidden && !document.querySelector('#play').disabled"
SONG_FILE = SONG if os.environ.get("SONA_E2E_SONG") else STEM
LYRIC = "あめが ふる よるの\nまちを あるく\nきみの こえが きこえる\nらら るら りら"
KATAKANA = "あめが ふる よるの\nマチを アルク"

def moras(text):
    out = []
    for ch in text:
        if ch.isspace():
            continue
        if ch in "ぁぃぅぇぉゃゅょゎ" and out and out[-1] not in "っーん":
            out[-1] += ch
        else:
            out.append(ch)
    return out

def lyrics_in_order(session):
    return [n.get("lyric") for n in sorted(session["notes"], key=lambda n: n["onset_s"])]

def laid(session):
    """The notes' lyrics as laying the lyric in order gives them: a mora each, none once it runs out."""
    n = len(session["notes"])
    return (moras(LYRIC) + [None] * n)[:n]

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1440, "height": 900}, accept_downloads=True)
    page = ctx.new_page(); watch(page, errors)
    posts = []
    page.on("request", lambda r: r.method == "POST" and "/api/jobs" in r.url and posts.append(dict(urllib.parse.parse_qsl(urllib.parse.urlsplit(r.url).query))))
    page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.5)

    page.set_input_files("#song-file", str(SONG_FILE)); page.wait_for_selector("#song-ready:not([hidden])", timeout=5000)
    page.keyboard.type("125")
    page.click("#song-more summary"); page.select_option("#song-separation", "none"); page.select_option("#song-engine", "pyin")
    check(page.is_visible("#song-lyric") and page.input_value("#song-lyric") == "", "the lyric box is on the start screen, empty")
    check(page.is_disabled("#song-lyric-asr") and "not yet" in page.inner_text(".lyric-fallback"), f"'Transcribe the lyric for me' is shown, off: {page.inner_text('.lyric-fallback')!r}")

    page.click("#song-go"); time.sleep(0.3)
    check(page.is_visible("#song-ready") and page.inner_text("#song-error") == "Paste the song's lyric in hiragana, one sung line per line." and not posts,
          f"Transcribe with no lyric starts nothing and asks for it: {page.inner_text('#song-error')!r}")

    page.fill("#song-lyric", KATAKANA); time.sleep(0.2)
    shown = page.inner_text("#song-lyric-check")
    check(shown == "Hiragana only for now. Line 2: マ チ ア ル ク" and "bad" in page.get_attribute("#song-lyric-check", "class"), f"a katakana line is named as you type: {shown!r}")
    page.screenshot(path=str(S / "L1-katakana.png"))
    page.click("#song-go"); time.sleep(0.3)
    check(page.inner_text("#song-error") == shown and not posts, "Transcribe refuses it with the same words and starts nothing")
    check(page.input_value("#song-lyric") == KATAKANA, "nothing was converted")

    page.fill("#song-lyric", LYRIC); time.sleep(0.2)
    check(page.inner_text("#song-lyric-check") == f"4 lines · {len(moras(LYRIC))} moras", f"a hiragana lyric is counted: {page.inner_text('#song-lyric-check')!r}")
    page.screenshot(path=str(S / "L2-lyric.png"))
    page.click("#song-go"); page.wait_for_function(LOADED, timeout=900_000); time.sleep(0.3)
    check(len(posts) >= 1 and all(q.get("lyric") == LYRIC for q in posts), f"the job carries the lyric, line breaks and all: {[q.get('lyric') for q in posts]}")
    status = page.inner_text("#status")
    first = save(page)
    check(first["provenance"]["job"].get("lyric") == LYRIC, "the server kept the lyric with the job")
    check(lyrics_in_order(first) == laid(first) and "lyric on" in status, f"the notes take the lyric a mora each, in order: {lyrics_in_order(first)[:10]} | {status!r}")
    print(f"{len(first['notes'])} notes, {len(moras(LYRIC))} moras | status {status!r}")
    page.screenshot(path=str(S / "L3-notes.png"))
    mclick(page, "#lyrics"); time.sleep(0.2)
    check(page.input_value("#lyrics-text") == LYRIC, "Edit → Lyrics… holds the song's lyric"); page.click("#lyrics-close")

    posts.clear()
    page.click("#tempo-chip"); page.click("#bpm-half"); time.sleep(0.2)
    page.click("#retranscribe"); page.wait_for_function(LOADED, timeout=900_000); time.sleep(0.3)
    page.evaluate("window.dispatchEvent(new Event('beforeunload'))"); time.sleep(0.2)  # autosave, as leaving the page does
    again = save(page)
    check(posts and all(q.get("lyric") == LYRIC for q in posts), f"Re-transcribe sends the same lyric: {[(q.get('upload', 'file')[:8], q.get('lyric')) for q in posts]}")
    check(again["bpm"] != first["bpm"] and lyrics_in_order(again) == laid(again), f"the new notes take it too: {lyrics_in_order(again)[:10]}")

    page2 = ctx.new_page(); watch(page2, errors)
    page2.goto(SERVE_URL); time.sleep(0.5)
    page2.click("#start-resume"); time.sleep(0.5)
    resumed = save(page2)
    check(lyrics_in_order(resumed) == lyrics_in_order(again), "a session reload keeps the notes' lyrics")
    check(resumed["provenance"]["job"].get("lyric") == LYRIC, "and the song's lyric")
    mclick(page2, "#lyrics"); time.sleep(0.2)
    check(page2.input_value("#lyrics-text") == LYRIC, "Edit → Lyrics… still holds it after the reload"); page2.click("#lyrics-close")
    b.close()
finish(errors)
