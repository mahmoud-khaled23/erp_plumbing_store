#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
test -f "$project_root/output/print-fixtures/invoice-a4.html" || {
  echo 'Run npm run test:ui -- tests/ui/printing.spec.ts first.' >&2
  exit 1
}
docker run --rm --user "$(id -u):$(id -g)" \
  -e CARGO_HOME=/work/output/cargo -e CARGO_TARGET_DIR=/work/output/target \
  -e CARGO_BUILD_JOBS=2 -v "$project_root:/work:Z" -w /work/src-tauri \
  plumbing-erp-print:stage4 cargo build --locked --features desktop --example native_print_check "$@"
# A disposable, network-isolated test container; the installed app keeps WebKit's sandbox.
docker run --rm --network none --user "$(id -u):$(id -g)" --shm-size=512m \
  -e XDG_CACHE_HOME=/tmp/erp-cache -e XDG_DATA_HOME=/tmp/erp-data \
  -e WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1 \
  -v "$project_root:/work:Z" -w /work plumbing-erp-print:stage4 \
  bash scripts/render-native-print.sh
