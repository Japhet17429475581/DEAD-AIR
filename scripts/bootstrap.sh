#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dry=false
[[ ${1:-} != --dry-run ]] || dry=true
[[ $(uname -s) == Linux ]] || { echo 'Build the native application on Linux.' >&2; exit 1; }
source /etc/os-release
case "$ID" in
  ubuntu|debian) manager=apt-get; packages=(build-essential pkg-config curl ca-certificates file xz-utils libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev patchelf python3 pkexec dbus);;
  fedora) manager=dnf; packages=(gcc gcc-c++ make pkgconf-pkg-config curl ca-certificates file xz webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel libxdo-devel patchelf python3 polkit dbus-daemon);;
  arch) manager=pacman; packages=(base-devel pkgconf curl ca-certificates file xz webkit2gtk-4.1 libappindicator-gtk3 librsvg openssl xdotool patchelf python polkit dbus);;
  *) echo "Unsupported distribution: $ID" >&2; exit 1;;
esac
printf 'Distribution: %s\nPackage manager: %s\nBuild packages: %s\n' "$ID" "$manager" "${packages[*]}"
echo 'Wireless and optional Hashcat runtime packages can be installed inside the application.'
if $dry; then echo 'Dry run: no package changes, downloads, or writes.'; exit 0; fi
read -r -p 'Install/check these build packages using signed system repositories? [y/N] ' consent
[[ $consent == y || $consent == Y ]] || { echo 'Cancelled.'; exit 1; }
case "$manager" in
  apt-get) sudo apt-get update; sudo apt-get install -y --no-install-recommends "${packages[@]}";;
  dnf) sudo dnf install -y "${packages[@]}";;
  pacman) sudo pacman -S --needed "${packages[@]}";;
esac
pkg-config --modversion webkit2gtk-4.1
mkdir -p "$root/.tools"
arch=$(uname -m)
case "$arch" in x86_64) nodearch=x64; rustarch=x86_64;; aarch64) nodearch=arm64; rustarch=aarch64;; *) echo "Unsupported architecture: $arch"; exit 1;; esac
if ! command -v cargo >/dev/null; then
  echo 'Installing minimal Rust from the official HTTPS endpoint, with SHA-256 verification.'
  base="https://static.rust-lang.org/rustup/dist/$rustarch-unknown-linux-gnu"
  curl --fail --location --proto '=https' --tlsv1.2 "$base/rustup-init" -o "$root/.tools/rustup-init"
  curl --fail --location --proto '=https' --tlsv1.2 "$base/rustup-init.sha256" -o "$root/.tools/rustup-init.sha256"
  expected=$(cut -d' ' -f1 "$root/.tools/rustup-init.sha256")
  printf '%s  %s\n' "$expected" "$root/.tools/rustup-init" | sha256sum --check -
  chmod 0700 "$root/.tools/rustup-init"
  "$root/.tools/rustup-init" -y --profile minimal --no-modify-path
  source "${CARGO_HOME:-$HOME/.cargo}/env"
fi
if ! command -v node >/dev/null || ! node -e 'const [a,b]=process.versions.node.split(".").map(Number);process.exit(a>22||(a===22&&b>=12)?0:1)'; then
  echo 'Installing local Node 22 LTS from the official HTTPS endpoint, verified against SHASUMS256.txt.'
  base=https://nodejs.org/dist/latest-v22.x
  curl --fail --location --proto '=https' --tlsv1.2 "$base/SHASUMS256.txt" -o "$root/.tools/node-SHASUMS256.txt"
  file=$(awk -v suffix="linux-$nodearch.tar.xz" 'index($2,suffix) && $2 !~ /headers/ {print $2;exit}' "$root/.tools/node-SHASUMS256.txt")
  [[ $file =~ ^node-v[0-9.]+-linux-(x64|arm64)\.tar\.xz$ ]] || { echo 'Unexpected Node artifact name'; exit 1; }
  curl --fail --location --proto '=https' --tlsv1.2 "$base/$file" -o "$root/.tools/$file"
  (cd "$root/.tools"; awk -v file="$file" '$2==file' node-SHASUMS256.txt | sha256sum --check -)
  mkdir -p "$root/.tools/node"
  tar -xf "$root/.tools/$file" -C "$root/.tools/node" --strip-components=1
  export PATH="$root/.tools/node/bin:$PATH"
fi
cd "$root"
npm ci
cargo fetch --manifest-path src-tauri/Cargo.toml --locked
echo 'Setup complete. Start development with ./scripts/dev.sh; build packages with ./scripts/package.sh.'
