#!/usr/bin/env bash
# Create the Python environments that `sona serve` (and the corpus scripts) use to separate vocals:
#   demucs  → ~/.local/share/sona/demucs-venv     (vocals from a song; htdemucs)
#   lead    → ~/.local/share/sona/separator-venv  (the lead singer from the vocals; UVR karaoke)
# They live outside the Rust workspace and never become crate dependencies (ADR-0005).
# One-time, about 2 GB each (CPU torch), plus the models they run: the htdemucs weights (84 MB, in
# torch's cache) and the UVR karaoke model (53 MB, in the lead venv's models/). Already-built
# environments and already-downloaded models are left as they are.
#
# Usage:  scripts/setup-separators.sh [demucs|lead|all]     (default: all)
# Env:    SONA_DEMUCS_VENV, SONA_LEAD_VENV, SONA_LEAD_MODEL, SONA_PYTHON (a Python 3.10–3.12 interpreter)
set -euo pipefail

DEMUCS_VERSION="4.0.1"
TORCH_VERSION="2.5.1"
demucs_venv="${SONA_DEMUCS_VENV:-$HOME/.local/share/sona/demucs-venv}"
lead_venv="${SONA_LEAD_VENV:-$HOME/.local/share/sona/separator-venv}"
lead_model="${SONA_LEAD_MODEL:-UVR_MDXNET_KARA_2.onnx}"

# sona serve turns separation off without ffmpeg, and audio-separator refuses to start without it.
if ! ffmpeg -version >/dev/null 2>&1; then
  echo "error: ffmpeg doesn't run; install it first (e.g. 'sudo apt install ffmpeg', 'sudo dnf install ffmpeg', 'brew install ffmpeg')" >&2
  exit 1
fi

find_python() {
  if [ -n "${SONA_PYTHON:-${SONA_DEMUCS_PYTHON:-}}" ]; then echo "${SONA_PYTHON:-$SONA_DEMUCS_PYTHON}"; return; fi
  # Probe by running each candidate: a pyenv shim exists on PATH even when its version isn't active.
  for cand in python3.12 python3.11 python3.10 "$HOME"/.pyenv/versions/3.1[012].*/bin/python; do
    if "$cand" -c 'import sys; sys.exit(0 if (3, 10) <= sys.version_info[:2] <= (3, 12) else 1)' >/dev/null 2>&1; then
      echo "$cand"; return
    fi
  done
  echo "error: need Python 3.10–3.12; set SONA_PYTHON" >&2
  exit 1
}

setup_demucs() {
  if [ ! -x "$demucs_venv/bin/python" ]; then
    py="$(find_python)"
    echo "Creating the demucs environment at $demucs_venv (one-time, ~2 GB)"
    "$py" -m venv "$demucs_venv"
    "$demucs_venv/bin/pip" install --quiet --upgrade pip
    "$demucs_venv/bin/pip" install --quiet "torch==$TORCH_VERSION" "torchaudio==$TORCH_VERSION" \
        --index-url https://download.pytorch.org/whl/cpu
    "$demucs_venv/bin/pip" install --quiet "demucs==$DEMUCS_VERSION" "soundfile"
  fi
  "$demucs_venv/bin/python" -c "import demucs; assert demucs.__version__ == '$DEMUCS_VERSION', demucs.__version__"
  "$demucs_venv/bin/python" -c "from demucs.pretrained import get_model; get_model('htdemucs')" >/dev/null
  echo "demucs ready: $demucs_venv (htdemucs weights in torch's cache)"
}

setup_lead() {
  if [ ! -x "$lead_venv/bin/audio-separator" ]; then
    py="$(find_python)"
    echo "Creating the lead-separator environment at $lead_venv (one-time, ~2 GB)"
    "$py" -m venv "$lead_venv"
    "$lead_venv/bin/pip" install --quiet --upgrade pip
    "$lead_venv/bin/pip" install --quiet torch torchvision --index-url https://download.pytorch.org/whl/cpu
    "$lead_venv/bin/pip" install --quiet "audio-separator[cpu]>=0.30"
  fi
  # torch and torchvision must come from the same build (audio-separator can pull a PyPI torchvision
  # built against a different torch); repair a mismatched environment.
  if ! "$lead_venv/bin/python" -c "import torchvision" >/dev/null 2>&1; then
    torch_v="$("$lead_venv/bin/python" -c 'import torch; print(torch.__version__.split("+")[0])')"
    echo "Repairing torchvision to match torch $torch_v"
    "$lead_venv/bin/pip" install --quiet --force-reinstall --no-deps "torch==$torch_v" torchvision --index-url https://download.pytorch.org/whl/cpu
  fi
  mkdir -p "$lead_venv/models"
  "$lead_venv/bin/audio-separator" --download_model_only --log_level warning --model_filename "$lead_model" --model_file_dir "$lead_venv/models" >/dev/null
  echo "lead separator ready: $lead_venv ($lead_model in $lead_venv/models)"
}

case "${1:-all}" in
  demucs) setup_demucs ;;
  lead) setup_lead ;;
  all) setup_demucs; setup_lead ;;
  *) echo "usage: $0 [demucs|lead|all]" >&2; exit 2 ;;
esac
