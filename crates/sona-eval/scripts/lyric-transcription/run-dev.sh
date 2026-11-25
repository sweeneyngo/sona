#!/usr/bin/env bash
# Run candidate recognisers on the dev lead stems, one after another (lyrics-first 05).
#   run-dev.sh OUT_ROOT CANDIDATE...
# Transcripts land in OUT_ROOT/<name>/<id>.json; OUT_ROOT must be outside the repo (they are
# lyric-derived). Songs already transcribed are skipped.
#
# Environment (all outside the repo, as the report describes):
#   ~/.cache/sona/lyric-asr/venv       Python 3.10: faster-whisper, sherpa-onnx, fugashi, unidic-lite,
#                                      pykakasi, pyopenjtalk-plus, torch (CPU) and transformers 4.57
#   ~/.cache/sona/lyric-asr/venv-moss  torch (CPU), transformers >= 5.17 and faster-whisper (Echora Kiki)
#   ~/.cache/sona/lyric-asr/models     reazonspeech-k2-v2 (ONNX), and anime-whisper and whisper-ja-760M
#                                      converted with `ct2-transformers-converter --quantization int8`
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
stems="$here/../../data/pifuyuu-stems"
venv="${LYRIC_ASR_VENV:-$HOME/.cache/sona/lyric-asr/venv}"
moss_venv="${LYRIC_MOSS_VENV:-$HOME/.cache/sona/lyric-asr/venv-moss}"
models="${LYRIC_ASR_MODELS:-$HOME/.cache/sona/lyric-asr/models}"
export HF_HOME="${HF_HOME:-$HOME/.cache/sona/lyric-asr/hf}" HF_HUB_DISABLE_TELEMETRY=1
out="$1"; shift
# dev songs with a lead stem (plus-space has none)
ids=(amaten atamaro-deluxe azemichi-ni-ochita-ginga fukagyakuteki-henyou innocent-addict kamaitachi-ni-yoru
     my-axiom telescope-nevermore yamineko)
wavs=(); for id in "${ids[@]}"; do wavs+=("$stems/$id/lead.wav"); done
declare -A cand=(
  [whisper-small]="whisper:small"
  [reazonspeech-k2-v2]="k2:$models/reazonspeech-k2-v2"
  [kotoba-whisper-v2.0]="whisper:kotoba-tech/kotoba-whisper-v2.0-faster"
  [whisper-large-v3-turbo]="whisper:large-v3-turbo"
  [anime-whisper]="whisper:$models/anime-whisper-ct2"
  [whisper-ja-760M]="whisper:$models/whisper-ja-760M-ct2"
  [whisper-large-v3]="whisper:large-v3"
  [echora-kiki]="moss:hcxlabs/echora-kiki"
  [w2v-bert-lyrics-ctc]="ctc:mrfakename/w2v-bert-2.0-music-lyrics-ctc"
)
for name in "$@"; do
  words=1; case "$name" in kotoba-whisper-v2.0|anime-whisper) words=0 ;; esac
  py="$venv/bin/python"; [ "$name" = echora-kiki ] && py="$moss_venv/bin/python"
  ASR_WORDS=$words "$py" -I "$here/transcribe.py" "${cand[$name]}" "$out/$name" "${wavs[@]}"
  touch "$out/$name.done"
done
