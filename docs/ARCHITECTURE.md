# Architecture and data handling

Version 0.3.2 supersedes the secondary-interface description below: capture uses the selected managed card itself, or borrows an existing monitor interface and restores its original channel/link state. See [adapter handling and diagnostics](CAPTURE-TROUBLESHOOTING.md). Physical driver acceptance remains outstanding.

Version 0.2.0 adds a per-working-session privileged broker, MAC leases, PANIC/shred and verified tmpfs storage. The [0.2.0 architecture and limitations](PRIVACY.md) supersede older statements below about elevation per operation and disk-backed Hashcat requirements: RAM SESSION now supports a tmpfs Hashcat workspace.

## Process boundaries

The unprivileged Tauri process owns sessions, NetworkManager discovery, packet metadata and Hashcat jobs. React calls typed commands through Tauri IPC. No shell plugin, arbitrary command execution, arbitrary package names or arbitrary output paths are exposed to the frontend.

`/usr/libexec/deadair-capture-helper` is authorized through the included polkit policy. Fixed operations: dependency installation, adapter report, passive capture and app-owned recovery. It uses system executable paths and argument arrays. It never runs the GUI as root or stops NetworkManager globally.

`network.rs` uses NetworkManager D-Bus and waits for `LastScan` to change before returning access-point properties. `iw` supplies driver/capability data. A D-Bus integration test uses an isolated service fixture; physical discovery remains a hardware test.

## Passive observation

A capture targets an access point from current live discovery. The helper validates adapter, physical radio, frequency and BSSID again. It requires a disconnected managed secondary interface, checks that no other interface on the radio is active, journals original state, temporarily releases that interface from NetworkManager, creates a uniquely named monitor interface, and runs `airodump-ng` on one frequency/BSSID.

The journal records owner UID, helper PID/start time, physical radio, interface indices and original NetworkManager/link state. Normal stop, input-pipe closure and handled termination signals stop the collector and restore the selected adapter. Recovery checks identity before touching an interface. Uncertain cleanup produces a visible error.

The collector writes into a root-only directory on verified `/run` tmpfs. Its temporary PCAP can contain other frames from the selected BSSID before filtering; this is not metadata-only collection at the driver boundary. The helper forwards only selected management and unprotected EAPOL-Key frames. Bounds: 16 MiB runtime PCAP, five minutes, 20,000 forwarded frames, 4,096 app-retained raw frames, 200 displayed packet records. Dropped-retention counters are visible. The collector can briefly exceed the byte threshold between polling checks.

The EAPOL tracker checks station, role, replay counters, nonces and time proximity, including reordering/retransmission. It reports observed correlation, not MIC validation, key possession or password recovery. WPA3 authentication coverage is not claimed. No deauthentication, rogue AP, forced reconnection, injection or automated capture-to-password workflow is provided.

## Sessions and secrets

Disk-backed sessions use UUID directories (0700), files (0600), active locks and manifests. Descriptor-relative access rejects symlinks and hardlinks. Exports stay inside the session. Temporary sessions are removed on successful close; persistent sessions remain. Unexpected filesystem contents cause cleanup to fail visibly.

Memory-only sessions do not create a session directory. Passive observation still uses `/run` tmpfs because airodump requires a file output. Tmpfs can be swapped: this is not a forensic non-persistence guarantee. Crashes and force-kills can leave journals/capture files until recovery or reboot. Exports and Hashcat jobs require a disk-backed session.

Hashcat accepts benchmarks or SHA-256 vectors generated from passwords the user already knows. Candidate text is limited to 4 MiB. Rust-owned password/wordlist strings use zeroization; JavaScript/OS/library copies cannot be guaranteed erased. Potfile, restore and log output are disabled. Temporary inputs are removed when a job finishes. HOME/cache/temp paths stay under the session. Only allowlisted statistics reach the GUI, not candidates or recovered plaintext. Runtime limits and explicit stop reap the process. Persistent-session caches remain; temporary-session cleanup removes them.

Kismet tokens are used for a loopback status request, with no proxy or redirects, and are not persisted. Preferences and observations are not written to browser localStorage. External programs and services can maintain their own logs.

## Recovery limits

Cleanup cannot run after SIGKILL or power loss. Use **ADAPTERS → RESTORE STALE CAPTURE INTERFACES** after restarting. If an interface changed identity or a crash occurred between its creation and recording its index, recovery refuses automatic deletion; inspect `/run/deadair/*.json` and `iw dev` on Linux. Do not indiscriminately delete interfaces or system directories.

Driver-specific monitor behavior and restoration require physical hardware validation.
