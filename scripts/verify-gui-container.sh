#!/usr/bin/env bash
# Visual smoke-test harness for the disposable Docker verification container only.
set -euo pipefail
[[ -f /.dockerenv ]] || { echo 'Container-only harness'; exit 1; }
export DISPLAY=:93
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp/deadair-runtime}
# Docker lacks the user namespace support needed by WebKit's inner sandbox.
# This override is not used by the application or installed desktop entry.
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1
Xvfb "$DISPLAY" -screen 0 1600x1080x24 > /tmp/deadair-xvfb.log 2>&1 &
xvfb_pid=$!
trap 'kill "$xvfb_pid" 2>/dev/null || true' EXIT
for attempt in {1..30}; do
  [[ ! -S /tmp/.X11-unix/X93 ]] || break
  sleep 0.1
done
dbus-run-session "${DEADAIR_TEST_BINARY:-/build/target/release/dead-air}" > /tmp/deadair-gui.log 2>&1
