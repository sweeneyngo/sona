"""End-to-end: lyrics + UST/USTX export in sona's review surface."""
import time, pathlib, json
from playwright.sync_api import sync_playwright

from _env import *  # noqa: F403
D = OUT
JSON, WAV = ANNOTATION, STEM
shots = OUT / "shots"; shots.mkdir(exist_ok=True)
LYRICS = "きらきら ひかる おそらの ほしよ\nまばたき しては みんなを みてる\nきらきら ひかる おそらの ほしよ"
errors = []

FIND_NOTE = """() => {
  const c = document.querySelector('#roll'); const g = c.getContext('2d');
  const r = c.getBoundingClientRect(); const sx = c.width / r.width, sy = c.height / r.height;
  const d = g.getImageData(0, 0, c.width, c.height).data;
  for (let x = Math.floor(c.width * 0.35); x < c.width * 0.9; x += 3)
    for (let y = 40; y < c.height - 40; y += 2) {
      const i = (y * c.width + x) * 4, R = d[i], G = d[i + 1], B = d[i + 2];
      if (G > 110 && R < 170 && B < 90 && G > R) return { x: r.left + x / sx, y: r.top + y / sy };
    }
  return null;
}"""

def load(page, json_path):
    mclick(page, "#settings")
    page.set_input_files("#json-input", str(json_path))
    page.set_input_files("#audio-input", str(WAV))
    page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000)
    page.click("#settings-close")

def export(page, button, name):
    with page.expect_download(timeout=30000) as dl:
        mclick(page, button)
    out = D / name
    dl.value.save_as(str(out))
    return out


with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1600, "height": 900}, accept_downloads=True)
    page = ctx.new_page()
    watch(page, errors)
    page.goto(EDITOR_URL)
    page.evaluate("localStorage.clear()")
    page.reload()
    load(page, JSON)
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")

    # 1. paste lyrics and fill from the first note
    mclick(page, "#lyrics")
    page.fill("#lyrics-text", LYRICS)
    print("preview:", page.inner_text("#lyrics-preview"))
    page.screenshot(path=str(shots / "L1-dialog.png"))
    page.click("#lyrics-from-start")
    print("status:", page.inner_text("#status"))

    # pan to the first sung notes (~11 s) and zoom in a little
    box = page.query_selector("#roll").bounding_box()
    page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift")
    for _ in range(4): page.mouse.wheel(0, 300); time.sleep(0.1)
    page.keyboard.up("Shift"); time.sleep(0.5)
    page.screenshot(path=str(shots / "L2-lyrics-on-notes.png"))

    # 2. select a note, press L, type two lyrics with Enter
    pt = page.evaluate(FIND_NOTE)
    print("clicking note at", pt)
    page.mouse.click(pt["x"], pt["y"]); time.sleep(0.2)
    page.keyboard.press("l")
    page.wait_for_selector("#lyric-editor:not([hidden])", timeout=5000)
    page.keyboard.type("テスト"); page.screenshot(path=str(shots / "L3-inline-editor.png"))
    page.keyboard.press("Enter")
    page.keyboard.type("ら"); page.keyboard.press("Enter")
    page.keyboard.press("Escape"); time.sleep(0.3)
    page.screenshot(path=str(shots / "L4-after-typing.png"))

    # 3. exports
    ust = export(page, "#export-ust", "e2e.ust"); print("UST status:", page.inner_text("#status"))
    ustx = export(page, "#export-ustx", "e2e.ustx"); print("USTX status:", page.inner_text("#status"))
    sess = export(page, "#save", "e2e-session.json")

    # 4. reload the saved session in a fresh page: lyrics must survive
    page2 = ctx.new_page()
    watch(page2, errors)
    page2.goto(EDITOR_URL)
    page2.evaluate("localStorage.clear()")
    page2.reload()
    load(page2, sess)
    ust2 = export(page2, "#export-ust", "e2e-reloaded.ust")
    check(ust.read_bytes() == ust2.read_bytes(), "UST identical after session reload")
    b.close()

s = json.loads(sess.read_text())
lyrics = [n.get("lyric") for n in s["notes"]]
check(sum(1 for l in lyrics if l) >= 40 and "テスト" in lyrics and "ら" in lyrics, f"session notes with lyrics: {sum(1 for l in lyrics if l)} of {len(lyrics)}, typed テスト and ら among them")
check("Lyric=テスト" in ust.read_bytes().decode("cp932"), "the UST carries the typed lyric")
finish(errors)
