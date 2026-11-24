# 13 — Run the editor browser drivers in CI

Status: resolved (pending a green CI run on the pushed branch)
Type: task
Blocked by: —

## Evidence

Since utau-09, `web/e2e/run_all.sh` passes on a clean checkout with `SONA_E2E_SKIP_SERVE=1` and nothing
else set (a generated fixture; 10 editor drivers). They still run only locally (roadmap 6.4), so a UI
regression reaches `main` unseen.

## Proposal

A CI job: Node + Python, `pip install playwright` pinned, `playwright install --with-deps chromium`, build or
`vite` on a fixed port, `SONA_E2E_SKIP_SERVE=1 web/e2e/run_all.sh`; upload screenshots on failure. Pin
versions like the rest of CI. Keep it under a few minutes or run it on a separate workflow.

## How to tell

The job passes on `main`; a deliberately broken handle hit-test on a branch fails it.

## Comments

## Answer

Branch `audit-a13-e2e-ci`. **Resolved pending the first CI run**: the job has run only locally so far.

- **Job `e2e`** in `.github/workflows/ci.yml`, beside `web`. It uses Node 22 (as `web` does) and `npm ci`,
  Python 3.13 with **`playwright==1.62.0`** (released 2026-07-31; the version the drivers were verified on
  locally), and `python -m playwright install --with-deps chromium`. Then it runs `scripts/ci/e2e.sh`. On
  failure it uploads `$SONA_E2E_OUT` (screenshots, downloads, `vite.log`, but not the synthetic fixture) as
  the `e2e-screenshots` artifact. Timeout: 20 min.
- **`scripts/ci/e2e.sh`** runs the same thing locally and in CI. It starts the Vite dev server
  (`node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 5180 --strictPort`, port overridable with
  `SONA_E2E_PORT`) in the background, polls it for up to 60 s and requests `main.ts` once so the first
  driver doesn't wait on compilation. It then runs `SONA_E2E_SKIP_SERVE=1 web/e2e/run_all.sh` with
  `SONA_E2E_EDITOR_URL` pointing at the server, and kills only the server PID it started (an `EXIT` trap).
  `SONA_E2E_OUT` defaults to a fresh temp dir. The dev server is used rather than a preview of the build
  because it is what the drivers were written and checked against (README).
- **No `sona serve`:** the app's `/api/capabilities` probe goes through Vite's proxy to 127.0.0.1:8977 and
  fails (29 `ECONNREFUSED` lines in `vite.log` on the local run). `_env.watch` drops console errors whose
  location is an `/api/` URL, so no driver counted them. `song_to_notes` and `cancel_and_retry` are skipped.
  They are what remains of roadmap 6.4 (`sona serve` with stub separators).
- **Local pass** (fresh `SONA_E2E_OUT`, port 5199, Playwright 1.62.0, Chromium 1255): all ten editor drivers
  passed, exit 0, 4 min 0 s wall (28% CPU, so most of it is the drivers' waits). Afterwards nothing was
  listening on the port.
- **Mutant:** `handleAtPoint` in `web/main.ts` skipped `vibrato-period` handles. `pitch_handles` failed with
  `FAIL  period handle →: period 175 → 175` (then an `IndexError` on the next drag). The other nine passed
  and the script exited 1. Restored afterwards.
- **Expected CI time:** about 1–2 min of setup (cached npm, pip, Chromium + apt deps) plus about 4 min of
  drivers, so 5–7 min. It runs in parallel with the other jobs.
- Docs: `docs/agents/rules.md` (enforced table, commands), `docs/dev-workflow.md` (verify table, ship),
  `docs/roadmap.md` 6.4 (editor drivers done; the serve drivers remain), `web/e2e/README.md`.
