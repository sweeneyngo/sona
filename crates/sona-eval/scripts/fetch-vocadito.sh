#!/usr/bin/env bash
# Fetch the Vocadito corpus (Zenodo record 5578807) into the git-ignored local
# data dir. The corpus is NOT committed (size + licensing, ADR-0011); this
# populates it so the harness's --corpus path and the #[ignore]d integration
# test can run.
#
# Usage:
#   crates/sona-eval/scripts/fetch-vocadito.sh
#   SONA_VOCADITO_DIR=/somewhere crates/sona-eval/scripts/fetch-vocadito.sh
set -euo pipefail

RECORD_URL="https://zenodo.org/api/records/5578807/files/vocadito.zip/content"
EXPECTED_MD5="dea40fd18f14d899643c4ba221b33a46"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="${SONA_VOCADITO_DIR:-$script_dir/../data/vocadito}"
mkdir -p "$dest"
zip_path="$dest/vocadito.zip"

echo "Downloading vocadito.zip (~58 MB) -> $zip_path"
curl -L --fail --progress-bar -o "$zip_path" "$RECORD_URL"

echo "Verifying md5 (expect $EXPECTED_MD5)..."
if command -v md5sum >/dev/null 2>&1; then
  actual="$(md5sum "$zip_path" | awk '{print $1}')"
elif command -v md5 >/dev/null 2>&1; then
  actual="$(md5 -q "$zip_path")"
else
  echo "error: neither md5sum nor md5 is available to verify the download" >&2
  exit 1
fi
if [ "$actual" != "$EXPECTED_MD5" ]; then
  echo "error: md5 mismatch (got $actual, want $EXPECTED_MD5) — download corrupt" >&2
  rm -f "$zip_path"
  exit 1
fi

echo "Extracting..."
unzip -o -q "$zip_path" -d "$dest"
rm -f "$zip_path"

echo "Vocadito ready at $dest"
echo "Next: cargo run -p sona-eval --bin sona-eval-report -- --corpus"
