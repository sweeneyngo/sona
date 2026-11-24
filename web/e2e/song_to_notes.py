"""The one-click path a newcomer takes: drop a song, give its BPM (here by picking the detected ½× hint), transcribe, play; then the secondary controls."""
import hashlib, json, pathlib, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; S = OUT / "shots"; errors = []
SONG = SONG
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1440, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    page.goto(SERVE_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.6)
    page.screenshot(path=str(S / "U1-start.png"))
    print("visible controls on the start screen:", page.evaluate("Array.from(document.querySelectorAll('header button, header input')).filter(e => e.offsetParent).map(e => e.id || e.textContent.trim())"), "| inputs:", page.evaluate("Array.from(document.querySelectorAll('input,select,textarea')).filter(e => e.offsetParent && e.type !== 'file').length"))
    page.set_input_files("#song-file", str(SONG))
    page.wait_for_selector("#song-ready:not([hidden])", timeout=5000)
    print("on drop: BPM field", repr(page.input_value("#song-bpm")), "| focused:", page.evaluate("document.activeElement.id"), "| hint:", repr(page.inner_text("#song-hint")))
    page.wait_for_selector("#song-hint-half:not([hidden])", timeout=60000)
    page.screenshot(path=str(S / "U2-ready.png"))
    print("detected:", repr(page.inner_text("#song-hint")), "| BPM field after the estimate:", repr(page.input_value("#song-bpm")))
    assert page.input_value("#song-bpm") == "", "the estimate filled the BPM field by itself"
    page.click("#song-go")
    print("Transcribe with no BPM:", page.is_visible("#song-ready"), repr(page.inner_text("#song-error")))
    assert page.is_visible("#song-ready") and "BPM" in page.inner_text("#song-error"), "Transcribe ran without a BPM"
    half = page.inner_text("#song-hint-half").split()[-1]
    page.click("#song-hint-half"); print("after picking", repr(page.inner_text("#song-hint-half")) + ":", page.input_value("#song-bpm"))
    assert page.input_value("#song-bpm") == half, "picking the hint did not fill its value"
    t0 = time.time(); page.click("#song-go")
    page.wait_for_selector("#song-running:not([hidden])", timeout=10000); time.sleep(3)
    page.screenshot(path=str(S / "U3-running.png")); print("running:", page.inner_text("#song-phase"))
    page.wait_for_selector("#start", state="hidden", timeout=1500000); page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000); time.sleep(0.3)
    print(f"loaded after {time.time() - t0:.0f} s; status: {page.inner_text('#status')!r}; tempo chip: {page.inner_text('#tempo-chip')!r}")
    time.sleep(0.5); page.screenshot(path=str(S / "U4-loaded.png"))
    print("visible inputs in the editor:", page.evaluate("Array.from(document.querySelectorAll('input,select,textarea')).filter(e => e.offsetParent && e.type !== 'file').map(e => e.id)"))
    mclick(page, "#details"); time.sleep(0.3)
    sections = page.eval_on_selector_all("#details-body h3", "hs => hs.map(h => h.textContent)")
    body = page.inner_text("#details-body")
    print("details sections:", sections, "| engine game:", "game" in body, "| steps listed:", "separating-vocals" in body)
    with page.expect_download() as dl: page.click("#details-download")
    prov = json.load(open(dl.value.path()))
    print("downloaded provenance: upload", prov["job"]["upload"]["name"], prov["job"]["upload"]["sha256"][:12], "| sha256sum of the song:", hashlib.sha256(SONG.read_bytes()).hexdigest()[:12])
    page.keyboard.press("Escape")
    page.click("#play"); time.sleep(1.2); print("play button while playing:", page.inner_text("#play"), page.get_attribute("#play", "aria-label"))
    page.keyboard.press("Space"); time.sleep(0.3); print("after Space:", page.inner_text("#play"))
    page.click("#tempo-chip"); time.sleep(0.3); page.screenshot(path=str(S / "U5-tempo-pop.png"))
    page.click("#bpm-double"); print("chip after 2×:", page.inner_text("#tempo-chip")); page.click("#bpm-half")
    page.keyboard.press("Escape"); print("popover closed by Escape:", page.is_hidden("#tempo-pop"))
    page.click("text=View"); time.sleep(0.3); page.screenshot(path=str(S / "U6-view-menu.png"))
    page.keyboard.press("ArrowDown"); page.keyboard.press("Enter"); time.sleep(0.2)
    print("pitch curve toggled from the keyboard:", not page.is_checked("#show-curve"))
    page.keyboard.press("Escape")
    page.click("text=File"); page.click("#generate"); time.sleep(0.3)
    print("File → New song shows the start screen:", page.is_visible("#start"), "| back link:", page.is_visible("#start-back"))
    page.click("#start-back"); time.sleep(0.2); print("Back hides it:", page.is_hidden("#start"))
    b.close()
print("console errors:", errors or "none")
