"""Motion: things animate in, and not at all under prefers-reduced-motion."""
import time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
D = OUT; errors = []
ANIM = "(s) => { const e = document.querySelector(s); return e ? getComputedStyle(e).animationName : null; }"
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    for motion in ["no-preference", "reduce"]:
        page = b.new_context(viewport={"width": 1500, "height": 860}, reduced_motion=motion).new_page()
        watch(page, errors)
        page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.2)
        start = page.evaluate(ANIM, ".start-inner")
        page.click(".menu-btn"); menu = page.evaluate(ANIM, ".menu-list:not([hidden])"); page.keyboard.press("Escape")
        page.click("#help"); modal = page.evaluate(ANIM, ".overlay:not([hidden]) .modal")
        page.screenshot(path=str(OUT / "shots" / f"M-{motion}-help-mid.png")); page.keyboard.press("Escape")
        names = [start, menu, modal]
        check(all(n == "none" for n in names) if motion == "reduce" else all(n and n != "none" for n in names), f"{motion:14} start card: {start:10} menu: {menu:10} dialog: {modal}")
    b.close()
finish(errors)
