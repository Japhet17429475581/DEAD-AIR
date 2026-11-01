#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
[[ ! -f ${CARGO_HOME:-$HOME/.cargo}/env ]] || source "${CARGO_HOME:-$HOME/.cargo}/env"
export PATH="$root/.tools/node/bin:$PATH"
cd "$root"
npm run tauri dev
