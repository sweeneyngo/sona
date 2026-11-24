"""Hover a point, scroll so the curve moves away, press Delete: the selected note goes, not a point that is no longer under the mouse."""
import time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []
points = lambda s: sum(len((n.get("expr") or {}).get("points", [])) for n in s["notes"])
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Control"); [page.mouse.wheel(0, -300) for _ in range(2)]; page.keyboard.up("Control"); time.sleep(0.4)
    note = next(n for n in page.evaluate(FIND_NOTES) if n["left"] > box["x"] + 60 and n["right"] < box["x"] + box["width"] - 10)
    page.mouse.click(note["x"], note["y"]); time.sleep(0.3)
    page.mouse.dblclick(note["left"] + note["w"] * 0.6, page.evaluate(FIND_HANDLES)[0]["y"]); time.sleep(0.3)
    s0 = save(page); n0, pts0 = len(s0["notes"]), points(s0)
    check(pts0 == 3, f"the selected note has three points: {pts0}")
    h = page.evaluate(FIND_HANDLES)[-1]; page.mouse.move(h["x"], h["y"]); time.sleep(0.2)
    page.keyboard.down("Shift"); page.mouse.wheel(0, 300); page.keyboard.up("Shift"); time.sleep(0.3)
    page.keyboard.press("Delete"); time.sleep(0.2)
    s1 = save(page); pts1 = points(s1)
    check(len(s1["notes"]) == n0 - 1 and pts1 == pts0 - 3, f"after scroll + Delete: notes {n0} → {len(s1['notes'])}, points {pts0} → {pts1} (the note goes with its 3 points)")
    b.close()
finish(errors)
