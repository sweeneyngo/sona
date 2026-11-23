"""Shared settings and helpers for the browser drivers (see README.md). Nothing here is corpus data:
the audio and annotation paths point at local, git-ignored files."""
import json, os, pathlib, tempfile

def _path(var, hint):
    v = os.environ.get(var)
    return pathlib.Path(v) if v else pathlib.Path("/nonexistent") / hint

ANNOTATION = _path("SONA_E2E_ANNOTATION", "annotation.json")  # an annotation from `sona transcribe` (with an f0 contour)
STEM = _path("SONA_E2E_STEM", "stem.wav")                     # its vocal stem
SONG = _path("SONA_E2E_SONG", "song.mp3")                     # a short full mix, for the song-to-notes driver
EDITOR_URL = os.environ.get("SONA_E2E_EDITOR_URL", "http://localhost:5173/")   # `npm run dev`
SERVE_URL = os.environ.get("SONA_E2E_SERVE_URL", "http://127.0.0.1:8977/")     # `sona serve`
OUT = pathlib.Path(os.environ.get("SONA_E2E_OUT", pathlib.Path(tempfile.gettempdir()) / "sona-e2e"))
(OUT / "shots").mkdir(parents=True, exist_ok=True)

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
def save(page):
    """Download the session JSON through File → Save session."""
    with page.expect_download() as dl: mclick(page, "#save")
    return json.loads(open(dl.value.path()).read())
def sel_expr(s, onset):
    n = [n for n in s["notes"] if abs(n["onset_s"] - onset) < 1e-6][0]; return n.get("expr")
