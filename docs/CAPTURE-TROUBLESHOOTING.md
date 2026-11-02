# Capture adapter fix — 0.3.2

This update addresses confirmed defects in adapter handling. The original reporter could not provide the exact error, so these defects are not proof of the cause on that specific machine.

## What changed

The previous helper rejected an interface already in monitor mode. For an idle managed card it also attempted to create an additional monitor VIF; a driver can support monitor mode without supporting that interface combination.

Capture now uses the selected interface itself:

- **Managed, idle:** save NetworkManager/link state, release the selected interface, switch that same interface to monitor, and restore managed state afterward.
- **Managed, connected:** require explicit temporary-disconnection consent, save the active profile UUID, disconnect, observe, and reactivate the saved profile on stop. Reconnection is verified by NetworkManager state/UUID.
- **Already monitor:** do not require a NetworkManager device or create another interface. Record the original channel definition and link state, observe, and restore the recorded settings. Leave the interface in monitor mode. AP/mesh interfaces remain excluded.

Existing monitor channels preserve control frequency, width, center frequencies, and NOHT where reported. A monitor interface with no channel reported is still accepted; it keeps its original mode/link state afterward, but there is no original channel to restore, so the last capture frequency can remain configured. A malformed or unsupported reported channel is refused before modification. Recovery checks interface index/PHY and mode; changed identities or failed restoration retain the journal for inspection. Another active interface on the same radio is not silently stopped.

An unfinished DEAD//AIR capture journal blocks a new capture on that radio until recovery. This prevents treating an interface left in monitor mode by a crashed app as an unrelated pre-existing monitor interface. Older journals remain readable.

## Useful error details

The helper previously discarded stderr from adapter commands and the collector. Bounded diagnostics now reach the GUI, including command context/exit status and collector stderr. The **OBSERVATION FAILED** panel is selectable for copying. Cleanup diagnostics are retained alongside the original error.

The capture page has **REFRESH CAPTURE ADAPTERS**. If a mode change renamed an interface, refreshing discards the missing selection and selects a present candidate. Both preflight rejection and later collector failure stay visible near the start controls.

If a failure remains, provide the exact panel text, application version, selected interface, driver, mode, and target band/frequency. Remove identifiers before sharing publicly. A supported monitor mode does not establish support for every frequency, simultaneous interface combination, or regulatory condition.

After an interrupted older capture, use **ADAPTERS → RESTORE STALE CAPTURE INTERFACES**, then refresh. Do not remove root recovery journals by hand to bypass a failed restoration.

## References and acceptance

- [Linux Wireless: iw and monitor interfaces](https://wireless.docs.kernel.org/en/latest/en/users/documentation/iw.html).
- [Aircrack-ng: airodump-ng, frequency selection and troubleshooting](https://www.aircrack-ng.org/doku.php?id=airodump-ng).

Regression tests cover mode admission, consent, channel parsing/argument validation, old journals, command diagnostics, selection refresh and visible asynchronous errors. An opt-in test runs the real airodump-ng against a nonexistent interface to check stderr delivery and runtime cleanup. This is not a radio test.

Physical capture, driver-specific mode transitions, borrowed-interface restoration, and saved-profile reconnect still require verification on Linux Wi-Fi hardware. The fix does not claim that the reporter's card has been tested.

### Recorded checks for 0.3.2

- `npm test`: 23 passing frontend unit tests; production frontend build passed.
- Native Linux suite: 38 passing tests; two compute-runtime tests and the opt-in collector test are excluded from the default run.
- Real collector failure-path test: separately passed with installed airodump-ng, a nonexistent interface, and verified tmpfs storage. No radio capture was performed.
- `cargo check --locked --all-targets`: passed on Debian 12.
- Release GUI/helper and DEB/RPM packages were built. The DEB was installed over 0.3.1 in the Debian container; RPM installation on Fedora was not tested.
- Browser suite: 17 scenarios passed with one worker. The initial parallel run encountered timeouts in two older Academy scenarios and its stalled runner was stopped; the complete sequential rerun passed without test or application changes.

Local logs live under `artifacts/monitor-fix-*` and are excluded from Git. Updating from an older version requires both the GUI and installed helper: close DEAD//AIR, install the new package, then relaunch. Updating frontend files alone leaves the old system helper in use. Source builds must rerun `scripts/install-helper.sh` after rebuilding the helper.
