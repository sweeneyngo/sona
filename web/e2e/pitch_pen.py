"""Pitch pen: draw over the white line, see it in the session and the sung-pitch USTX; erase it."""
import json, math, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; errors = []
def download(page, sel):
    with page.expect_download(timeout=30000) as dl: mclick(page, sel)
    return open(dl.value.path(), encoding="utf-8").read()
cents = lambda hz: 1200 * math.log2(hz / 440) + 6900
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Control"); [page.mouse.wheel(0, -300) for _ in range(2)]; page.keyboard.up("Control"); time.sleep(0.3)
    note = next(n for n in page.evaluate(FIND_NOTES) if n["left"] > box["x"] + 60 and n["right"] < box["x"] + box["width"] - 10)
    page.evaluate("document.activeElement.blur()"); page.keyboard.press("p")
    check(page.is_checked("#pitch-pen"), "P turns the pen on: " + page.inner_text("#status"))
    y, x0, x1 = note["y"] - note["h"] - 2, note["left"] + 10, note["right"] - 10
    page.mouse.move(x0, y); page.mouse.down(); page.mouse.move(x1, y, steps=12); page.mouse.up(); time.sleep(0.2)
    page.mouse.move(5, 5); page.screenshot(path=str(OUT / "shots" / "N1-pen.png"))
    sess = json.loads(download(page, "#save"))
    drawn = [f for f in sess["f0_contour"] if f["voicing"] == 1]
    t0, t1 = drawn[0]["time_s"], drawn[-1]["time_s"]
    under = next(n for n in sess["notes"] if n["onset_s"] <= (t0 + t1) / 2 < n["offset_s"])
    cs = [cents(f["frequency_hz"]) for f in drawn]
    check(len(drawn) > 10 and under["onset_s"] <= t0 and t1 <= under["offset_s"] and all(abs(c - (under["midi_pitch"] + 1) * 100) < 30 for c in cs),
          f"a stroke one row above note {under['midi_pitch']} draws {len(drawn)} frames inside it at {round(min(cs))}…{round(max(cs))} cents")
    sung = download(page, "#export-ustx-sung")
    check("pitd" in sung, "the sung-pitch USTX carries a pitd curve")
    xm = (x0 + x1) / 2
    page.keyboard.down("Alt"); page.mouse.move(xm - 20, y); page.mouse.down(); page.mouse.move(xm + 20, y, steps=5); page.mouse.up(); page.keyboard.up("Alt"); time.sleep(0.2)
    sess2 = json.loads(download(page, "#save"))
    left = sum(1 for f in sess2["f0_contour"] if f["voicing"] == 1)
    erased = sum(1 for f in sess2["f0_contour"] if f["voicing"] == 0 and t0 <= f["time_s"] <= t1)
    check(erased > 0 and left == len(drawn) - erased, f"Alt-drag erases the middle: drawn frames {len(drawn)} → {left}, erased {erased}")
    page.mouse.move(5, 5); page.screenshot(path=str(OUT / "shots" / "N2-erased.png"))
    page.keyboard.press("p"); check(not page.is_checked("#pitch-pen"), "P turns the pen off")
    b.close()
open(D / "pen-sung.ustx", "w").write(sung)
finish(errors)
