"""Hover a point, scroll so the curve moves away, press Delete: the selected note goes, not a point that is no longer under the mouse."""
import json, pathlib, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT
errors = []
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift"); [page.mouse.wheel(0, 300) for _ in range(4)]; page.keyboard.up("Shift")
    page.keyboard.down("Control"); [page.mouse.wheel(0, -300) for _ in range(3)]; page.keyboard.up("Control"); time.sleep(0.4)
    pt = page.evaluate(FIND_NOTE); page.mouse.click(pt["x"], pt["y"]); time.sleep(0.3)
    hs = page.evaluate(FIND_HANDLES); h = hs[-1]
    page.mouse.move(h["x"], h["y"]); page.mouse.down(); page.mouse.move(h["x"] + 40, h["y"] - 25, steps=6); page.mouse.up(); time.sleep(0.2)
    hs = page.evaluate(FIND_HANDLES); z = hs[-1]
    page.mouse.dblclick(z["x"] - 45, z["y"] + 18); time.sleep(0.3)   # now 3 points
    s0 = save(page); n0 = len(s0["notes"]); pts0 = sum(len((n.get("expr") or {}).get("points", [])) for n in s0["notes"])
    hs = page.evaluate(FIND_HANDLES); h = hs[-1]; page.mouse.move(h["x"], h["y"]); time.sleep(0.2)
    print("points on the edited note before:", max(len((n.get("expr") or {}).get("points", [])) for n in s0["notes"]))
    page.keyboard.down("Shift"); page.mouse.wheel(0, 300); page.keyboard.up("Shift"); time.sleep(0.3)    # time pan: the curve moves, the mouse doesn't
    page.keyboard.press("Delete"); time.sleep(0.2)
    s1 = save(page); pts1 = sum(len((n.get("expr") or {}).get("points", [])) for n in s1["notes"])
    print("after scroll + Delete: notes", n0, "→", len(s1["notes"]), "| points", pts0, "→", pts1, "(expected: note deleted, its 3 points with it, no other point removed)")
    b.close()
print("console errors:", errors or "none")
