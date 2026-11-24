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
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    undo_off = lambda: page.evaluate("document.querySelector('#edit-undo').disabled")
    check(undo_off(), "fresh file: Undo disabled")
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
    check(s_drag["session"]["bpm"] == 130 and notes_of(s_drag) != notes_of(s0) and notes_of(s_lyric) != notes_of(s_drag) and drawn(s_pen) > drawn(s_lyric),
          f"edits made: bpm {s_drag['session']['bpm']}, note moved, lyric, pen frames {drawn(s_pen) - drawn(s_lyric)}")

    page.click("#expr-exact-toggle"); page.focus("#glide-start"); assert page.evaluate("document.activeElement.id") == "glide-start"; page.keyboard.press("Control+z")
    page.evaluate("document.activeElement.blur()"); page.click("#expr-exact-toggle")
    check(drawn(save(page)) == drawn(s_pen), "Ctrl+Z in a text field left the document alone")

    states = [s_pen, s_lyric, s_drag]
    labels = ["pen", "lyric", "drag"]
    for label, expect in zip(labels, states[1:] + [None]):
        page.keyboard.press("Control+z"); time.sleep(0.15)
        now = save(page)
        if label == "pen":
            check(drawn(now) == drawn(s_lyric) and notes_of(now) == notes_of(s_lyric), "undo pen stroke")
        elif label == "lyric":
            check(notes_of(now) == notes_of(s_drag), "undo lyric")
        else:
            check(notes_of(now) == notes_of(s0) and now["session"]["bpm"] == 130, f"undo drag, bpm still {now['session']['bpm']}")
    page.keyboard.press("Control+z"); time.sleep(0.15)
    now = save(page)
    check(now["session"]["bpm"] == s0["session"]["bpm"] and undo_off(), "undo tempo burst (one step), Undo now disabled")
    for _ in range(4):
        page.keyboard.press("Control+Shift+z"); time.sleep(0.15)
    now = save(page)
    check(notes_of(now) == notes_of(s_pen) and drawn(now) == drawn(s_pen) and now["session"]["bpm"] == 130, "redo all four")
    mclick(page, "#edit-undo"); time.sleep(0.15)
    check(drawn(save(page)) == drawn(s_lyric), "Edit → Undo works")

    # review cases: Ctrl+Z with focus on a select or the pitch bar
    page.click("#tempo-chip"); page.focus("#division"); page.select_option("#division", "8")
    page.keyboard.press("Control+z"); time.sleep(0.15)
    check(save(page)["session"]["division"] == 16, "Ctrl+Z with the grid select focused undoes the grid change")
    page.keyboard.press("Escape")
    pt = page.evaluate(FIND_NOTE); page.mouse.click(pt["x"], pt["y"]); time.sleep(0.2)
    page.click("#expr-vib"); time.sleep(0.1)
    page.keyboard.press("Control+z"); time.sleep(0.15)
    check(page.is_hidden("#expr-bar"), "after undo from the pitch bar, it no longer shows a replaced note")
    bad = OUT / "bad.json"; bad.write_text('{"version": 1}')
    page.mouse.click(pt["x"], pt["y"]); page.keyboard.press("Delete")
    mclick(page, "#settings"); page.set_input_files("#json-input", str(bad)); page.click("#settings-close"); time.sleep(0.2)
    check(undo_off(), "a malformed file leaves no old history: Undo disabled")
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close"); time.sleep(0.3)
    check(undo_off() and page.evaluate("document.querySelector('#edit-redo').disabled"), "loading a file starts fresh: Undo and Redo disabled")
    b.close()
finish(errors)
