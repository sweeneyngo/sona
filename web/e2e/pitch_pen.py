"""Pitch pen: draw over the white line, see it in the session and the sung-pitch USTX; erase it."""
import json, math, pathlib, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; errors = []
def download(page, sel):
    with page.expect_download(timeout=30000) as dl: mclick(page, sel)
    return open(dl.value.path(), encoding="utf-8").read()
cents = lambda hz: 1200 * math.log2(hz / 440) + 6900
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift"); [page.mouse.wheel(0, 300) for _ in range(4)]; page.keyboard.up("Shift"); time.sleep(0.3)
    page.evaluate("document.activeElement.blur()"); page.keyboard.press("p"); print("pen on:", page.is_checked("#pitch-pen"), "|", page.inner_text("#status"))
    # a straight stroke one row above F4's centre (F#4 centre = y 399 in the earlier screenshots)
    y = 399; x0, x1 = 930, 1090
    page.mouse.move(x0, y); page.mouse.down(); page.mouse.move(x1, y, steps=12); page.mouse.up(); time.sleep(0.2)
    page.mouse.move(20, 20); page.screenshot(path=str(OUT / "shots" / "N1-pen.png"))
    sess = json.loads(download(page, "#save"))
    t = lambda x: page.evaluate("""(x) => { const r = document.querySelector('#roll').getBoundingClientRect(); return x - r.left; }""", x)
    drawn = [f for f in sess["f0_contour"] if f["voicing"] == 1]
    print("frames drawn:", len(drawn), "| their pitch (cents):", round(min(cents(f["frequency_hz"]) for f in drawn)), "…", round(max(cents(f["frequency_hz"]) for f in drawn)), "(F#4 = 6600)")
    sung = download(page, "#export-ustx-sung")
    t0, t1 = drawn[0]["time_s"], drawn[-1]["time_s"]
    # Alt-drag erases the middle of it
    page.keyboard.down("Alt"); page.mouse.move(990, 300); page.mouse.down(); page.mouse.move(1030, 300, steps=5); page.mouse.up(); page.keyboard.up("Alt"); time.sleep(0.2)
    sess2 = json.loads(download(page, "#save"))
    print("after Alt-drag: drawn frames", len(drawn), "→", sum(1 for f in sess2["f0_contour"] if f["voicing"] == 1), "| erased frames:", sum(1 for f in sess2["f0_contour"] if f["voicing"] == 0 and t0 <= f["time_s"] <= t1))
    page.mouse.move(20, 20); page.screenshot(path=str(OUT / "shots" / "N2-erased.png"))
    page.keyboard.press("p"); print("pen off:", not page.is_checked("#pitch-pen"))
    b.close()
open(D / "pen-sung.ustx", "w").write(sung)
print("drawn span s:", round(t0, 3), "→", round(t1, 3)); print("console errors:", errors or "none")
