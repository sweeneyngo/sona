# 14 — The dev harness's f0/salience cache doesn't reproduce fresh results exactly

Status: ready-for-agent
Type: task
Blocked by: utau-edit-cost 03 (it is adding a GAME cache to sona-eval-cellstage; do this after, on top)

## Evidence

A07: the same `sona-eval-ust-dev` binary gives grid COnPOff 0.387 from a cold cache and 0.388 from a warm one
on dev. The cached f0/salience values are not bit-identical to freshly computed ones (likely a lossy
serialisation, e.g. f32 or text rounding). Close A/B comparisons can flip on cache state alone.

## Proposal

Find the lossy step (serialisation format, float width, ordering); store values losslessly (e.g. raw f64
little-endian, or exact text) and key the cache by the producing code's version. Add a test: a cold run and a
warm run of the harness on a short stem give identical scores and identical per-song outputs.

## How to tell

Cold vs warm dev runs identical to the last digit; a mutant that rounds on write fails the test.

## Comments
