#!/bin/bash
# utau-edit-cost 19's final CV (fold assignment F: 10 folds by song, permutation seed 1). Env: PY (a Python
# with torch, numpy, onnx, onnxruntime), BIN (sona-eval-cellstage built with --features game, plus its own
# ORT_DYLIB_PATH, SONA_GAME_DIR, SONA_DEV_CACHE, SONA_UST_DIR, SONA_UST_STEMS_DIR), CELLS (a --dump-cells
# directory covering every spent split) and CV (an output directory). Both hold UST-derived data and
# weights: keep them out of the repository.
#   final19.sh train RECIPE_SPEC...      train each recipe's ten fold models (cv19.py, SAVE=1)
#   final19.sh export NAME SWITCHES      export them, with one switch cost per fold (comma-separated; 0 = argmax)
#   final19.sh score NAME                score every spent split through the Rust scorer with --fold-models
set -euo pipefail
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
: "${PY:?}" "${BIN:?}" "${CELLS:?}" "${CV:?}"
REASON="utau-edit-cost 19: song-level CV of the stage retrain (out-of-sample fold models on spent songs); diagnosis only, confirms nothing"
mkdir -p "$CV"
case "$1" in
  train)
    shift
    SAVE=1 SONA_SCRIPTS="$HERE/.." "$PY" -I "$HERE/cv19.py" "$CELLS" "$CV" 10 1 "$@"
    ;;
  export)
    IFS=, read -ra sw <<< "$3"
    for k in $(seq 0 9); do
      "$PY" -I "$HERE/export_onnx.py" "$CV/$2/fold$k.pt" "$CV/$2/models/fold$k" "${sw[$k]}" | grep -E "parity|wrote"
    done
    ;;
  score)
    "$BIN" --model "$CV/$2/models/fold0" --fold-models "$CV/$2/models" --edit-cost > "$CV/$2/rust_dev.txt"
    for s in holdout holdout2 holdout3; do
      "$BIN" --model "$CV/$2/models/fold0" --fold-models "$CV/$2/models" --edit-cost --split "$s" --reread --reason "$REASON" > "$CV/$2/rust_$s.txt"
    done
    ;;
esac
