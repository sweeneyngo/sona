#!/usr/bin/env bash
# Fetch each manifest song's ORIGINAL audio (YouTube / niconico) into the
# git-ignored cache (UST corpus gate, ticket 03). MAINTAINER-RUN: these are
# commercial masters, retrieved for local evaluation only and never committed.
#
# Manifest-driven, so anyone holding corpus/ust-manifest.json can reproduce the
# corpus (as fetch-vocadito.sh does for Vocadito).
#
# Idempotent: a song whose WAV exists with the expected duration is skipped.
# Verified: each download's duration must match the manifest's
# original_duration_s within ±5 s, or it is rejected (wrong video / cut).
#
# Usage:  crates/sona-eval/scripts/fetch-pifuyuu-audio.sh
# Env:    SONA_UST_AUDIO_DIR (default crates/sona-eval/data/pifuyuu-audio),
#         SONA_UST_MANIFEST  (default crates/sona-eval/corpus/ust-manifest.json)
# Needs:  yt-dlp, ffmpeg, ffprobe, python3
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="${SONA_UST_MANIFEST:-$script_dir/../corpus/ust-manifest.json}"
dest="${SONA_UST_AUDIO_DIR:-$script_dir/../data/pifuyuu-audio}"
mkdir -p "$dest"
for tool in yt-dlp ffmpeg ffprobe python3; do
  command -v "$tool" >/dev/null || { echo "error: $tool is required" >&2; exit 1; }
done

duration() { ffprobe -v error -show_entries format=duration -of csv=p=0 "$1"; }
close_enough() { python3 -I -c 'import sys; sys.exit(0 if abs(float(sys.argv[1]) - float(sys.argv[2])) <= 5 else 1)' "$1" "$2"; }

failed=0
while IFS=$'\t' read -r id url want; do
  wav="$dest/$id.wav"
  if [ -f "$wav" ] && close_enough "$(duration "$wav")" "$want"; then
    echo "  cached  $id"
    continue
  fi
  echo "  fetch   $id  <- $url"
  tmp="$(mktemp -d)"
  if ! yt-dlp --no-update --quiet --no-warnings --no-playlist -f bestaudio \
         -o "$tmp/src.%(ext)s" "$url"; then
    echo "  error   $id: download failed" >&2; failed=1; rm -rf "$tmp"; continue
  fi
  # 44.1 kHz stereo PCM: demucs's native rate; mono-ing happens after separation.
  ffmpeg -nostdin -loglevel error -y -i "$tmp"/src.* -ar 44100 -ac 2 -c:a pcm_s16le "$tmp/out.wav"
  got="$(duration "$tmp/out.wav")"
  if ! close_enough "$got" "$want"; then
    echo "  error   $id: duration ${got}s, manifest says ${want}s — wrong video or a different cut" >&2
    failed=1; rm -rf "$tmp"; continue
  fi
  mv "$tmp/out.wav" "$wav"; rm -rf "$tmp"
done < <(python3 -I -c '
import json, sys
for s in json.load(open(sys.argv[1], encoding="utf-8"))["songs"]:
    print(s["id"], s["original_url"], s["original_duration_s"], sep="\t")' "$manifest")

[ "$failed" -eq 0 ] || { echo "some songs failed; rerun to retry (cached songs are skipped)" >&2; exit 1; }
echo "Original audio ready at $dest"
echo "Next: crates/sona-eval/scripts/separate-stems.sh"
