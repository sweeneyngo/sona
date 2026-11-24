#!/bin/bash
# Runs every reader evaluation on the 9 dev songs with lyric texts and UST dumps (dev only).
# Needs SONA_LANG_DUMP (03's dev dumps). Per-song outputs go to SONA_LANG_OUT, never the repo.
set -u
cd "$(dirname "$0")"
PY=~/.cache/sona/lang-units/bin/python
export SONA_LANG_OUT=${SONA_LANG_OUT:-$HOME/.cache/sona/lang-units/out}
mkdir -p "$SONA_LANG_OUT"
out=$SONA_LANG_OUT
IDS=(innocent-addict kamaitachi-ni-yoru telescope-nevermore atamaro-deluxe azemichi-ni-ochita-ginga yamineko my-axiom amaten fukagyakuteki-henyou)
M=(read read-raw pron pron-merge pron-opt)
rm -f "$out"/ALLDONE "$out"/*.json
$PY -I evalread.py --ids "${IDS[@]}" --unit lines12 --modes "${M[@]}" > "$out"/r-lines12-txt.log 2>&1
$PY -I evalread.py --ids "${IDS[@]}" --unit lines12 --input hira --modes "${M[@]}" > "$out"/r-lines12-hira.log 2>&1
$PY -I evalread.py --ids "${IDS[@]}" --unit lines12 --input readings --modes read pron-opt > "$out"/r-lines12-readings.log 2>&1
$PY -I evalread.py --ids "${IDS[@]}" --unit para --modes read pron-opt > "$out"/r-para-txt.log 2>&1
$PY -I evalread.py --ids "${IDS[@]}" --unit lines12 --modes read pron-opt --readers pyopenjtalk-plus fugashi+unidic-lite --latin2kana --only-latin --tag latin- > "$out"/r-latin.log 2>&1
touch "$out"/ALLDONE
