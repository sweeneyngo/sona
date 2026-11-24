"""Shared settings and helpers for the browser drivers (see README.md). Nothing here is corpus data: the
default annotation and stem are synthetic (fixture.py), and any you set point at local, uncommitted files."""
import json, os, pathlib, tempfile

from fixture import synthetic

def _path(var, hint):
    v = os.environ.get(var)
    return pathlib.Path(v) if v else pathlib.Path("/nonexistent") / hint

SONG = _path("SONA_E2E_SONG", "song.mp3")                     # a short full mix, for the song-to-notes driver
EDITOR_URL = os.environ.get("SONA_E2E_EDITOR_URL", "http://localhost:5173/")   # `npm run dev`
SERVE_URL = os.environ.get("SONA_E2E_SERVE_URL", "http://127.0.0.1:8977/")     # `sona serve`
OUT = pathlib.Path(os.environ.get("SONA_E2E_OUT", pathlib.Path(tempfile.gettempdir()) / "sona-e2e"))
(OUT / "shots").mkdir(parents=True, exist_ok=True)
if os.environ.get("SONA_E2E_ANNOTATION"):
    ANNOTATION = _path("SONA_E2E_ANNOTATION", "annotation.json")  # an annotation from `sona transcribe` (with an f0 contour)
    STEM = _path("SONA_E2E_STEM", "stem.wav")                     # its vocal stem
else:
    ANNOTATION, STEM = synthetic(OUT)                             # fixture.py: built here, never committed

def mclick(page, sel):
    """Click a menu item, opening its menu first."""
    if not page.is_visible(sel):
        page.evaluate("(s) => document.querySelector(s).closest('.menu').querySelector('.menu-btn').click()", sel)
    page.click(sel)

FIND_NOTE = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),sx=c.width/r.width,sy=c.height/r.height,d=g.getImageData(0,0,c.width,c.height).data;
 for(let x=Math.floor(c.width*0.45);x<c.width*0.9;x+=3)for(let y=40;y<c.height-40;y+=2){const i=(y*c.width+x)*4,R=d[i],G=d[i+1],B=d[i+2];if(G>110&&R<170&&B<90&&G>R)return {x:r.left+x/sx+10,y:r.top+y/sy+4};} return null; }"""
# handle blobs: a 5x5 block that is all handle-orange (the 2px curve line can't fill it)
FIND_HANDLES = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),sx=c.width/r.width,sy=c.height/r.height,W=c.width,H=c.height,d=g.getImageData(0,0,W,H).data;
 const fill=(x,y)=>{const i=(y*W+x)*4;return d[i]>240&&d[i+1]>195&&d[i+1]<225&&d[i+2]>140&&d[i+2]<175;}; const cl=[];
 for(let y=0;y<H;y++)for(let x=0;x<W;x++)if(fill(x,y)){let c0=cl.find(c=>Math.abs(c.cx-x)<8*sx&&Math.abs(c.cy-y)<8*sy);if(!c0){c0={sx:0,sy:0,n:0,cx:x,cy:y};cl.push(c0);}c0.sx+=x;c0.sy+=y;c0.n++;c0.cx=c0.sx/c0.n;c0.cy=c0.sy/c0.n;}
 return cl.filter(c=>c.n>=6).map(c=>({x:r.left+c.cx/sx,y:r.top+c.cy/sy})).sort((a,b)=>a.x-b.x); }"""
failures = []
def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failures.append(what)
def finish(errors):
    print("failures:", failures or "none")
    print("console errors:", errors or "none")
    raise SystemExit(1 if failures else 0)

def watch(page, errors):
    """Collect page errors and console errors. A failed /api call is the missing `sona serve`, not an app error."""
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.on("console", lambda m: m.type == "error" and "/api/" not in (m.location or {}).get("url", "") and errors.append(m.text))

# note bars on the roll, widest first: rows of note-green runs that share both ends (the contour and lyrics only split rows);
# one voice, so bars that share most of their time span are one note
FIND_NOTES = """() => { const c=document.querySelector('#roll'),g=c.getContext('2d'),r=c.getBoundingClientRect(),sx=c.width/r.width,sy=c.height/r.height,W=c.width,H=c.height,d=g.getImageData(0,0,W,H).data;
 const on=(x,y)=>{const i=(y*W+x)*4,R=d[i],G=d[i+1],B=d[i+2];return G>110&&G>R+50&&B<140;}; const rs=[];
 for(let y=0;y<H;y++)for(let x=0;x<W;){if(!on(x,y)){x++;continue;}let x1=x;while(x1<W&&on(x1,y))x1++;
  if(x1-x>=6*sx){const m=rs.find(q=>Math.abs(q.x0-x)<=2*sx&&Math.abs(q.x1-x1)<=2*sx&&y-q.y1<=40*sy);if(m)m.y1=y;else rs.push({x0:x,x1,y0:y,y1:y});} x=x1;}
 const ns=[]; for(const q of rs.filter(q=>q.y1-q.y0>=4*sy)){const m=ns.find(n=>Math.min(n.x1,q.x1)-Math.max(n.x0,q.x0)>0.5*Math.min(n.x1-n.x0,q.x1-q.x0));
  if(m){m.x0=Math.min(m.x0,q.x0);m.x1=Math.max(m.x1,q.x1);m.y0=Math.min(m.y0,q.y0);m.y1=Math.max(m.y1,q.y1);}else ns.push({...q});}
 return ns.map(q=>({left:r.left+q.x0/sx,right:r.left+q.x1/sx,w:(q.x1-q.x0)/sx,x:r.left+(q.x0+q.x1)/2/sx,y:r.top+(q.y0+q.y1+1)/2/sy,h:(q.y1-q.y0+1)/sy})).sort((a,b)=>b.w-a.w); }"""
def save(page):
    """Download the session JSON through File → Save session."""
    with page.expect_download() as dl: mclick(page, "#save")
    return json.loads(open(dl.value.path()).read())
def sel_expr(s, onset):
    n = [n for n in s["notes"] if abs(n["onset_s"] - onset) < 1e-6][0]; return n.get("expr")
