#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target \
  -v "$project_root:/work:Z" -w /work/src-tauri \
  rust:1.95-slim@sha256:e14e87345b4d5964ddcc3491d27ee046a0f23820f340c3c1e24da6880141f7c0 \
  cargo build --locked --no-default-features --example ui_driver
