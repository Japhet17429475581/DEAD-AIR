# Changelog

## 0.4.0

- Add a native Rust virtual-WLAN runtime and Academy lesson 15: isolated hwsim AP, client and observer, real WPA2 association, captured M1–M4 and client-to-AP ping. Completion requires observed evidence.
- Create radios through generic netlink with socket-scoped leases, separate client IP namespace, no uplink and no physical-interface input. Add bounded processes/capture/logs, a ten-minute lifetime, owned cleanup and stale-worker recovery.
- Integrate virtual-lab shutdown with session close, window close and PANIC; allow explicit retained-frame PCAPNG export to a file-capable audit session.
- Add a separate GUI dependency profile for Ubuntu, Debian, Fedora and Arch. Report missing kernel support instead of substituting sample results.
- Make calm crimson the default theme, with neutral text, muted accents and bloom/scanlines disabled. Retain the original phosphor-green CRT theme in Settings.
- Document the pinned WifiForge architecture review, independent Rust implementation and scenario roadmap. Add a real QEMU/Linux integration harness alongside GUI IPC checks.

## 0.3.3

- Replaced unconditional waiting-for-client status with evidence-based receiver health, elapsed/remaining time, raw PCAP records, file size, actual frequency, beacons and EAPOL counters.
- Added helper progress reports, stale/missing report detection, and visible channel-drift failure. No telemetry/timer creates packet events.
- Record all target beacons; normal AP reception no longer depends on a client authenticating.
- Preserve explicit stop reasons and distinguish no target frames, no EAPOL, and partial exchanges after normal completion.
- Allow connected managed cards, including built-in cards, for MAC profiles after disconnection consent. Keep NetworkManager released during the lease and restore the saved profile afterward.
- Preserve MAC errors/rollback diagnostics and allow retry after a cleanly rejected preflight.
- Added actual airodump-ng offline output compatibility coverage and GUI tests for silent receivers and built-in MAC consent. Physical hardware validation remains outstanding.

## 0.3.2

- Fixed rejection of existing monitor interfaces, including those not represented by NetworkManager.
- Use the selected managed interface itself for both idle and connected-card capture; no additional virtual-interface support is required. Connected cards still require disconnection consent.
- Restore borrowed monitor channel width/centers and link state without deleting the interface or converting it to managed mode. Older app-owned recovery records retain their original behavior.
- Block new capture when an unfinished app journal on the radio still requires recovery.
- Preserve bounded system-command and airodump-ng stderr diagnostics, and show capture failures next to the launch controls.
- Added adapter refresh, stale interface-selection correction, and native/browser regression coverage.

## 0.3.1

- Expanded Academy to 14 modules with a dedicated deauthentication/disassociation and PMF lesson.
- Added a four-question quiz and a four-case offline station-state lab.
- Added model tests and browser checks covering progress and the absence of radio/privileged calls from the lab.
- Real deauth transmission remains outside the application scope.

## 0.3.0

- Allowed capture using a connected built-in Wi-Fi card with explicit temporary-disconnection consent and saved-profile restoration.
- Added the Academy learning path with theory, quizzes, and labs.
- Added passive PMKID observations and a real known-answer WebCrypto teaching sandbox.
- Physical adapter transitions and reconnection still require hardware validation.

## 0.2.0

- Added a scoped privileged daemon with one polkit authorization per working session.
- Added private/vendor MAC address profiles and restoration controls.
- Added configurable PANIC cleanup and verified tmpfs sessions.

## 0.1.0

- Introduced the Rust/Tauri workstation, CRT interface, and animated ASCII skull.
- Implemented native discovery, passive capture integration, packet parsing, sessions, dependency setup, and Hashcat laboratory workflows.
- Built Linux DEB/RPM packages and exercised the installed GUI in a Debian/WebKitGTK environment.

Detailed reports distinguish implemented functionality from hardware acceptance. See [README](README.md#verification-and-current-limits) and the milestone documents in `docs/`.
