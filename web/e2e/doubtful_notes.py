"""A learned-stage result outlines its doubtful 20% and says why; the share is a setting; a fixed note loses its
outline; a pYIN result keeps the threshold and the weakest sub-score."""
import json, time
from playwright.sync_api import sync_playwright
from _env import *  # noqa: F403
from fixture import synthetic
ANNOTATION, STEM = synthetic(OUT)  # always the fixture: the checks count its notes
errors = []

# Outlined notes: horizontal runs of the flag red (#ff5c5c); runs that share most of their span are one note's top and
# bottom edge, and a piece the contour cut off overlaps its note's box. One voice, so two outlined boxes never overlap.
OUTLINED = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),sx=c.width/r.width,sy=c.height/r.height,W=c.width,H=c.height,d=g.getImageData(0,0,W,H).data;
 const red=(x,y)=>{const i=(y*W+x)*4;return d[i]>200&&d[i+1]<130&&d[i+2]<130&&d[i]-d[i+1]>100;}; const gs=[];
 const same=(q,x0,x1)=>Math.min(q.x1,x1)-Math.max(q.x0,x0)>0.5*Math.min(q.x1-q.x0,x1-x0);
 for(let y=0;y<H;y++)for(let x=0;x<W;){if(!red(x,y)){x++;continue;}let x1=x;while(x1<W&&red(x1,y))x1++;
  if(x1-x>=4*sx){const m=gs.find(q=>same(q,x,x1));if(m){m.x0=Math.min(m.x0,x);m.x1=Math.max(m.x1,x1);m.y0=Math.min(m.y0,y);m.y1=Math.max(m.y1,y);}else gs.push({x0:x,x1,y0:y,y1:y});} x=x1;}
 const touch=(a,b)=>same(a,b.x0,b.x1)||(Math.min(a.x1,b.x1)>=Math.max(a.x0,b.x0)&&Math.min(a.y1,b.y1)>=Math.max(a.y0,b.y0));
 for(let i=0;i<gs.length;i++)for(let j=i+1;j<gs.length;j++)if(touch(gs[i],gs[j])){const a=gs[i],b=gs[j];a.x0=Math.min(a.x0,b.x0);a.x1=Math.max(a.x1,b.x1);a.y0=Math.min(a.y0,b.y0);a.y1=Math.max(a.y1,b.y1);gs.splice(j,1);j=i;}
 return gs.filter(q=>q.y1-q.y0>=3*sy).map(q=>({x:r.left+(q.x0+q.x1)/2/sx,y:r.top+(q.y0+q.y1)/2/sy,w:(q.x1-q.x0)/sx})).sort((a,b)=>a.x-b.x); }"""

LONG, UNSURE = "long note, may hide several syllables", "the model was unsure here"


def stage_annotation():
    """The synthetic fixture as a learned-stage result: the eight held notes and two short ones are the least sure."""
    a = json.loads(ANNOTATION.read_text())
    unsure = {3, 27}
    for i, n in enumerate(a["notes"]):
        held = n["offset_s"] - n["onset_s"] > 1.5
        if held:
            n["confidence"], n["stage_doubt"] = 0.30 + 0.005 * i / 6, {"length": 1.6, "unsure": 0.3}
        elif i in unsure:
            n["confidence"], n["stage_doubt"] = 0.35, {"length": 0.4, "unsure": 1.9}
        else:
            n["confidence"], n["stage_doubt"] = 0.6 + 0.3 * ((i * 7) % 11) / 11, {"length": 0.4, "unsure": 0.6}
    a["producer"] = "game"
    a["provenance"] = {"sona_version": "0.1.0", "sona_commit": None, "created_unix_s": 0, "settings": {"engine": "game+cells", "bpm": "125", "grid": "16"},
                       "input": {"role": "input", "name": "lead.wav", "bytes": 1, "sha256": "0" * 64}, "models": []}
    path = OUT / "stage.annotation.json"; path.write_text(json.dumps(a)); return path, len(a["notes"])


def load(page, path):
    mclick(page, "#settings"); page.set_input_files("#json-input", str(path)); page.set_input_files("#audio-input", str(STEM))
    page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000)


def whole_song(page):
    box = page.query_selector("#roll").bounding_box(); page.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2)
    page.keyboard.down("Control"); [page.mouse.wheel(0, 400) for _ in range(6)]; page.keyboard.up("Control")
    page.mouse.move(box["x"] + 5, box["y"] + 5); time.sleep(0.4)


def why(page, o):
    page.mouse.move(o["x"], o["y"]); time.sleep(0.15)
    return page.inner_text("#tooltip")


def set_range(page, sel, value):
    mclick(page, "#settings"); page.fill(sel, str(value)); page.click("#settings-close"); time.sleep(0.3)


with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True)
    page = ctx.new_page(); watch(page, errors)
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload(); time.sleep(0.2)

    stage, n = stage_annotation()
    load(page, stage)
    check(page.is_visible("#flag-share") and not page.is_visible("#flag"), "a stage result offers the share, not the threshold")
    check(page.inner_text("#flag-share-val") == "20%", f"the share starts at 20%: {page.inner_text('#flag-share-val')}")
    page.click("#settings-close"); whole_song(page)

    out = page.evaluate(OUTLINED)
    check(len(out) == round(0.2 * n), f"20% of {n} notes outlined: {len(out)}")
    reasons = [why(page, o) for o in out]
    longs, unsures = sum(LONG in r for r in reasons), sum(UNSURE in r for r in reasons)
    check(longs == 8 and unsures == 2, f"the eight held notes say '{LONG}', the two unsure ones '{UNSURE}': {longs}, {unsures}")
    check(not any("uncertain offset" in r or "soft onset" in r for r in reasons), "no stage note names a sub-score")
    held = next(o for o, r in zip(out, reasons) if LONG in r)
    why(page, held); page.screenshot(path=str(OUT / "shots" / "doubtful_notes.png"))
    print("screenshot:", OUT / "shots" / "doubtful_notes.png")

    set_range(page, "#flag-share", 0); whole_song(page)
    check(len(page.evaluate(OUTLINED)) == 0, "a 0% share outlines nothing")
    set_range(page, "#flag-share", 30); whole_song(page)
    check(len(page.evaluate(OUTLINED)) == round(0.3 * n), f"a 30% share outlines {round(0.3 * n)}: {len(page.evaluate(OUTLINED))}")
    set_range(page, "#flag-share", 20); whole_song(page)

    held = next(o for o in page.evaluate(OUTLINED) if LONG in why(page, o))
    page.mouse.move(held["x"], held["y"]); page.mouse.down(); page.mouse.move(held["x"], held["y"] - 40, steps=6); page.mouse.up(); time.sleep(0.3)
    page.keyboard.press("Escape"); whole_song(page)
    check(len(page.evaluate(OUTLINED)) == round(0.2 * n) - 1, f"a dragged note loses its outline and none takes its place: {len(page.evaluate(OUTLINED))}")

    page2 = ctx.new_page(); watch(page2, errors)
    page2.goto(EDITOR_URL); page2.evaluate("localStorage.clear()"); page2.reload(); time.sleep(0.2)
    load(page2, ANNOTATION)
    check(page2.is_visible("#flag") and not page2.is_visible("#flag-share"), "a pYIN result offers the threshold, not the share")
    page2.click("#settings-close"); whole_song(page2)
    check(len(page2.evaluate(OUTLINED)) == 0, "at 0.50 no pYIN note (all 0.92) is outlined")
    set_range(page2, "#flag", 0.95); whole_song(page2)
    pyin = page2.evaluate(OUTLINED)
    check(len(pyin) == n, f"at 0.95 every pYIN note is outlined: {len(pyin)} of {n}")
    tip = why(page2, pyin[0])
    check("soft onset (0.90)" in tip and LONG not in tip and UNSURE not in tip, f"a pYIN note names its weakest sub-score: {tip!r}")
    b.close()
finish(errors)
