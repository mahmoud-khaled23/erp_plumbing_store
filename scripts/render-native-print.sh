#!/usr/bin/env bash
# Container entry point. Use check-native-print.sh from the Fedora host.
set -euo pipefail
Xvfb :99 -screen 0 1280x900x24 -nolisten tcp -ac > /tmp/erp-xvfb.log 2>&1 &
display_pid=$!
trap 'kill "$display_pid" 2>/dev/null || true' EXIT
export DISPLAY=:99
for attempt in {1..50}; do
  test -S /tmp/.X11-unix/X99 && break
  kill -0 "$display_pid"
  sleep 0.1
done
python3 scripts/verify_print_pdfs.py
