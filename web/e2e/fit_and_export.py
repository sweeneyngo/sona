"""End-to-end: fit pitch, view the curve, edit one note, export UST/USTX, reload the session."""
import json, pathlib, re, time
from playwright.sync_api import sync_playwright

from _env import *  # noqa: F403
D = OUT
JSON, WAV = ANNOTATION, STEM
shots = OUT / "shots"
errors = []
FIND = """(minX) => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),sx=c.width/r.width,sy=c.height/r.height,d=g.getImageData(0,0,c.width,c.height).data;
 for(let x=Math.floor(c.width*minX);x<c.width*0.95;x+=3)for(let y=40;y<c.height-40;y+=2){const i=(y*c.width+x)*4,R=d[i],G=d[i+1],B=d[i+2];if(G>110&&R<170&&B<90&&G>R)return {x:r.left+x/sx+8,y:r.top+y/sy};} return null; }"""

def load(page, path):
    mclick(page, "#settings"); page.set_input_files("#json-input", str(path)); page.set_input_files("#audio-input", str(WAV))
    page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000); page.click("#settings-close")

def export(page, button, name):
    with page.expect_download(timeout=30000) as dl:
        mclick(page, button)
    out = D / name; dl.value.save_as(str(out)); return out


with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1600, "height": 900}, accept_downloads=True)
    page = ctx.new_page()
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    load(page, JSON); page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    mclick(page, "#fit-pitch"); fit = page.inner_text("#status"); check(fit.startswith("Pitch fitted"), "fit: " + fit)
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Shift"); [page.mouse.wheel(0, 300) for _ in range(4)]; page.keyboard.up("Shift"); time.sleep(0.4)
    page.screenshot(path=str(shots / "P1-curve.png"))
    pt = page.evaluate(FIND, 0.55); page.mouse.click(pt["x"], pt["y"]); time.sleep(0.3)
    print("bar visible:", page.is_visible("#expr-bar"), "| glide", page.input_value("#glide-start"), "→", page.input_value("#glide-end"), "| vibrato on:", page.is_checked("#vib-on"))
    was_on = page.is_checked("#vib-on")
    page.keyboard.press("v"); time.sleep(0.2)
    check(page.is_checked("#vib-on") != was_on, f"V toggles the vibrato: {was_on} → {page.is_checked('#vib-on')}")
    if not page.is_checked("#vib-on"):
        page.keyboard.press("v"); time.sleep(0.2)
    page.click("#expr-exact-toggle"); page.fill("#vib-depth", "60"); page.press("#vib-depth", "Enter"); page.fill("#vib-period", "160"); page.press("#vib-period", "Enter"); time.sleep(0.3)
    page.mouse.move(box["x"] + 30, box["y"] + 30); time.sleep(0.2)
    page.screenshot(path=str(shots / "P2-edited.png"))
    ust = export(page, "#export-ust", "p.ust"); ustx = export(page, "#export-ustx", "p.ustx"); sess = export(page, "#save", "p-session.json")
    page2 = ctx.new_page(); watch(page2, errors)
    page2.goto(EDITOR_URL); page2.evaluate("localStorage.clear()"); page2.reload(); load(page2, sess)
    ust2 = export(page2, "#export-ust", "p-reloaded.ust")
    check(ust.read_bytes() == ust2.read_bytes(), "UST identical after session reload")
    b.close()

t = ust.read_bytes().decode("cp932")
print("UST: PBS lines", len(re.findall(r"^PBS=", t, re.M)), "| VBR lines", len(re.findall(r"^VBR=", t, re.M)), "| a VBR:", re.search(r"^VBR=.*", t, re.M).group(0) if "VBR=" in t else None)
s = json.loads(sess.read_text())
edited = [n["expr"]["vibrato"] for n in s["notes"] if n.get("expr", {}).get("vibrato", {}) and n["expr"]["vibrato"]["depth"] == 60]
check(len(re.findall(r"^PBS=", t, re.M)) > 0 and "VBR=" in t, "the UST has pitch bends and vibratos")
check(len(edited) == 1 and edited[0]["period"] == 160, f"session notes with expr: {sum(1 for n in s['notes'] if n.get('expr'))} | the edited vibrato: {edited[:1]}")
finish(errors)
