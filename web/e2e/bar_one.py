"""Bar 1 and one export timeline (ADR-0029), on a synthetic annotation and a silent stem it writes itself.

Checks: the default UST is the one the pre-frame code wrote, and the MIDI sits on the same ticks; Shift-click on
the ruler makes that line a bar line, in the ruler and in every export; undo/redo; a ruler drag moves the bars
with the grid; "At playhead"; a saved session reloads to the same UST."""
import json, re, struct, time, wave
from playwright.sync_api import sync_playwright

from _env import *  # noqa: F403

BPM, CELL = 120, 0.125
ONSETS = [1.3, 1.8, 2.3, 2.8, 3.3, 3.8, 4.3]
errors, failures = [], []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)


def fixtures():
    ann = OUT / "bar_one.annotation.json"
    sub = {"pitch_stability": 0.95, "voicing": 0.95, "onset_clarity": 0.5, "offset_clarity": 0.95}
    notes = [{"onset_s": t, "offset_s": t + 0.25, "midi_pitch": 64, "confidence": 0.95, "sub_scores": sub} for t in ONSETS]
    ann.write_text(json.dumps({"version": 1, "sample_rate": 22050, "bpm": BPM, "duration_s": 8.0, "producer": "pyin", "notes": notes, "f0_contour": [], "energy_envelope": []}))
    wav = OUT / "bar_one.silence.wav"
    with wave.open(str(wav), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(22050); w.writeframes(b"\0\0" * 22050 * 8)
    return ann, wav


def ust_positions(path):
    tick, out = 0, []
    for m in re.finditer(r"^Length=(\d+)\r\nLyric=(.*)\r\nNoteNum=(\d+)", path.read_bytes().decode("cp932"), re.M):
        if m.group(2) != "R":
            out.append(tick)
        tick += int(m.group(1))
    return out


def midi_positions(path):
    b = path.read_bytes()
    ppq, i, end, tick, out = struct.unpack(">H", b[12:14])[0], 22, 22 + struct.unpack(">I", b[18:22])[0], 0, []
    def vlq():
        nonlocal i
        v = 0
        while True:
            c = b[i]; i += 1; v = v * 128 + (c & 0x7F)
            if c < 0x80: return v
    while i < end:
        tick += vlq(); st = b[i]; i += 1
        if st == 0xFF:
            i += 1; n = vlq(); i += n
        else:
            if st & 0xF0 == 0x90 and b[i + 1] > 0: out.append(tick * 480 // ppq)
            i += 2
    return out


NOTE_STARTS = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),s=c.width/r.width,d=g.getImageData(0,0,c.width,c.height).data;
 let best=null; for(let y=30;y<c.height-30;y+=2){const xs=[];let inRun=false;for(let x=0;x<c.width;x++){const i=(y*c.width+x)*4,R=d[i],G=d[i+1],B=d[i+2],n=G>110&&R<170&&B<90&&G>R;if(n&&!inRun)xs.push(r.left+x/s);inRun=n;} if(!best||xs.length>best.xs.length)best={xs,y:r.top+y/s};}
 return best; }"""
BAR_TICKS = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),s=c.width/r.width,y=Math.round(3*s),d=g.getImageData(0,y,c.width,1).data;
 const xs=[];let inRun=false;for(let x=Math.round(46*s);x<c.width;x++){const n=d[x*4]+d[x*4+1]+d[x*4+2]>150;if(n&&!inRun)xs.push(r.left+x/s);inRun=n;} return xs; }"""


def load(page, path, wav):
    mclick(page, "#settings"); page.set_input_files("#json-input", str(path)); page.set_input_files("#audio-input", str(wav))
    page.wait_for_function("!document.querySelector('#play').disabled", timeout=60000); page.click("#settings-close")


def export(page, button, name):
    with page.expect_download(timeout=30000) as dl:
        mclick(page, button)
    out = OUT / name; dl.value.save_as(str(out)); return out


ann, wav = fixtures()
with sync_playwright() as p:
    b = p.chromium.launch(args=["--no-sandbox"])
    ctx = b.new_context(viewport={"width": 1500, "height": 860}, accept_downloads=True)
    page = ctx.new_page()
    page.on("console", lambda m: m.type == "error" and "/api/" not in (m.location or {}).get("url", "") and errors.append(m.text)); page.on("pageerror", lambda e: errors.append(str(e)))
    page.goto(EDITOR_URL); page.evaluate("localStorage.clear()"); page.reload(); load(page, ann, wav)

    default_ust = ust_positions(export(page, "#export-ust", "bar_one.default.ust"))
    default_mid = midi_positions(export(page, "#export", "bar_one.default.mid"))
    want_default = [round((t - 0.05) * 960) for t in ONSETS]
    check(default_ust == want_default, f"default UST is the pre-frame one (tick 0 = grid phase 50 ms): {default_ust[:3]}")
    check(default_mid == default_ust, f"MIDI on the UST's ticks: {default_mid[:3]} (stem-seconds MIDI would be {[round(t * 960) for t in ONSETS[:3]]})")
    print("status:", page.inner_text("#status"))

    notes = page.evaluate(NOTE_STARTS)
    xs = notes["xs"]
    check(len(xs) == len(ONSETS), f"found the {len(ONSETS)} notes on the roll: {len(xs)}")
    px_per_s = (xs[-1] - xs[0]) / (ONSETS[-1] - ONSETS[0])
    roll = page.query_selector("#roll").bounding_box()
    page.keyboard.down("Shift"); page.mouse.click(xs[2] + 1, roll["y"] + 9); page.keyboard.up("Shift"); time.sleep(0.2)
    print("status:", page.inner_text("#status"))
    page.click("#tempo-chip"); readout = page.inner_text("#bar-one-readout"); auto_on = page.is_enabled("#bar-one-auto"); page.keyboard.press("Escape")
    check(readout == "2.300 s" and auto_on, f"Shift-click on the note at 2.3 s sets bar 1 there: readout {readout!r}, Auto enabled {auto_on}")
    bars = page.evaluate(BAR_TICKS)
    near = min(abs(x - xs[2]) for x in bars) if bars else 99
    gaps = [round((b2 - b1) / px_per_s, 2) for b1, b2 in zip(bars, bars[1:])]
    check(near < 5 and all(abs(g - 2.0) < 0.05 for g in gaps), f"ruler bar ticks every bar (2 s) through the clicked line: off by {near:.1f} px, gaps {gaps}")
    page.screenshot(path=str(OUT / "shots" / "bar_one.png"))

    bar_ust = ust_positions(export(page, "#export-ust", "bar_one.bar.ust"))
    bar_mid = midi_positions(export(page, "#export", "bar_one.bar.mid"))
    check(bar_ust[2] % 1920 == 0 and all(t % 480 == 0 for t in bar_ust), f"UST: the clicked note on a bar line, the others on beats: {bar_ust}")
    check(bar_mid == bar_ust, f"MIDI the same: {bar_mid}")
    print("status:", page.inner_text("#status"))

    page.keyboard.press("Control+z"); time.sleep(0.2)
    undone = ust_positions(export(page, "#export-ust", "bar_one.undone.ust"))
    page.keyboard.press("Control+Shift+z"); time.sleep(0.2)
    redone = ust_positions(export(page, "#export-ust", "bar_one.redone.ust"))
    check(undone == default_ust and redone == bar_ust, f"Ctrl+Z restores the default frame, Ctrl+Shift+Z bar 1: {undone[:2]} / {redone[:2]}")

    sess = export(page, "#save", "bar_one.session.json")
    saved = json.loads(sess.read_text())["session"].get("barOne")
    page2 = ctx.new_page(); page2.on("pageerror", lambda e: errors.append(str(e)))
    page2.goto(EDITOR_URL); page2.evaluate("localStorage.clear()"); page2.reload(); load(page2, sess, wav)
    reloaded = export(page2, "#export-ust", "bar_one.reloaded.ust")
    check(abs(saved - 2.3) < 1e-9 and reloaded.read_bytes() == (OUT / "bar_one.redone.ust").read_bytes(), f"session keeps barOne={saved} and reloads to the same UST bytes")
    page2.close()

    dx = 0.04 * px_per_s
    page.mouse.move(roll["x"] + 300, roll["y"] + 9); page.mouse.down(); page.mouse.move(roll["x"] + 300 + dx / 2, roll["y"] + 9); page.mouse.move(roll["x"] + 300 + dx, roll["y"] + 9); page.mouse.up(); time.sleep(0.2)
    dragged = ust_positions(export(page, "#export-ust", "bar_one.dragged.ust"))
    check(dragged == bar_ust, f"a 40 ms ruler drag moves the bars with the grid: {dragged[:3]}")

    page.mouse.click(xs[3] + 1, roll["y"] + 9); time.sleep(0.1)
    page.click("#tempo-chip"); page.click("#bar-one-here"); readout = page.inner_text("#bar-one-readout"); page.keyboard.press("Escape")
    here = ust_positions(export(page, "#export-ust", "bar_one.here.ust"))
    check(here[3] % 1920 == 0, f"At playhead (seek to the 2.8 s note) puts it on a bar line: readout {readout!r}, {here}")

    page.click("#tempo-chip"); page.click("#bar-one-auto"); page.click("#tempo-chip")
    auto = ust_positions(export(page, "#export-ust", "bar_one.auto.ust"))
    print("after Auto (dragged grid, default rule):", auto[:3])
    b.close()

print("failures:", failures or "none")
print("console errors:", errors or "none")
raise SystemExit(1 if failures else 0)
