#!/usr/bin/env bash
# Separate each fetched original into stems with demucs and record provenance
# in the manifest (UST corpus gate, ticket 03).
#
# demucs is CORPUS-PREP tooling only: it lives in its own venv OUTSIDE the Rust
# workspace and never enters the shipped product, so sona's "no ML runtime, no
# C dependency" identity (ADR-0005) is untouched.
#
# Outputs per song, git-ignored, as mono 44.1 kHz WAV (sona's input contract):
#   <stems>/<id>/vocals.wav  — the stem sona transcribes
#   <stems>/<id>/drums.wav   — an alignment aid (ticket 05): grid phase from the
#                              drums is independent of the vocal and of sona
# The manifest's stem.{model,version,path} is written for every song separated.
# Different models leave different bleed, so an unrecorded model change would
# silently invalidate comparisons — the version string pins it.
#
# Idempotent: a song whose stems exist and whose manifest provenance matches
# the current model/version is skipped.
#
# Usage:  crates/sona-eval/scripts/separate-stems.sh
# Env:    SONA_DEMUCS_VENV (default ~/.local/share/sona/demucs-venv),
#         SONA_DEMUCS_PYTHON (interpreter for creating the venv; needs 3.10–3.12),
#         SONA_UST_AUDIO_DIR, SONA_UST_STEMS_DIR, SONA_UST_MANIFEST
set -euo pipefail

MODEL="htdemucs"
DEMUCS_VERSION="4.0.1"
TORCH_VERSION="2.5.1"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="${SONA_UST_MANIFEST:-$script_dir/../corpus/ust-manifest.json}"
audio="${SONA_UST_AUDIO_DIR:-$script_dir/../data/pifuyuu-audio}"
stems="${SONA_UST_STEMS_DIR:-$script_dir/../data/pifuyuu-stems}"
venv="${SONA_DEMUCS_VENV:-$HOME/.local/share/sona/demucs-venv}"
version_tag="demucs $DEMUCS_VERSION (torch $TORCH_VERSION, cpu)"

SONA_DEMUCS_VENV="$venv" "$script_dir/../../../scripts/setup-separators.sh" demucs
"$venv/bin/python" -c "import demucs, sys; assert demucs.__version__ == '$DEMUCS_VERSION', demucs.__version__"

mkdir -p "$stems"
done_ids=()
while IFS=$'\t' read -r id model version; do
  src="$audio/$id.wav"
  out="$stems/$id"
  if [ ! -f "$src" ]; then
    echo "  skip    $id: no audio (run fetch-pifuyuu-audio.sh)"; continue
  fi
  if [ -f "$out/vocals.wav" ] && [ -f "$out/drums.wav" ] && [ "$model" = "$MODEL" ] && [ "$version" = "$version_tag" ]; then
    echo "  cached  $id"; continue
  fi
  echo "  split   $id"
  tmp="$(mktemp -d)"
  "$venv/bin/python" -m demucs -n "$MODEL" -o "$tmp" "$src" >/dev/null 2>&1 \
    || { echo "error: demucs failed on $id" >&2; rm -rf "$tmp"; exit 1; }
  mkdir -p "$out"
  for part in vocals drums; do
    ffmpeg -nostdin -loglevel error -y -i "$tmp/$MODEL/$id/$part.wav" -ac 1 -ar 44100 -c:a pcm_s16le "$out/$part.wav"
  done
  rm -rf "$tmp"
  # Verify: the vocal stem spans the source.
  python3 -I -c '
import subprocess, sys
d = lambda p: float(subprocess.check_output(["ffprobe","-v","error","-show_entries","format=duration","-of","csv=p=0",p]))
a, v = d(sys.argv[1]), d(sys.argv[2])
sys.exit(0 if abs(a - v) < 0.5 else f"stem length {v:.1f}s != source {a:.1f}s")' "$src" "$out/vocals.wav"
  done_ids+=("$id")
done < <(python3 -I -c '
import json, sys
for s in json.load(open(sys.argv[1], encoding="utf-8"))["songs"]:
    st = s.get("stem") or {}
    print(s["id"], st.get("model") or "", st.get("version") or "", sep="\t")' "$manifest")

if [ "${#done_ids[@]}" -gt 0 ]; then
  python3 -I - "$manifest" "$MODEL" "$version_tag" "${done_ids[@]}" <<'PY'
import json, sys
path, model, version, *ids = sys.argv[1:]
m = json.load(open(path, encoding="utf-8"))
for s in m["songs"]:
    if s["id"] in ids:
        s["stem"] = {"model": model, "version": version, "path": f"{s['id']}/vocals.wav"}
with open(path, "w", encoding="utf-8") as f:
    json.dump(m, f, ensure_ascii=False, indent=2)
    f.write("\n")
PY
  echo "recorded stem provenance for ${#done_ids[@]} song(s) in the manifest"
fi
echo "Stems at $stems"
