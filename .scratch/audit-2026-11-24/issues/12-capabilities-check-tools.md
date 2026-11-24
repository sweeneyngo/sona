# 12 — Capabilities should check ffmpeg, the UVR model and demucs weights; surface tempo errors

Status: ready-for-agent
Type: task
Blocked by: —

## Evidence

A05 left these (`05-serve-worker-failure-modes.md` Answer): `/api/capabilities` reports separation as
available without checking that ffmpeg runs or that the UVR model file and demucs weights exist, so a first
job fails late; the client swallows a 422 from `/api/tempo`, so a bad file shows no reason.

## Proposal

Probe ffmpeg and the model files at start-up (cheap, cached), report missing pieces in capabilities with the
fix (`scripts/setup-separators.sh`), show them on the start screen; show the `/api/tempo` error text.

## How to tell

serve tests with a missing ffmpeg / model path; a vitest for the 422 path.

## Comments
