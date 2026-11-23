"""Cancel a running transcription: the server job stops, the song and tempo stay, and a retry starts at once."""
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
    def go():
        page.click("#song-go")
    try:
        page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.5)
        page.set_input_files("#song-file", str(SONG)); page.wait_for_selector("#song-ready:not([hidden])", timeout=30000)
        page.click("#song-half"); go(); time.sleep(4)
        page.click("#song-cancel"); time.sleep(1.5)
        print("after Cancel: ready again", page.is_visible("#song-ready"), "| tempo kept", page.input_value("#song-bpm"), "| message", page.inner_text("#song-error"))
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
