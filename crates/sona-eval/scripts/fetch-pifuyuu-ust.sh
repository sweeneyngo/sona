#!/usr/bin/env bash
# Fetch the UST zips named in the UST corpus manifest from Pifuyuu's public
# OneDrive folder into the git-ignored cache (UST corpus gate, tickets 02/03).
# The USTs are NEVER committed: the author's terms forbid redistribution.
#
# OneDrive serves a migrated public folder only behind a short-lived anonymous
# token — the same one its web viewer requests for any visitor of a public link.
#
# Idempotent: a song whose cached zip already matches the manifest's sha256 is
# skipped. Verified: a download whose sha256 disagrees with the manifest is
# rejected, and every listed lead/harmony file must exist after extraction.
#
# Usage:
#   crates/sona-eval/scripts/fetch-pifuyuu-ust.sh            # fetch + verify
#   crates/sona-eval/scripts/fetch-pifuyuu-ust.sh --record   # also write missing
#                                                            # sha256s into the manifest
# Env: SONA_UST_DIR (cache, default crates/sona-eval/data/pifuyuu-ust),
#      SONA_UST_MANIFEST (default crates/sona-eval/corpus/ust-manifest.json)
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export MANIFEST="${SONA_UST_MANIFEST:-$script_dir/../corpus/ust-manifest.json}"
export DEST="${SONA_UST_DIR:-$script_dir/../data/pifuyuu-ust}"
export RECORD=0
[ "${1:-}" = "--record" ] && RECORD=1
mkdir -p "$DEST"

python3 -I - <<'PY'
import base64, hashlib, io, json, os, shutil, sys, unicodedata, urllib.request, zipfile

manifest_path, dest, record = os.environ["MANIFEST"], os.environ["DEST"], os.environ["RECORD"] == "1"
manifest = json.load(open(manifest_path, encoding="utf-8"))

def get(url, headers=None, data=None):
    req = urllib.request.Request(url, data=data, headers=headers or {})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()

def sha256(b):
    return hashlib.sha256(b).hexdigest()

def zip_name(info):
    # Zips from Japanese Windows store Shift-JIS names without the UTF-8 flag;
    # Python then decodes them as cp437. Re-decode those as cp932.
    if info.flag_bits & 0x800:
        return info.filename
    try:
        return info.filename.encode("cp437").decode("cp932")
    except (UnicodeEncodeError, UnicodeDecodeError):
        return info.filename

def extract(blob, target):
    """Extract into a fresh dir, refusing absolute paths and '..' (zip-slip)."""
    tmp = target + ".partial"
    shutil.rmtree(tmp, ignore_errors=True)
    os.makedirs(tmp)
    with zipfile.ZipFile(io.BytesIO(blob)) as z:
        for info in z.infolist():
            name = zip_name(info)
            parts = [p for p in name.replace("\\", "/").split("/") if p not in ("", ".")]
            if not parts or name.startswith("/") or ".." in parts:
                raise SystemExit(f"refusing suspicious path in zip: {name!r}")
            if info.is_dir():
                continue
            path = os.path.join(tmp, *parts)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "wb") as f:
                f.write(z.read(info))
    shutil.rmtree(target, ignore_errors=True)
    os.rename(tmp, target)

todo, cached = [], 0
for song in manifest["songs"]:
    marker = os.path.join(dest, song["id"], ".zip.sha256")
    want = song.get("ust_zip_sha256")
    if want and os.path.exists(marker) and open(marker).read().strip() == want:
        cached += 1
    else:
        todo.append(song)
print(f"{cached} song(s) already cached and verified; {len(todo)} to fetch")

if todo:
    # Anonymous token, then list the shared folder (paginated).
    token = json.loads(get("https://api-badgerp.svc.ms/v1.0/token",
                           {"Content-Type": "application/json"},
                           json.dumps({"appId": "5cbed6ac-a083-4e14-b191-b4ba07653de2"}).encode()))["token"]
    auth = {"Authorization": f"Badger {token}", "Prefer": "autoredeem"}
    share = "u!" + base64.urlsafe_b64encode(manifest["ust_share_url"].encode()).decode().rstrip("=")
    page = json.loads(get(f"https://my.microsoftpersonalcontent.com/_api/v2.0/shares/{share}/driveitem?$expand=children&$top=1000", auth))
    items, nxt = page.get("children", []), page.get("children@odata.nextLink")
    while nxt:
        page = json.loads(get(nxt, auth))
        items += page.get("value", [])
        nxt = page.get("@odata.nextLink")
    by_name = {unicodedata.normalize("NFC", i["name"]): i for i in items}
    print(f"listed {len(items)} files in the UST folder")

    failed = []
    for song in todo:
        item = by_name.get(unicodedata.normalize("NFC", song["ust_zip"]))
        if item is None:
            failed.append(f"{song['id']}: {song['ust_zip']} not in the folder")
            continue
        blob = get(item["@content.downloadUrl"])
        digest = sha256(blob)
        want = song.get("ust_zip_sha256")
        if want and digest != want:
            failed.append(f"{song['id']}: sha256 mismatch (got {digest}, manifest {want})")
            continue
        try:
            zipfile.ZipFile(io.BytesIO(blob)).testzip()
        except zipfile.BadZipFile as e:
            failed.append(f"{song['id']}: not a valid zip ({e})")
            continue
        target = os.path.join(dest, song["id"])
        extract(blob, target)
        with open(os.path.join(target, ".zip.sha256"), "w") as f:
            f.write(digest + "\n")
        if not want:
            if record:
                song["ust_zip_sha256"] = digest
            else:
                print(f"  note: {song['id']} has no sha256 in the manifest (got {digest}); rerun with --record")
        print(f"  fetched {song['id']:34} {len(blob):>7} bytes  sha256 {digest[:12]}…")
    if record:
        with open(manifest_path, "w", encoding="utf-8") as f:
            json.dump(manifest, f, ensure_ascii=False, indent=2)
            f.write("\n")
    if failed:
        sys.exit("error:\n  " + "\n  ".join(failed))

# Every song: the files the manifest names must be on disk.
missing = []
for song in manifest["songs"]:
    for rel in (song.get("ust_lead_files") or []) + (song.get("ust_other_voice_files") or []):
        p = os.path.join(dest, song["id"], rel)
        if not (os.path.isfile(p) and os.path.getsize(p) > 0):
            missing.append(f"{song['id']}/{rel}")
if missing:
    sys.exit("error: manifest files missing after fetch:\n  " + "\n  ".join(missing))
print(f"UST cache ready at {dest}")
PY
