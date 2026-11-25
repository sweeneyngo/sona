"""utau-edit-cost 17: the ear-label kit for a held-out split's music frames (beat and bar 1), before its read.

The labels file is local and never committed (`~/.cache/sona/holdout4/labels.json`); it holds what the user
hears, per song:
    {"format": "sona-music-frame-labels/1", "split": "holdout4", "songs": {"<id>": {
        "meter": null | "4/4" | "other",     # null: not labelled yet; "other": not 4/4 or can't be placed (excluded)
        "beat_shift_cells": null | int,      # the beat overlay whose clicks sit on the kick and snare (0 .. cells per beat - 1)
        "bar_shift_cells": null | int,       # the bar overlay whose high click starts phrases and chords (beat + k * cells per beat)
        "confirmed": false | true,           # the user heard both excerpts at these shifts and is sure
        "note": ""}}}
`sona-eval-frame --estimate-bar-one --music-frames FILE` reads it (`sona_eval::frame::ear_labels`).

Commands (each reads only the manifest's offset and BPM and the song's mix; no UST note, no sona output):
    kit   write the beat overlays for every song (one per beat shift, at 25% and 75% of the song) and the
          labels template if there is none
    bars  write the bar overlays for every song whose beat is labelled and bar is not
    check validate the labels and say whether the read can start (>= MIN_CONFIRMED confirmed, every label
          consistent); exit 1 if not
    from-manifest  (dev rehearsal only) a labels file from the manifest's own music frames

Usage: python3 -I labels.py kit|bars|check|from-manifest --split S --labels FILE [--clicks-dir DIR]
       [--bin DIR] [--min-confirmed 5]
Env: SONA_UST_MANIFEST, SONA_UST_AUDIO_DIR as the Rust tools read them.
"""
import sys

sys.dont_write_bytecode = True  # never leave __pycache__ beside the scripts
import json, os, subprocess, wave  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
FORMAT = "sona-music-frame-labels/1"
MIN_CONFIRMED = 5
EXCERPT_S = 24.0
AT = (("q1", 0.25), ("q3", 0.75))


def arg(k, d=None):
    return sys.argv[sys.argv.index(k) + 1] if k in sys.argv else d


def manifest():
    p = os.environ.get("SONA_UST_MANIFEST") or os.path.join(REPO, "crates/sona-eval/corpus/ust-manifest.json")
    return json.load(open(p))


def audio_dir():
    return os.environ.get("SONA_UST_AUDIO_DIR") or os.path.join(REPO, "crates/sona-eval/data/pifuyuu-audio")


def songs_of(split):
    return [s for s in manifest()["songs"] if s.get("split") == split]


def cells_per_beat(song):
    return (song.get("grid_division") or 16) // 4


def duration_s(sid):
    with wave.open(os.path.join(audio_dir(), sid + ".wav")) as w:
        return w.getnframes() / w.getframerate()


def overlay(bin_dir, sid, shift, at, out):
    if os.path.exists(out):
        return
    start = max(0.0, duration_s(sid) * at - EXCERPT_S / 2)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    subprocess.run([os.path.join(bin_dir, "sona-eval-frame"), "--clicks", sid, "--bar-shift", str(shift), "--from", f"{start:.2f}",
                    "--seconds", str(EXCERPT_S), "--out", out], check=True, stdout=subprocess.DEVNULL)


def load(path):
    return json.load(open(path))


def template(split):
    return {"format": FORMAT, "split": split, "songs": {s["id"]: {"meter": None, "beat_shift_cells": None, "bar_shift_cells": None,
                                                                   "confirmed": False, "note": ""} for s in songs_of(split)}}


def problems(labels, split):
    """Every reason the labels can't be read, and the songs per state."""
    errs, state = [], {"4/4": [], "other": [], "unlabelled": [], "confirmed": []}
    if labels.get("format") != FORMAT or labels.get("split") != split:
        errs.append(f"labels: format {labels.get('format')!r} / split {labels.get('split')!r}, expected {FORMAT!r} / {split!r}")
    by_id = {s["id"]: s for s in songs_of(split)}
    for sid in labels.get("songs", {}):
        if sid not in by_id:
            errs.append(f"{sid}: not a {split} song")
    for sid, song in by_id.items():
        l = labels.get("songs", {}).get(sid)
        if l is None or l.get("meter") is None:
            state["unlabelled"].append(sid)
            continue
        if l["meter"] != "4/4":
            state["other"].append(sid)
            continue
        c = cells_per_beat(song)
        beat, bar = l.get("beat_shift_cells"), l.get("bar_shift_cells")
        if not isinstance(beat, int) or not isinstance(bar, int):
            errs.append(f"{sid}: 4/4 needs whole beat_shift_cells and bar_shift_cells")
        elif not (0 <= beat < c and 0 <= bar < 4 * c and bar % c == beat):
            errs.append(f"{sid}: bar {bar} is not on beat {beat} ({c} cells per beat, {4 * c} per bar)")
        state["4/4"].append(sid)
        if l.get("confirmed") is True:
            state["confirmed"].append(sid)
    return errs, state


def kit(split, path, clicks, bin_dir):
    for s in songs_of(split):
        for k in range(cells_per_beat(s)):
            for tag, at in AT:
                overlay(bin_dir, s["id"], k, at, os.path.join(clicks, s["id"], f"beat{k}-{tag}.wav"))
        print(f"{s['id']}: beat overlays 0..{cells_per_beat(s) - 1}")
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        json.dump(template(split), open(path, "w"), indent=1)
        print(f"wrote the labels template {path}")


def bars(split, path, clicks, bin_dir):
    labels = load(path)
    for s in songs_of(split):
        l = labels["songs"].get(s["id"], {})
        beat = l.get("beat_shift_cells")
        if l.get("meter") == "other" or not isinstance(beat, int) or l.get("bar_shift_cells") is not None:
            continue
        c = cells_per_beat(s)
        for j in range(4):
            for tag, at in AT:
                overlay(bin_dir, s["id"], beat + j * c, at, os.path.join(clicks, s["id"], f"bar{beat + j * c}-{tag}.wav"))
        print(f"{s['id']}: bar overlays {', '.join(str(beat + j * c) for j in range(4))}")


def check(split, path, min_confirmed):
    errs, st = problems(load(path), split)
    print(f"{split}: {len(st['4/4'])} labelled in 4/4 ({len(st['confirmed'])} confirmed), {len(st['other'])} excluded as not 4/4 "
          f"({', '.join(st['other']) or 'none'}), {len(st['unlabelled'])} not labelled yet")
    for e in errs:
        print("  " + e)
    ready = not errs and not st["unlabelled"] and len(st["confirmed"]) >= min_confirmed
    print("ready for the read" if ready else f"NOT ready (needs every song labelled, no errors, >= {min_confirmed} confirmed)")
    return ready, st


def from_manifest(split, path):
    out = template(split)
    for s in songs_of(split):
        if s.get("music_bar_shift_cells") is not None:
            out["songs"][s["id"]] = {"meter": "4/4", "beat_shift_cells": s["music_beat_shift_cells"], "bar_shift_cells": s["music_bar_shift_cells"],
                                     "confirmed": False, "note": f"rehearsal: copied from the manifest ({s.get('music_frame_status')})"}
        else:
            out["songs"][s["id"]].update(meter="other", note="rehearsal: no music frame in the manifest")
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    json.dump(out, open(path, "w"), indent=1)
    print(f"wrote {path}: {sum(1 for l in out['songs'].values() if l['meter'] == '4/4')} songs from the manifest")


def main():
    cmd, split, path = sys.argv[1], arg("--split"), arg("--labels")
    if not split or not path:
        sys.exit(__doc__)
    bin_dir = arg("--bin", os.path.join(REPO, "target/release"))
    if cmd == "kit":
        kit(split, path, arg("--clicks-dir"), bin_dir)
    elif cmd == "bars":
        bars(split, path, arg("--clicks-dir"), bin_dir)
    elif cmd == "check":
        sys.exit(0 if check(split, path, int(arg("--min-confirmed", MIN_CONFIRMED)))[0] else 1)
    elif cmd == "from-manifest":
        from_manifest(split, path)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
