"""The start screen's Bar 1 choice (ADR-0036): First grid line, From the harmony, and From Beat This only when the
server has Beat This; otherwise the install line shows under Options and not among the setup notes. With Beat This
installed, a job asks for bar_one=beat-this and the ♩ popover names it and says whether the harmony agrees. Needs
`sona serve` (stub separators and a stand-in Beat This are enough; run it once without the tool and once with)."""
import json, time, urllib.request, wave
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []
LOADED = "document.querySelector('#start').hidden && !document.querySelector('#play').disabled"

caps = json.load(urllib.request.urlopen(f"{SERVE_URL}api/capabilities"))
installed = caps["bar_one"]["beat-this"]
song = OUT / "bar_one_choice.wav"
with wave.open(str(song), "wb") as w:
    import math, struct
    w.setnchannels(1); w.setsampwidth(2); w.setframerate(44100)
    frames = bytearray()
    for i in range(44100 * 4):
        t = i / 44100
        on = (t % 0.5) < 0.3
        frames += struct.pack("<h", int(12000 * math.sin(2 * math.pi * (440 if (t % 1.0) < 0.5 else 494) * t)) if on else 0)
    w.writeframes(bytes(frames))

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    page = b.new_context(viewport={"width": 1440, "height": 860}).new_page(); watch(page, errors)
    posts = []
    page.on("request", lambda r: r.method == "POST" and "/api/jobs" in r.url and posts.append(r.url))
    page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.5)
    page.set_input_files("#song-file", str(song)); page.wait_for_selector("#song-ready:not([hidden])", timeout=5000)
    page.keyboard.type("120"); page.fill("#song-lyric", "あめが ふる\nかぜが ふく")
    page.click("#song-more summary"); time.sleep(0.2)
    offered = page.eval_on_selector_all("#song-bar-one option", "os => os.filter(o => !o.hidden && !o.disabled).map(o => o.value)")
    print("server has Beat This:", installed, "| offered:", offered)
    check(offered == (["off", "harmonic", "beat-this"] if installed else ["off", "harmonic"]), "Beat This is offered exactly when the server has it")
    check(page.input_value("#song-bar-one") == "off", "the default is today's rule, the first grid line")
    note = page.inner_text("#song-bar-one-note") if page.is_visible("#song-bar-one-note") else ""
    listed = page.inner_text("#setup-notes") if page.is_visible("#setup-notes") else ""
    if installed:
        check(note == "", "no install line when it is installed")
    else:
        check("setup-separators.sh beat-this" in note, f"the install line shows under Options: {note!r}")
        check("Beat This" not in listed, "and not among the start screen's setup notes")
        print("failures:", failures or "none"); print("console errors:", errors or "none")
        raise SystemExit(1 if failures or errors else 0)

    page.select_option("#song-separation", "vocals"); page.select_option("#song-bar-one", "beat-this")
    page.click("#song-go"); page.wait_for_function(LOADED, timeout=120_000); time.sleep(0.3)
    check(any("bar_one=beat-this" in u for u in posts), f"the job asks for bar 1 from Beat This: {posts}")
    page.click("#tempo-chip"); time.sleep(0.2)
    readout = page.inner_text("#bar-one-readout")
    print("♩ popover:", readout)
    check("Beat This" in readout and ("the harmony agrees" in readout or "the harmony says" in readout), "the popover names Beat This and the harmony beside it")
    disagree = "the harmony says" in readout
    check(page.eval_on_selector("#bar-one-readout", "e => e.classList.contains('disagree')") == disagree, "a disagreement, and only a disagreement, is flagged")
    page.screenshot(path=str(OUT / "shots" / "bar_one_choice.png"))
    finish(errors)
