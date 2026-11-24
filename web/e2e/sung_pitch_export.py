"""Export USTX with the singer's pitch; check the pitd curve reproduces the white line in OpenUtau's terms."""
import re
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT
errors = []
def export(page, button, name):
    with page.expect_download(timeout=30000) as dl: mclick(page, button)
    out = D / name; dl.value.save_as(str(out)); return out
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    mclick(page, "#fit-pitch")
    plain = export(page, "#export-ustx", "plain.ustx"); sung = export(page, "#export-ustx-sung", "sung.ustx")
    print("status:", page.inner_text("#status"))
    b.close()
plain_t, sung_t = plain.read_text(), sung.read_text()
curve = re.search(r"^    curves:\n((?:      .*\n)+)", sung_t, re.M)
check(curve is not None and 'abbr: "pitd"' in curve.group(1) and curve.group(1).count("\n") > 100, "the sung USTX has a pitd curve")
check(curve is not None and sung_t.replace(curve.group(0), "    curves: []\n") == plain_t, "apart from that curve it equals the plain USTX")
finish(errors)
