"""A decimal BPM typed straight after the drop, before the estimate: it is the job's BPM. Cancel stops the server
job, the song and BPM stay, and a retry starts at once."""
import json, re, time, urllib.request
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []
def server_job(jid):
    return json.load(urllib.request.urlopen(f"{SERVE_URL}api/jobs/{jid}"))["phase"]
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1440, "height": 860}).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    jobs = []
    def seen(r):
        m = re.search(r"/api/jobs/([^/?]+)$", r.url)
        if m and m.group(1) not in jobs: jobs.append(m.group(1))
    page.on("request", seen)
    sent = []
    page.on("request", lambda r: r.method == "POST" and "/api/jobs" in r.url and sent.append(re.search(r"[?&]bpm=([^&]+)", r.url).group(1)))
    def go():
        page.click("#song-go")
    try:
        page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.5)
        page.set_input_files("#song-file", str(SONG)); page.wait_for_selector("#song-ready:not([hidden])", timeout=5000)
        page.keyboard.type("123,5"); print("typed before the estimate, hint:", repr(page.inner_text("#song-hint")))
        go(); time.sleep(4)
        print("BPM sent with the job:", sent)
        assert sent == ["123.5"], "the typed decimal BPM was not the job's"
        page.click("#song-cancel"); time.sleep(1.5)
        print("after Cancel: ready again", page.is_visible("#song-ready"), "| tempo kept", page.input_value("#song-bpm"), "| hint", repr(page.inner_text("#song-hint")), "| message", page.inner_text("#song-error"))
        assert page.input_value("#song-bpm") == "123.5", "the BPM was not kept after Cancel"
        print("server job after Cancel:", server_job(jobs[0]))
        go(); time.sleep(5)
        print("retry phase after 5 s:", page.inner_text("#song-phase"), "| server:", server_job(jobs[-1]))
        page.click("#song-cancel"); time.sleep(1)
        print("second job after Cancel:", server_job(jobs[-1]), "| jobs submitted:", len(jobs))
    finally:
        for jid in jobs:  # never leave a job holding the server's queue
            urllib.request.urlopen(urllib.request.Request(f"{SERVE_URL}api/jobs/{jid}", method="DELETE")).read()
    b.close()
print("console errors:", errors or "none")
