#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
target=${CARGO_TARGET_DIR:-"$root/src-tauri/target"}
stage="$target/release/bundle/arch"
command -v makepkg >/dev/null || { echo 'Run this script on Arch with base-devel installed.' >&2; exit 1; }
[[ $EUID -ne 0 ]] || { echo 'makepkg must run as a regular user.' >&2; exit 1; }
mkdir -p "$stage"
install -m 0755 "$target/release/dead-air" "$stage/dead-air"
install -m 0755 "$target/release/deadair-capture-helper" "$stage/deadair-capture-helper"
install -m 0644 "$root/packaging/arch/PKGBUILD" "$root/packaging/dead-air.desktop" "$root/packaging/io.deadair.capture.policy" "$root/LICENSE" "$stage/"
install -m 0644 "$root/src-tauri/icons/128x128.png" "$stage/dead-air.png"
cd "$stage"
makepkg --force
