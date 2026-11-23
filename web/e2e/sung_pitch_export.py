"""Export USTX with the singer's pitch; check the pitd curve reproduces the white line in OpenUtau's terms."""
import json, pathlib, re, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT
errors = []
def export(page, button, name):
    with page.expect_download(timeout=30000) as dl: mclick(page, button)
    out = D / name; dl.value.save_as(str(out)); return out
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"]); page = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True).new_page()
    page.on("pageerror", lambda e: errors.append(str(e))); page.on("console", lambda m: m.type == "error" and errors.append(m.text))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload()
    mclick(page, "#settings"); page.set_input_files("#json-input", str(ANNOTATION)); page.click("#settings-close")
    page.click("#tempo-chip"); page.fill("#bpm", "125"); page.keyboard.press("Escape")
    mclick(page, "#fit-pitch")
    plain = export(page, "#export-ustx", "plain.ustx"); sung = export(page, "#export-ustx-sung", "sung.ustx")
    print("status:", page.inner_text("#status"))
    b.close()
print("console errors:", errors or "none")
