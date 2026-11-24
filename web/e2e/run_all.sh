#!/usr/bin/env bash
# Run every browser driver; stop at the first failure. See README.md for what each needs.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
py="${SONA_E2E_PYTHON:-python3}"
editor=(bar_one pitch_handles pitch_pen sung_pitch_export lyrics_export fit_and_export stale_hover lite_autosave motion undo_redo help_and_grids doubtful_notes)
serve=(song_to_notes cancel_and_retry retranscribe lyric_start)
fail=0
run() {
  local out
  out="$(cd "$here" && "$py" "$1.py" 2>&1)" || { echo "FAIL  $1"; echo "$out" | tail -20; fail=1; return; }
  if echo "$out" | grep -q "console errors: none\|errors: none"; then echo "pass  $1"; else echo "FAIL  $1 (console errors)"; echo "$out" | tail -5; fail=1; fi
  echo "$out" | sed 's/^/      /' | grep -v "console errors" | tail -6
}
for d in "${editor[@]}"; do run "$d"; done
[ "${SONA_E2E_SKIP_SERVE:-}" = 1 ] || for d in "${serve[@]}"; do run "$d"; done
exit $fail
