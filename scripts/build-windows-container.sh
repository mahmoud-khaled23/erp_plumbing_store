#!/usr/bin/env bash
set -euo pipefail
test -f node_modules/@tauri-apps/cli/package.json || {
  echo 'Run npm ci on Fedora before the container build.' >&2
  exit 1
}
npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross
