# Building and testing on Linux

This guide covers Ubuntu, Debian, Fedora, and Arch Linux. The validated native build environment was Debian 12 x86_64 with WebKitGTK 4.1. Instructions for other distributions are provided, but Fedora/Arch installation and physical Wi-Fi behavior still need acceptance testing.

## Requirements

- A Linux desktop with WebKitGTK 4.1, system D-Bus, and a graphical polkit authentication agent.
- NetworkManager for native discovery and connection management.
- A Wi-Fi adapter and driver supporting monitor mode for passive capture. A built-in card is allowed after explicit consent to disconnect it temporarily.
- For Academy's real virtual WPA2 lab: a kernel providing `mac80211_hwsim` and network namespaces. No physical Wi-Fi adapter is required for this lab.
- Node.js 22.12+ and Rust for development. The bootstrap script can install missing toolchains.

The GUI must run as a regular user. Install the helper and polkit policy through the application package or the developer helper installer.

## Build packages

```sh
git clone https://github.com/Japhet17429475581/DEAD-AIR.git
cd DEAD-AIR
bash scripts/bootstrap.sh --dry-run
bash scripts/bootstrap.sh
bash scripts/package.sh
```

The bootstrap script displays its package plan and asks before installation. It uses signed distribution repositories for system packages. Missing Rust/Node toolchains are fetched from their official HTTPS endpoints and checked against published SHA-256 checksums. Developer downloads go into `.tools/` where applicable; `npm ci` and `cargo fetch --locked` use the committed lockfiles.

| Distribution | Build package manager | Runtime wireless packages | Optional CPU OpenCL |
| --- | --- | --- | --- |
| Ubuntu 22.04+ / Debian 12+ | `apt-get` | `network-manager`, `iw`, `iproute2`, `aircrack-ng` | `pocl-opencl-icd`, `clinfo` |
| Fedora with WebKitGTK 4.1 | `dnf` | `NetworkManager-wifi`, `iw`, `iproute`, `aircrack-ng` | `pocl`, `clinfo` |
| Arch Linux | `pacman` | `networkmanager`, `iw`, `iproute2`, `aircrack-ng` | `pocl`, `clinfo` |

The exact build-library list is in [`scripts/bootstrap.sh`](../scripts/bootstrap.sh). Hashcat is optional. Keep Arch's packages in a consistent updated state; the application does not perform a full distribution upgrade.

`package.sh` first builds the privileged helper, places it in the ignored `packaging/bin/` directory, then builds the GUI and packages. DEB/RPM output is under `src-tauri/target/release/bundle/`, or `$CARGO_TARGET_DIR/release/bundle/` if set. On Arch the script invokes the local package recipe; `makepkg` must run as an ordinary user. No prebuilt helper binary is stored in Git.

## Install and start

Open the generated DEB/RPM using your distribution's package installer. On Arch, install the locally built package. Launch **DEAD AIR** from the application menu.

The package installs the helper at `/usr/libexec/deadair-capture-helper` and its policy under `/usr/share/polkit-1/actions/`. The application inventories runtime dependencies on startup. Review the list and choose **INSTALL DEPENDENCIES** to install missing tools after system authorization. **INSTALL CPU RUNTIME** can install PoCL for Hashcat. GPU drivers and wordlists are not fetched.

The scoped daemon starts on demand and receives authorization for a working session. It does not store an administrator password. Package-manager locks, missing services, canceled authorization, and network failures are reported as errors.

Academy lesson 15 has a separate **INSTALL LAB TOOLS** profile for `hostapd`, `wpa_supplicant`, and the remaining virtual-lab tools on all four distributions. Its preflight checks the running kernel's hwsim module; the application does not silently replace the kernel. See [virtual-lab requirements, architecture and QEMU integration test](VIRTUAL-LABS.md).

Packages built on a newer distribution can require newer glibc or system libraries. Do not assume a Debian-built binary is compatible with every Linux distribution. AppImage and Windows/macOS native radio backends are not currently provided.

## Development

```sh
bash scripts/dev.sh
```

For privileged workflows in a development checkout, build and install the helper once:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --locked --release --no-default-features --bin deadair-capture-helper
bash scripts/install-helper.sh
```

If using a custom target directory, pass the helper binary path as the install script's first argument. Do not launch the GUI with `sudo`.

For frontend-only work on any supported Node development host:

```sh
npm ci
npm run dev
```

Visit http://127.0.0.1:1420/. Browser mode cannot call the Linux backend. Sample networks require explicit DEMO mode; Academy simulations do not require radio access.

## Automated checks

```sh
export PATH="$PWD/.tools/node/bin:$PATH"
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
bash scripts/doctor.sh
```

Install Google Chrome before running `npm run test:e2e`; the Playwright configuration uses its `chrome` channel. Browser scenarios can validate simulated states and IPC contracts but do not validate actual radios or polkit dialogs.

With Hashcat and an actual OpenCL runtime installed, run the explicitly ignored integration tests separately:

```sh
POCL_MAX_PTHREAD_COUNT=4 cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features --test hashcat_real -- --ignored --test-threads=1
```

These tests exercise the CPU/GPU runtime and can produce substantial compute load. They cover a SHA-256 benchmark, an owned known-answer vector, cancellation, and temporary-file cleanup.

## Physical hardware acceptance

Before using the application for an assessment, exercise the following on each target distribution and representative driver:

1. **Setup:** test dependency installation, canceled authorization, offline failure, and package-manager contention in a normal desktop session.
2. **Discovery:** compare native observations with NetworkManager; unplug the adapter and verify an error or empty result, with no sample-data fallback.
3. **Capture:** use your own AP and consenting client. Observe the selected BSSID and compare frames, counters, and exported PCAPNG with Wireshark. EAPOL correlation must not be presented as cryptographic validation.
4. **Connectivity:** check STOP, END SESSION, normal window close, USB removal, SIGTERM, and crash recovery. For a built-in card, verify the original NetworkManager profile actually reconnects. Keep a local recovery path while testing disconnection.
5. **MAC profiles:** verify the assigned address and actual restoration; reject operations on unavailable or concurrently changed devices.
6. **Storage:** verify temporary cleanup, persistent retention, tmpfs validation, and refusal to follow unknown files/links. Cleanup failures must remain visible.
7. **Desktop:** check X11/Wayland, keyboard navigation, scaling, reduced motion, sound-off behavior, and accessibility.

CI builds and unit tests complement this checklist; they do not replace it. Existing milestone documents under `docs/` contain historical results and may refer to local artifacts excluded from Git.
