"""Drag, add and remove pitch points and vibrato handles on the roll; check the saved expression."""
import json, pathlib, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT
errors = []

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift"); [page.mouse.wheel(0, 300) for _ in range(4)]; page.keyboard.up("Shift")
    page.keyboard.down("Control"); [page.mouse.wheel(0, -300) for _ in range(3)]; page.keyboard.up("Control"); time.sleep(0.4)
    pt = page.evaluate(FIND_NOTE); page.mouse.click(pt["x"], pt["y"]); time.sleep(0.3)
    s0 = save(page)
    def changed(s1):
        for n0, n1 in zip(s0["notes"], s1["notes"]):
            if n0.get("expr") != n1.get("expr"): return n1["onset_s"]
    print("canvas backing matches its size after selecting:", page.evaluate("(() => { const c = document.querySelector('#roll'); const r = c.getBoundingClientRect(); return [c.height, Math.round(r.height * devicePixelRatio)]; })()"))
    hs = page.evaluate(FIND_HANDLES); print("solid handles on the selected note:", len(hs))
    page.screenshot(path=str(OUT / "shots" / "H1-handles.png"))
    h = hs[-1]; page.mouse.move(h["x"], h["y"]); page.mouse.down(); page.mouse.move(h["x"] + 40, h["y"] - 25, steps=6); page.mouse.up(); time.sleep(0.2)
    s1 = save(page); onset = changed(s1); e1 = sel_expr(s1, onset)
    print("drag the last point (ms, tenths):", (15, 0), "→", [(q["x"], q["y"]) for q in e1["points"]][-1])
    hs = page.evaluate(FIND_HANDLES); z = hs[-1]
    page.mouse.dblclick(z["x"] - 45, z["y"] + 18); time.sleep(0.2)
    e2 = sel_expr(save(page), onset); print("double-click on the curve added a point:", len(e1["points"]), "→", len(e2["points"]), [(q["x"], q["y"]) for q in e2["points"]])
    hs = page.evaluate(FIND_HANDLES); target = hs[-2]
    page.keyboard.down("Alt"); page.mouse.click(target["x"], target["y"]); page.keyboard.up("Alt"); time.sleep(0.2)
    e3 = sel_expr(save(page), onset); print("alt-click removed a point:", len(e2["points"]), "→", len(e3["points"]))
    # vibrato on a long note, zoomed in so its handles separate
    page.mouse.click(1010, 448); time.sleep(0.3)
    page.mouse.move(20, 300)
    s_before_v = save(page)
    page.keyboard.press("v"); time.sleep(0.3)
    s_v = save(page)
    vonset = next(n1["onset_s"] for n0, n1 in zip(s_before_v["notes"], s_v["notes"]) if n0.get("expr") != n1.get("expr"))
    e4 = sel_expr(s_v, vonset); v0 = e4["vibrato"]
    hs = page.evaluate(FIND_HANDLES); print("solid handles once vibrato is on:", len(hs), [(round(q["x"]), round(q["y"])) for q in hs])
    page.mouse.move(20, 20); page.screenshot(path=str(OUT / "shots" / "H2-vibrato.png"))
    base_y = sorted(q["y"] for q in hs)[len(hs) // 2]
    tops = sorted([q for q in hs if q["y"] < base_y - 3], key=lambda q: q["x"])
    bottom = [q for q in hs if q["y"] > base_y + 3]
    def drag_by(q, dx, dy):
        page.mouse.move(q["x"], q["y"]); page.mouse.down(); page.mouse.move(q["x"] + dx, q["y"] + dy, steps=6); page.mouse.up(); time.sleep(0.2)
        return sel_expr(save(page), vonset)["vibrato"]
    v1 = drag_by(tops[-1], -30, -20); print("fade-out handle ← and ↑: depth", v0["depth"], "→", v1["depth"], "| fade-out", v0["out"], "→", v1["out"])
    hs = page.evaluate(FIND_HANDLES); tops = sorted([q for q in hs if q["y"] < base_y - 3], key=lambda q: q["x"])
    v2 = drag_by(tops[0], 25, 0); print("fade-in handle →: fade-in", v1["in"], "→", v2["in"], "| depth stays", v2["depth"])
    if bottom:
        hs = page.evaluate(FIND_HANDLES); bottom = [q for q in hs if q["y"] > base_y + 3]
        v3 = drag_by(bottom[0], 30, 0); print("period handle →: period", v2["period"], "→", v3["period"])
    page.click("#expr-exact-toggle"); page.fill("#vib-depth", "40")
    hs = page.evaluate(FIND_HANDLES); hs = [q for q in hs if q["y"] < base_y - 3 or q["y"] > base_y + 3]
    top = sorted(hs, key=lambda q: q["y"])[0]
    v4 = drag_by(top, 0, 0.2)
    print("typed depth 40 without Enter, then touched a handle: depth", v4["depth"], "(40 expected)")
    page.mouse.move(20, 20); page.screenshot(path=str(OUT / "shots" / "H3-vibrato-dragged.png"))
    b.close()
print("console errors:", errors or "none")
