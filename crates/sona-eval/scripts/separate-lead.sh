#!/usr/bin/env bash
# Split each song's demucs vocal stem into lead and backing vocals with a UVR
# karaoke model, and record provenance beside the stem.
#
# Why: in the UST corpus, two thirds of the lead's notes sit where other voices
# sing too (harmony, doubling). Two voices in one stem wreck pYIN's voicing. In
# those regions grid mode gets only 49% of the lead's cells right, against 83%
# where the lead sings alone (dev split; see reports/grid-cell-decoder.md). A
# lead-only stem is what a UVR user can already make, so it is a fair input.
#
# Corpus-prep tooling only, like demucs: it lives in a venv outside the Rust
# workspace and never enters the shipped product.
#
# Outputs per song, git-ignored, as mono 44.1 kHz WAV:
#   <stems>/<id>/lead.wav             the lead vocal (sona-eval-ust-dev --stem lead)
#   <stems>/<id>/lead.provenance.json model + package version (a model change
#                                     changes bleed, so it must be recorded)
#
# Idempotent: a song whose lead.wav exists with matching provenance is skipped.
#
# Usage:  crates/sona-eval/scripts/separate-lead.sh [song-id ...]
# Env:    SONA_LEAD_VENV  (default ~/.local/share/sona/separator-venv)
#         SONA_LEAD_MODEL (default the UVR mel-band roformer karaoke model)
#         SONA_UST_STEMS_DIR
set -euo pipefail

MODEL="${SONA_LEAD_MODEL:-mel_band_roformer_karaoke_aufr33_viperx_sdr_10.1956.ckpt}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
stems="${SONA_UST_STEMS_DIR:-$script_dir/../data/pifuyuu-stems}"
venv="${SONA_LEAD_VENV:-$HOME/.local/share/sona/separator-venv}"
models_dir="$venv/models"

if [ ! -x "$venv/bin/audio-separator" ]; then
  py=""
  for cand in python3.12 python3.11 python3.10 "$HOME"/.pyenv/versions/3.1[012].*/bin/python; do
    if "$cand" -c 'import sys; sys.exit(0 if (3, 10) <= sys.version_info[:2] <= (3, 12) else 1)' >/dev/null 2>&1; then
      py="$cand"; break
    fi
  done
  [ -n "$py" ] || { echo "error: need Python 3.10–3.12" >&2; exit 1; }
  echo "Creating separator venv at $venv (one-time, ~2 GB with torch)"
  "$py" -m venv "$venv"
  "$venv/bin/pip" install --quiet --upgrade pip
  "$venv/bin/pip" install --quiet torch --index-url https://download.pytorch.org/whl/cpu
  "$venv/bin/pip" install --quiet "audio-separator[cpu]>=0.30"
fi
mkdir -p "$models_dir"
# Record the version actually installed: the provenance pins it, not this script.
pkg_version="$("$venv/bin/python" -c 'import importlib.metadata as m; print(m.version("audio-separator"))')"
tag="audio-separator $pkg_version / $MODEL"

ids=("$@")
if [ ${#ids[@]} -eq 0 ]; then
  for d in "$stems"/*/; do ids+=("$(basename "$d")"); done
fi

for id in "${ids[@]}"; do
  src="$stems/$id/vocals.wav"
  out="$stems/$id"
  [ -f "$src" ] || { echo "  skip    $id: no vocals.wav (run separate-stems.sh)"; continue; }
  if [ -f "$out/lead.wav" ] && grep -qF "\"$tag\"" "$out/lead.provenance.json" 2>/dev/null; then
    echo "  cached  $id"; continue
  fi
  echo "  lead    $id"
  tmp="$(mktemp -d)"
  "$venv/bin/audio-separator" "$src" --model_filename "$MODEL" --model_file_dir "$models_dir" \
      --output_dir "$tmp" --output_format WAV --single_stem Vocals >/dev/null 2>&1 \
    || { echo "error: audio-separator failed on $id" >&2; rm -rf "$tmp"; exit 1; }
  lead="$(find "$tmp" -iname '*Vocals*.wav' | head -1)"
  [ -n "$lead" ] || { echo "error: no Vocals output for $id in $tmp" >&2; ls "$tmp" >&2; exit 1; }
  ffmpeg -nostdin -loglevel error -y -i "$lead" -ac 1 -ar 44100 -c:a pcm_s16le "$out/lead.wav"
  rm -rf "$tmp"
  printf '{"source": "vocals.wav", "separator": "%s"}\n' "$tag" > "$out/lead.provenance.json"
done
echo "Done. Score with: cargo run --release -p sona-eval --bin sona-eval-ust-dev -- --stem lead --cfg grid:grid=est"
