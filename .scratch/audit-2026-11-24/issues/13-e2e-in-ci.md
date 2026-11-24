# 13 — Run the editor browser drivers in CI

Status: ready-for-agent
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
