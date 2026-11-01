# Changelog

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
