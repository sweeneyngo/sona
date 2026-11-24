"""Pen edits survive a lite autosave (storage quota hit), and the reload resumes them."""
import json, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; errors = []
QUOTA = """() => { const real = Storage.prototype.setItem; Storage.prototype.setItem = function (k, v) {
  if (v.length > (window.__quota || 1e12)) throw new DOMException('quota', 'QuotaExceededError'); return real.call(this, k, v); }; }"""
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); ctx = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True)
    ctx.add_init_script("(" + QUOTA + ")()")
    page = ctx.new_page(); watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    box = page.query_selector("#roll").bounding_box(); x, y = box["x"] + box["width"] * 0.3, box["y"] + box["height"] * 0.4
    page.evaluate("document.activeElement.blur()"); page.keyboard.press("p")
    page.mouse.move(x, y); page.mouse.down(); page.mouse.move(x + 100, y, steps=6); page.mouse.up()
    page.evaluate("window.dispatchEvent(new Event('beforeunload'))")
    size = page.evaluate("localStorage.getItem(Object.keys(localStorage)[0]).length")
    page.evaluate(f"window.__quota = {int(size * 0.6)}")
    print("full autosave chars:", size, "| quota now", int(size * 0.6))
    print("probe:", page.evaluate("(() => { try { localStorage.setItem('probe', 'y'.repeat(25e5)); localStorage.removeItem('probe'); return 'no throw'; } catch (e) { return e.name; } })()"), page.evaluate("String(Storage.prototype.setItem).slice(0, 60)"))
    page.mouse.move(x + 100, y); page.mouse.down(); page.mouse.move(x + 200, y, steps=6); page.mouse.up()
    page.evaluate("window.dispatchEvent(new Event('beforeunload'))"); time.sleep(0.2)
    rec = json.loads(page.evaluate("localStorage.getItem('" + page.evaluate("Object.keys(localStorage)[0]") + "')"))
    drawn = sum(1 for f in rec["file"]["f0_contour"] if (f[2] if isinstance(f, list) else f["voicing"]) == 1)
    check(rec.get("lite") and drawn > 0, f"autosave over the quota is lite and keeps the pen: lite {rec.get('lite')}, contour frames {len(rec['file']['f0_contour'])}, pen frames {drawn} | {page.inner_text('#status')}")
    page2 = ctx.new_page(); watch(page2, errors)
    page2.goto(EDITOR_URL); time.sleep(0.3)
    page2.click("#start-resume"); time.sleep(0.5)
    check(not page2.is_disabled("#pitch-pen") and not page2.evaluate("document.querySelector('#export-ustx-sung').disabled"), "resumed: the pen and the sung-pitch export are enabled")
    with page2.expect_download() as dl: mclick(page2, "#save")
    s = json.load(open(dl.value.path()))
    resumed = sum(1 for f in s["f0_contour"] if f["voicing"] == 1)
    check(resumed == drawn, f"the resumed session has the pen frames: {resumed} of {drawn}")
    b.close()
finish(errors)
