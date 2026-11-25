#!/usr/bin/env bash
# Lyrics-first 14: align every dev song's pasted lines to its lead stem with each candidate, score the lines,
# and write the `--lyric-line-times` files. Everything it writes is lyric-derived and stays in $L14_WORK.
#
#   L01_DUMP=<cellstage --dump of dev> L01_WORK=<01's work dir, with lines/> L14_WORK=<scratch dir> \
#   SONA_STEMS=crates/sona-eval/data/pifuyuu-stems crates/sona-eval/scripts/line-alignment/run_all.sh
#
# Venvs: ~/.cache/sona/line-align/venv (torch 2.14 CPU, transformers 4.57) for the CTC models, venv-whisper
# (torch 2.11, stable-ts 2.19, faster-whisper 1.2) for Whisper. Models already downloaded by ticket 05 are read
# from ~/.cache/sona/lyric-asr/hf, read only. On a loaded machine run one process per song instead (2–4 threads
# each): torch's intra-op threads stall on an oversubscribed CPU.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
venv="$HOME/.cache/sona/line-align/venv/bin/python"
wvenv="$HOME/.cache/sona/line-align/venv-whisper/bin/python"
python3 "$here/texts.py"
"$venv" -I "$here/lead_rms.py"
for m in lyrics hira jg mms; do
  "$venv" -I "$here/ctc_align.py" --model "$m" --stem lead
done
"$venv" -I "$here/ctc_align.py" --model lyrics --stem vocals
"$wvenv" -I "$here/whisper_align.py" --name whisper-large-v3-lead \
  --model "$(echo "$HOME"/.cache/sona/lyric-asr/hf/hub/models--Systran--faster-whisper-large-v3/snapshots/*)"
for end in own next energy ust; do
  python3 "$here/score.py" lyrics-lead lyrics-vocals hira-lead jg-lead mms-lead whisper-large-v3-lead --end "$end" --per-song \
    --dump-rows "$L14_WORK/rows-$end.json"
done
# The Rust landing reproduces the prototype (0 rows differ), e.g. for the chosen aligner and end rule:
#   sona-eval-cellstage --model ~/.cache/sona/cellstage-D --edit-cost --lyric-placement --lyric-lines "$L01_WORK/lines" \
#       --lyric-line-times "$L14_WORK/times/lyrics-lead-energy" > rust.out
#   python3 "$here/check_port.py" rust.out "$L14_WORK/rows-energy.json" lyrics-lead-energy
