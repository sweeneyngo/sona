#!/usr/bin/env bash
# Run the editor browser drivers (web/e2e) against a Vite dev server, as CI does.
#
# Needs web/node_modules (`npm ci`) and a Python with Playwright and Chromium. No `sona serve`: the drivers
# that need it are skipped, and the editor's failed /api probe is not counted as a console error.
#
#   SONA_E2E_PORT    the dev server's port (default 5180; 5173 is left to `npm run dev`)
#   SONA_E2E_OUT     screenshots, downloads, the fixture and vite.log (default: a fresh temp dir)
#   SONA_E2E_PYTHON  a Python with Playwright (default python3)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
port="${SONA_E2E_PORT:-5180}"
export SONA_E2E_OUT="${SONA_E2E_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/sona-e2e.XXXXXX")}"
export SONA_E2E_EDITOR_URL="http://127.0.0.1:$port/"
export SONA_E2E_SKIP_SERVE=1
mkdir -p "$SONA_E2E_OUT"
echo "out: $SONA_E2E_OUT"

cd "$root/web"
node node_modules/vite/bin/vite.js --host 127.0.0.1 --port "$port" --strictPort >"$SONA_E2E_OUT/vite.log" 2>&1 &
vite=$!
trap 'kill "$vite" 2>/dev/null || true; wait "$vite" 2>/dev/null || true' EXIT

for _ in $(seq 120); do
  curl -fsS -o /dev/null "$SONA_E2E_EDITOR_URL" 2>/dev/null && break
  kill -0 "$vite" 2>/dev/null || { echo "error: vite exited" >&2; cat "$SONA_E2E_OUT/vite.log" >&2; exit 1; }
  sleep 0.5
done
curl -fsS -o /dev/null "$SONA_E2E_EDITOR_URL" || { echo "error: vite not ready on :$port after 60 s" >&2; cat "$SONA_E2E_OUT/vite.log" >&2; exit 1; }
# Compile the app once before the first driver's page load waits on it.
curl -fsS -o /dev/null "${SONA_E2E_EDITOR_URL}main.ts"

"$root/web/e2e/run_all.sh"
