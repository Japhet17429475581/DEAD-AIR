#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
[[ ! -f ${CARGO_HOME:-$HOME/.cargo}/env ]] || source "${CARGO_HOME:-$HOME/.cargo}/env"
export PATH="$root/.tools/node/bin:$PATH"
cd "$root"
mkdir -p packaging/bin
cargo build --manifest-path src-tauri/Cargo.toml --locked --release --no-default-features --bin deadair-capture-helper
target=${CARGO_TARGET_DIR:-"$root/src-tauri/target"}
install -m 0755 "$target/release/deadair-capture-helper" packaging/bin/deadair-capture-helper
source /etc/os-release
if [[ $ID == arch ]]; then
  npm run tauri build -- --no-bundle
  bash scripts/package-arch.sh
else
  npm run tauri build
fi
echo "Packages: $target/release/bundle"
