"""Drag, add and remove pitch points and vibrato handles on the roll; check the saved expression.

Notes and handles are found on the canvas (note bars and handle blobs by colour), never at fixed positions."""
import time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
errors = []


def changed(s0, s1):
    return [n1["onset_s"] for n0, n1 in zip(s0["notes"], s1["notes"]) if n0.get("expr") != n1.get("expr")]


with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Control"); [page.mouse.wheel(0, -300) for _ in range(2)]; page.keyboard.up("Control"); time.sleep(0.4)
    for _ in range(20):
        whole = [n for n in page.evaluate(FIND_NOTES) if n["left"] > box["x"] + 60 and n["right"] < box["x"] + box["width"] - 10 and n["w"] >= 100]
        if len(whole) >= 2:
            break
        page.keyboard.down("Shift"); page.mouse.wheel(0, 200); page.keyboard.up("Shift"); time.sleep(0.1)
    check(len(whole) >= 2, f"two notes at least 100 px wide in view: {[round(n['w']) for n in whole]}")
    long_note, note = whole[0], whole[1]

    page.mouse.click(note["x"], note["y"]); time.sleep(0.3)
    backing = page.evaluate("(() => { const c = document.querySelector('#roll'); const r = c.getBoundingClientRect(); return [c.height, Math.round(r.height * devicePixelRatio)]; })()")
    check(backing[0] == backing[1], f"canvas backing matches its size after selecting: {backing}")
    hs = page.evaluate(FIND_HANDLES)
    check(len(hs) == 1, f"one solid handle on the selected note (the first point is snapped): {len(hs)}")
    row_y = hs[0]["y"]
    page.screenshot(path=str(OUT / "shots" / "H1-handles.png"))
    s0 = save(page)

    page.mouse.dblclick(note["left"] + note["w"] * 0.6, row_y); time.sleep(0.2)
    s1 = save(page); on = changed(s0, s1)
    e1 = sel_expr(s1, on[0]) if len(on) == 1 else {"points": []}
    check(len(s1["notes"]) == len(s0["notes"]) and len(e1["points"]) == 3 and abs(e1["points"][-1]["y"]) <= 3,
          f"double-click on the curve adds a point there, not a note: {[(q['x'], q['y']) for q in e1['points']]}")

    hs = page.evaluate(FIND_HANDLES); h = hs[-1]
    page.mouse.move(h["x"], h["y"]); page.mouse.down(); page.mouse.move(h["x"] + 30, h["y"] - 25, steps=6); page.mouse.up(); time.sleep(0.2)
    e2 = sel_expr(save(page), on[0]); a, z = e1["points"][-1], e2["points"][-1]
    check(z["x"] > a["x"] and z["y"] > a["y"] + 5, f"drag the new point → and ↑: {(a['x'], a['y'])} → {(z['x'], z['y'])}")

    hs = page.evaluate(FIND_HANDLES)
    page.keyboard.down("Alt"); page.mouse.click(hs[-2]["x"], hs[-2]["y"]); page.keyboard.up("Alt"); time.sleep(0.2)
    e3 = sel_expr(save(page), on[0])
    check([(q["x"], q["y"]) for q in e3["points"]] == [(e2["points"][0]["x"], e2["points"][0]["y"]), (z["x"], z["y"])],
          f"alt-click removed the middle point: {len(e2['points'])} → {len(e3['points'])}")

    long_note = max((n for n in page.evaluate(FIND_NOTES) if abs(n["left"] - long_note["left"]) < 3), key=lambda n: n["w"])
    page.mouse.click(long_note["x"], long_note["y"]); time.sleep(0.3)
    row_y = page.evaluate(FIND_HANDLES)[0]["y"]
    page.mouse.move(5, 5)
    s_before = save(page); page.keyboard.press("v"); time.sleep(0.3); s_v = save(page)
    von = changed(s_before, s_v)
    v0 = sel_expr(s_v, von[0])["vibrato"] if len(von) == 1 else None
    check(v0 is not None and v0["length"] == 75, f"V adds the default vibrato to the selected note: {v0}")
    vonset = von[0]

    def vib_handles():
        hs = page.evaluate(FIND_HANDLES)
        tops = sorted([q for q in hs if q["y"] < row_y - 2], key=lambda q: q["x"])
        base = sorted([q for q in hs if abs(q["y"] - row_y) <= 2], key=lambda q: q["x"])
        bottom = [q for q in hs if q["y"] > row_y + 2]
        return tops, base, bottom

    def drag_by(q, dx, dy):
        x, y = round(q["x"]), round(q["y"])
        page.mouse.move(x, y); page.mouse.down(); page.mouse.move(x + dx, y + dy, steps=6); page.mouse.up(); time.sleep(0.2)
        return sel_expr(save(page), vonset)["vibrato"]

    tops, base, bottom = vib_handles()
    print("vibrato handles (top, base, bottom):", [[(round(q["x"]), round(q["y"])) for q in g] for g in (tops, base, bottom)])
    page.mouse.move(5, 5); page.screenshot(path=str(OUT / "shots" / "H2-vibrato.png"))
    check(len(tops) == 2 and len(bottom) == 1 and len(base) == 2, f"fade-in and fade-out above the note, period below, start on it: {len(tops)}, {len(bottom)}, {len(base)}")

    v1 = drag_by(tops[-1], -30, -20)
    check(v1["depth"] > v0["depth"] and v1["out"] > v0["out"], f"fade-out handle ← and ↑: depth {v0['depth']} → {v1['depth']}, fade-out {v0['out']} → {v1['out']}")
    tops, base, bottom = vib_handles()
    v2 = drag_by(tops[0], 25, 0)
    check(v2["in"] > v1["in"] and v2["depth"] == v1["depth"], f"fade-in handle →: fade-in {v1['in']} → {v2['in']}, depth stays {v2['depth']}")
    tops, base, bottom = vib_handles()
    v3 = drag_by(bottom[0], 30, 0)
    check(v3["period"] > v2["period"], f"period handle →: period {v2['period']} → {v3['period']}")
    tops, base, bottom = vib_handles()
    v4 = drag_by(base[-1], -30, 0)
    check(v4["length"] > v3["length"], f"start handle ←: length {v3['length']} → {v4['length']}")

    page.click("#expr-exact-toggle"); page.fill("#vib-depth", "40")
    tops, base, bottom = vib_handles()
    v5 = drag_by(min(tops, key=lambda q: q["y"]), 0, 0)
    check(v5["depth"] == 40, f"typed depth 40 without Enter, then touched a handle: depth {v5['depth']}")
    page.mouse.move(5, 5); page.screenshot(path=str(OUT / "shots" / "H3-vibrato-dragged.png"))
    b.close()
finish(errors)
