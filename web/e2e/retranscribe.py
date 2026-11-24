"""Re-transcribe from the editor (ADR-0032). Transcribe names the dropped song by its hash and sends the file only
when the server refuses it; after a lyric edit and ½× in the ♩ popover, "Re-transcribe at N BPM" sends the hash
alone, the new notes load, and Undo brings back the edited notes in one step (Redo the new ones). Needs `sona serve`."""
import hashlib, json, re, time, urllib.request
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []
LOADED = "document.querySelector('#start').hidden && !document.querySelector('#play').disabled"
SHA = hashlib.sha256(SONG.read_bytes()).hexdigest()

def first_log_line(jid):
    return json.load(urllib.request.urlopen(f"{SERVE_URL}api/jobs/{jid}"))["log"][0]

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    page = b.new_context(viewport={"width": 1440, "height": 860}, accept_downloads=True).new_page(); watch(page, errors)
    posts, answers, jobs = [], [], []
    page.on("request", lambda r: r.method == "POST" and "/api/jobs" in r.url and posts.append(r.url))
    page.on("response", lambda r: r.request.method == "POST" and "/api/jobs" in r.url and answers.append(r.status))
    def seen(r):
        m = re.search(r"/api/jobs/([0-9a-f]+)$", r.url)
        if m and m.group(1) not in jobs: jobs.append(m.group(1))
    page.on("request", seen)
    page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.5)

    page.set_input_files("#song-file", str(SONG)); page.wait_for_selector("#song-ready:not([hidden])", timeout=5000)
    page.keyboard.type("120"); time.sleep(1)
    page.click("#song-go"); page.wait_for_function(LOADED, timeout=1_500_000); time.sleep(0.3)
    sent = first_log_line(jobs[-1])
    print("first transcription:", ["hash" if f"upload={SHA}" in u else "file" for u in posts], answers, "| server:", sent)
    check(f"upload={SHA}" in posts[0], "Transcribe names the song by its hash first")
    if answers[0] in (404, 409):
        check(len(posts) == 2 and "upload=" not in posts[1] and answers[1] == 201 and f"received {SONG.stat().st_size} bytes" in sent, "a refused hash is followed by the file")
    else:
        check(answers == [201] and "named its upload by hash" in sent, "a song the server has starts without sending the file")

    page.click("#tempo-chip"); time.sleep(0.2)
    check(page.is_hidden("#retranscribe"), "nothing to offer at the tempo the notes were transcribed at")
    page.keyboard.press("Escape")
    mclick(page, "#lyrics"); page.fill("#lyrics-text", "la li lu le lo"); page.click("#lyrics-from-start"); time.sleep(0.2)
    page.click("#tempo-chip"); page.click("#bpm-half"); time.sleep(0.2)
    label = page.inner_text("#retranscribe") if page.is_visible("#retranscribe") else None
    check(label == "Re-transcribe at 60 BPM", f"½× offers a re-transcription: {label!r}")
    page.keyboard.press("Escape")
    edited = save(page)
    check(any(n.get("lyric") for n in edited["notes"]), "the edit to protect is in the notes")

    posts.clear(); answers.clear(); t0 = time.time()
    page.click("#tempo-chip"); page.click("#retranscribe"); page.wait_for_function(LOADED, timeout=600_000); took = time.time() - t0; time.sleep(0.3)
    sent = first_log_line(jobs[-1])
    print(f"re-transcribed in {took:.1f} s; status {page.inner_text('#status')!r}; chip {page.inner_text('#tempo-chip-text')!r}; server: {sent}")
    check(len(posts) == 1 and f"upload={SHA}" in posts[0] and "bpm=60" in posts[0] and answers == [201], f"the hash alone, at 60 BPM: {posts} {answers}")
    check("named its upload by hash" in sent, "the server ran it on its own copy of the song")
    after = save(page)
    stems = after["provenance"]["job"]["stems"]
    check(after["bpm"] == 60 and after["session"]["bpm"] == 60 and page.inner_text("#tempo-chip-text").startswith("60"), "the new notes are at 60 BPM")
    check(stems.get("by_hash") is True and stems["reused"] is True, f"the job reused the stems by hash: {stems}")
    check(not any(n.get("lyric") for n in after["notes"]), "the new notes replace the edited ones")

    mclick(page, "#edit-undo"); time.sleep(0.3)
    undone = save(page)
    check(undone["notes"] == edited["notes"] and undone["bpm"] == 120 and undone["session"]["bpm"] == 60, "one Undo brings back the edited notes and the transcription they were on")
    check(undone["provenance"] == edited["provenance"], "and that transcription's provenance")
    page.click("#tempo-chip"); time.sleep(0.2)
    check(page.is_visible("#retranscribe"), "the offer is back with them"); page.keyboard.press("Escape")
    mclick(page, "#edit-redo"); time.sleep(0.3)
    check(save(page)["notes"] == after["notes"], "Redo brings back the re-transcription")
    b.close()
finish(errors)
