#!/bin/sh
# Fail if the repo tracks corpus data it must not redistribute.
#
# Pifuyuu's USTs forbid redistribution (crates/sona-eval/corpus/ust-manifest.json,
# `ust_terms`), the source songs are commercial recordings, and Vocadito is
# licence-gated. All of it lives in the git-ignored crates/sona-eval/data/. The
# only audio or UST files the repo may track are the synthetic fixtures under
# crates/*/fixtures/ and crates/*/tests/fixtures/.
set -eu
cd "$(git rev-parse --show-toplevel)"
bad=$(git ls-files \
  | grep -E '^crates/sona-eval/data/|\.(ust|wav|mp3|flac|m4a|ogg|opus|zip|7z|ustx|vsqx)$' \
  | grep -vE '^crates/[^/]+/(tests/)?fixtures/' || true)
if [ -n "$bad" ]; then
  echo "error: these tracked files look like corpus data (see scripts/ci/check-no-corpus-data.sh):" >&2
  echo "$bad" | sed 's/^/  /' >&2
  exit 1
fi
echo "ok: no corpus data tracked"
