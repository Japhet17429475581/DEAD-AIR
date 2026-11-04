# Receiver health and connected-card MAC leases — 0.3.3

The reported symptom was zero frames while the application kept saying `WAITING FOR CLIENT AUTHENTICATION`, including after a normal client reconnect. That does not establish that the selected AP is being received. The earlier status was set immediately after launching the collector, without packet evidence.

## What can be established

The software file-format path was tested using the real distribution airodump-ng in offline mode with synthetic beacons and four EAPOL-Key messages. Its PCAP output was read by DEAD//AIR's incremental reader and parser. This does not test a physical monitor interface, driver, firmware, or the reporter's card; the actual cause of zero radio frames remains unconfirmed until receive evidence is available.

Capture now uses `--beacons` so all observed target beacons are recorded. AP reception can be shown without claiming that a client reauthenticated. The BSSID/frequency scope and memory/packet bounds remain in force.

The helper sends bounded progress reports once per second, with actual PCAP file size, records read, and kernel-reported frequency. Every two seconds it checks interface mode/frequency; a reported frequency different from the selected target or a changed mode stops observation with an error. A successful tuning command is not substituted for an actual frequency report.

## Interpreting LIVE EVIDENCE

| Evidence | Meaning |
| --- | --- |
| No readable capture file after startup | The collector/file path has not produced evidence; this is not a wait for client authentication. |
| PCAP data after the header, zero records read | No complete records have reached the incremental reader. Inspect the collector/reader path. |
| Records read, zero accepted target frames | The file has records, but they did not match the supported parser/BSSID scope. |
| Target beacons/frames, zero EAPOL | Reception is confirmed for those frames. An already connected client may never repeat the four-way exchange. |
| EAPOL frames | Real authentication-related frames were parsed; they can still be partial. |
| Missing or stale helper reports | Current receiver health is unknown. The display does not invent healthy reception. |

The receiver warning appears after 15 seconds without accepted target frames. Missing-file and helper-report problems are shown separately. LIVE counters change only when packets are parsed; progress reports are not packet events. The running helper version is read from its own ready message; an old helper without this protocol is shown as unreported.

Observation is bounded to five minutes, followed by adapter restoration. The GUI shows time remaining and the actual termination reason. After stopping, no target frames, no EAPOL, partial EAPOL, and a correlated sequence are different outcomes. Completion of a timed observation does not claim a handshake was captured or cryptographically verified.

For a failure report, copy the exact error and the LIVE EVIDENCE values after 20–30 seconds: actual frequency, PCAP bytes, raw records, target frames, beacon/EAPOL counters, and helper version. Include the adapter driver and distribution. Remove identifiers before sharing publicly. A monitor-mode capability flag alone does not prove working reception.

## MAC profiles on a connected built-in card

The old selector disabled connected cards. The updated selector accepts built-in or USB cards in managed mode. If connected, **I allow Wi-Fi disconnection for this MAC lease** must be checked before applying a profile. Consent is checked again against actual NetworkManager state in the helper.

The helper saves the original MAC, link/NetworkManager state, active profile UUID and autoconnect setting. It disconnects the selected profile, releases the interface from NetworkManager, changes the MAC while down, restores the original link-up setting, and verifies the address. NetworkManager remains released for the lease so it cannot immediately replace the address. **The selected card remains disconnected until RESTORE ORIGINAL MAC.** This is not a workflow for reconnecting with a spoofed identity.

Restoration verifies interface identity and an idle radio, restores/readbacks the original MAC, restores NetworkManager control, and reactivates the saved connection with state/UUID verification. Capture can temporarily use the leased idle radio; stop capture before restoring the MAC. Old recovery journals remain readable. If the driver refuses MAC changes, the actual command error is shown; built-in hardware support is not guaranteed.

A rejected preflight with a reaped helper and no rollback failure no longer creates an artificial permanent recovery requirement. Rollback errors or incomplete helper termination remain visible and require recovery. MAC helper stderr is retained rather than discarded.

## References

- [Aircrack-ng: airodump-ng options, beacons and troubleshooting](https://www.aircrack-ng.org/doku.php?id=airodump-ng).
- [Aircrack-ng upstream collector source](https://github.com/aircrack-ng/aircrack-ng/blob/master/src/airodump-ng/airodump-ng.c).
- [NetworkManager device D-Bus interface](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html).

Install the complete updated package, including its helper, with DEAD//AIR closed. A frontend-only update leaves the old privileged helper in use. Source builds need to rebuild/reinstall the helper using the documented build instructions.

## Recorded verification

- 23 frontend unit tests and the production frontend build passed.
- 46 native Linux tests passed: 38 library tests, four helper tests, one isolated D-Bus integration test, and three incremental-PCAP tests. Compute-runtime tests remain excluded from this run.
- Two opt-in checks passed against the real distribution airodump-ng: synthetic offline beacon/EAPOL output compatibility, and missing-interface stderr/runtime cleanup. Neither accesses a physical radio.
- 22 browser scenarios passed with one worker. After the final receiver-panel layout change, the five receive-health/MAC scenarios were rerun and passed. Browser data is explicitly mocked IPC, not observed radio data.
- `cargo check --locked --all-targets` passed on Debian 12.
- Release GUI/helper and DEB/RPM packages were built. The DEB was installed over 0.3.2 in the Debian container. RPM installation on Fedora and native GUI interaction on physical hardware were not tested in this run.

Browser verification used a separately managed Vite server because automatic test-server teardown stalled in the Windows sandbox. The stalled harness was stopped; this was not presented as an application failure or a passing completed run. The later completed runs are recorded in `artifacts/capture-health-browser-final.log` and `capture-health-browser-focused.log`.

Other local logs use the `artifacts/capture-health-*` prefix and are excluded from Git. The UI screenshot in that directory is a test fixture, not evidence from a real access point. Physical receive behavior, connected-card MAC changes, rollback and reconnect still require testing on actual Linux Wi-Fi hardware.
