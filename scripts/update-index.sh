#!/usr/bin/env bash
# Regenerates src-tauri/resources/index.json from SpriteCollab at a commit (default: current HEAD).
set -euo pipefail
cd "$(dirname "$0")/.."
SHA="${1:-$(git ls-remote https://github.com/PMDCollab/SpriteCollab.git HEAD | cut -f1)}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/$SHA/tracker.json" -o "$TMP/tracker.json"
curl -fsSL "https://raw.githubusercontent.com/PMDCollab/SpriteCollab/$SHA/credit_names.txt" -o "$TMP/credit_names.txt"
cargo run --quiet --manifest-path src-tauri/Cargo.toml --example build_index -- \
  "$TMP/tracker.json" "$TMP/credit_names.txt" "$SHA" > "$TMP/index.json"
mv "$TMP/index.json" src-tauri/resources/index.json
echo "Index updated to SpriteCollab $SHA"
