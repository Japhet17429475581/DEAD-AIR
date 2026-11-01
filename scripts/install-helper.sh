#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
binary=${1:-"$root/src-tauri/target/release/deadair-capture-helper"}
[[ $(uname -s) == Linux ]] || { echo 'Linux required.' >&2; exit 1; }
[[ -f "$binary" && ! -L "$binary" ]] || { echo "Build the helper first: $binary" >&2; exit 1; }
if [[ $EUID -ne 0 ]]; then
  echo 'Installing the scoped helper and polkit policy requires administrator authorization.'
  exec pkexec /usr/bin/bash "$0" "$binary"
fi
install -d -m 0755 /usr/libexec
install -o root -g root -m 0755 "$binary" /usr/libexec/deadair-capture-helper
install -o root -g root -m 0644 "$root/packaging/io.deadair.capture.policy" /usr/share/polkit-1/actions/io.deadair.capture.policy
echo 'Installed helper. The GUI stays unprivileged.'
