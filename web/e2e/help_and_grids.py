"""The Help describes the app as it is, every grid is offered, and a 1/16T session reopens on 1/16T."""
import time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; errors = []
OPTIONS = "(s) => Array.from(document.querySelector(s).options).map((o) => [o.value, o.textContent, o.selected])"
EXPORTS = "() => Array.from(document.querySelectorAll('#export-ustx, #export-ustx-sung, #export-ust, #export')).map((b) => b.firstChild.textContent.trim())"

def load(page, path):
    mclick(page, "#settings"); page.set_input_files("#json-input", str(path)); page.set_input_files("#audio-input", str(STEM))
    page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000); page.click("#settings-close")

def export(page, button, name):
    with page.expect_download(timeout=30000) as dl:
        mclick(page, button)
    out = D / name; dl.value.save_as(str(out)); return out

with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True)
    page = ctx.new_page(); watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.2)

    page.click("#help"); help_text = page.inner_text("#help-overlay .modal-body"); page.keyboard.press("Escape")
    exports = page.evaluate(EXPORTS)
    check(all(e in help_text for e in exports), f"the Help names every export: {exports}")
    for phrase in ["type its BPM", "Shift-click", "At playhead", "Bar 1 from the music", "says what and how to add it"]:
        check(phrase in help_text, f"the Help says {phrase!r}")
    check("suggests its tempo" not in help_text, "the Help no longer has sona suggest the tempo")

    grids = page.evaluate(OPTIONS, "#division"); jobs = page.evaluate(OPTIONS, "#song-grid")
    check(grids == [["4", "1/4", False], ["8", "1/8", False], ["16", "1/16", True], ["24", "1/16T", False], ["32", "1/32", False]], f"grid menu: {grids}")
    check(jobs == [["16", "1/16", True], ["24", "1/16T", False]], f"job grid menu: {jobs}")

    load(page, ANNOTATION)
    page.click("#tempo-chip"); page.select_option("#division", "24"); page.keyboard.press("Escape"); time.sleep(0.2)
    ust = export(page, "#export-ust", "g24.ust"); sess = export(page, "#save", "g24-session.json")
    page2 = ctx.new_page(); watch(page2, errors)
    page2.goto(EDITOR_URL); page2.evaluate("localStorage.clear()"); page2.reload(); load(page2, sess)
    check(page2.input_value("#division") == "24" and "1/16T" in page2.inner_text("#tempo-chip-text"), f"a 1/16T session reopens on 1/16T: {page2.inner_text('#tempo-chip-text')}")
    ust2 = export(page2, "#export-ust", "g24-reloaded.ust")
    check(ust.read_bytes() == ust2.read_bytes(), "the 1/16T UST is identical after the session reload")
    b.close()
finish(errors)
