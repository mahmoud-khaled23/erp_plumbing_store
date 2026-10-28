#!/usr/bin/env bash
set -euo pipefail
if [[ "${1:-}" != --accept-sdk-license ]]; then
  echo 'Read the Microsoft SDK/CRT terms: https://go.microsoft.com/fwlink/?LinkId=2086102' >&2
  echo 'After accepting them, run: bash scripts/build-windows-fedora.sh --accept-sdk-license' >&2
  exit 2
fi
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
docker build -f "$project_root/docker/Dockerfile.windows-release" \
  --build-arg ACCEPT_MICROSOFT_SDK_LICENSE=yes -t plumbing-erp-windows:stage5 "$project_root"
# Linux Node dependencies are shared with Fedora, but no host toolchain is changed.
docker run --rm --user "$(id -u):$(id -g)" -v "$project_root:/work:Z" \
  plumbing-erp-windows:stage5 bash scripts/build-windows-container.sh
