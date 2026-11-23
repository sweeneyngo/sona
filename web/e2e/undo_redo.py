"""Undo/redo through the real UI: a note drag, a lyric, a pen stroke and a tempo burst are one step each;
Ctrl+Z in a text field is the field's own undo; reopening a file starts a fresh history."""
import json, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []

def notes_of(s):
    return [(round(n["onset_s"], 4), round(n["offset_s"], 4), n["midi_pitch"], n.get("lyric")) for n in s["notes"]]

def drawn(s):
    return sum(1 for f in s["f0_contour"] if (f[2] if isinstance(f, list) else f["voicing"]) == 1)

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    undo_off = lambda: page.evaluate("document.querySelector('#edit-undo').disabled")
    print("fresh file: Undo disabled", undo_off())
    s0 = save(page)

    page.click("#tempo-chip")
    for v in ["1", "13", "130"]:
        page.fill("#bpm", v)
    page.keyboard.press("Escape"); page.evaluate("document.activeElement.blur()")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift"); [page.mouse.wheel(0, 300) for _ in range(4)]; page.keyboard.up("Shift"); time.sleep(0.3)

    pt = page.evaluate(FIND_NOTE)
    page.mouse.move(pt["x"], pt["y"]); page.mouse.down()
    for k in range(1, 9):
        page.mouse.move(pt["x"] + 6 * k, pt["y"] - 4 * k)
    page.mouse.up(); time.sleep(0.2)
    s_drag = save(page)

    page.keyboard.press("l"); page.wait_for_selector("#lyric-editor:not([hidden])")
    page.keyboard.type("ら"); page.keyboard.press("Enter"); page.keyboard.press("Escape"); time.sleep(0.2)
    s_lyric = save(page)

    page.keyboard.press("p")
    page.mouse.move(box["x"] + 300, box["y"] + 330); page.mouse.down(); page.mouse.move(box["x"] + 500, box["y"] + 330, steps=10); page.mouse.up()
    page.keyboard.press("p"); time.sleep(0.2)
    s_pen = save(page)
    print("edits made: bpm", s_drag["session"]["bpm"], "| note moved", notes_of(s_drag) != notes_of(s0), "| lyric", notes_of(s_lyric) != notes_of(s_drag), "| pen frames", drawn(s_pen) - drawn(s_lyric))

    page.click("#expr-exact-toggle"); page.focus("#glide-start"); assert page.evaluate("document.activeElement.id") == "glide-start"; page.keyboard.press("Control+z")
    page.evaluate("document.activeElement.blur()"); page.click("#expr-exact-toggle")
    print("Ctrl+Z in a text field left the document alone:", drawn(save(page)) == drawn(s_pen))

    states = [s_pen, s_lyric, s_drag]
    labels = ["pen", "lyric", "drag"]
    for label, expect in zip(labels, states[1:] + [None]):
        page.keyboard.press("Control+z"); time.sleep(0.15)
        now = save(page)
        if label == "pen":
            print("undo pen stroke:", drawn(now) == drawn(s_lyric) and notes_of(now) == notes_of(s_lyric))
        elif label == "lyric":
            print("undo lyric:", notes_of(now) == notes_of(s_drag))
        else:
            print("undo drag:", notes_of(now) == notes_of(s0), "| bpm still", now["session"]["bpm"])
    page.keyboard.press("Control+z"); time.sleep(0.15)
    now = save(page)
    print("undo tempo burst (one step):", now["session"]["bpm"] == s0["session"]["bpm"], "| Undo now disabled:", undo_off())
    for _ in range(4):
        page.keyboard.press("Control+Shift+z"); time.sleep(0.15)
    now = save(page)
    print("redo all four:", notes_of(now) == notes_of(s_pen) and drawn(now) == drawn(s_pen) and now["session"]["bpm"] == 130)
    mclick(page, "#edit-undo"); time.sleep(0.15)
    print("Edit → Undo works:", drawn(save(page)) == drawn(s_lyric))

    # review cases: Ctrl+Z with focus on a select or the pitch bar
    page.click("#tempo-chip"); page.focus("#division"); page.select_option("#division", "8")
    page.keyboard.press("Control+z"); time.sleep(0.15)
    print("Ctrl+Z with the grid select focused undoes the grid change:", save(page)["session"]["division"] == 16)
    page.keyboard.press("Escape")
    pt = page.evaluate(FIND_NOTE); page.mouse.click(pt["x"], pt["y"]); time.sleep(0.2)
    page.click("#expr-vib"); time.sleep(0.1)
    page.keyboard.press("Control+z"); time.sleep(0.15)
    print("after undo from the pitch bar, it no longer shows a replaced note:", page.is_hidden("#expr-bar"))
    bad = OUT / "bad.json"; bad.write_text('{"version": 1}')
    page.mouse.click(pt["x"], pt["y"]); page.keyboard.press("Delete")
    mclick(page, "#settings"); page.set_input_files("#json-input", str(bad)); page.click("#settings-close"); time.sleep(0.2)
    print("a malformed file leaves no old history: Undo disabled", undo_off())
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close"); time.sleep(0.3)
    print("loading a file starts fresh: Undo disabled", undo_off(), "| Redo disabled", page.evaluate("document.querySelector('#edit-redo').disabled"))
    b.close()
print("console errors:", errors or "none")
