# Verification record — 0.4.0

Recorded on 2026-11-05. These are development checks, not an independent security audit.

| Check | Result and scope |
| --- | --- |
| Frontend unit tests | 23 passed. |
| TypeScript / Vite production build | Passed. |
| Rust tests, `--locked --no-default-features` | 53 passed; 4 opt-in hardware/compute tests not rerun for this change. Includes bounded netlink parsing, completion-evidence requirements, owned cleanup refusal, recovery process identities, broker whitelist and distribution package plans. |
| Rust desktop, `--locked --all-targets` | Passed in Debian 12 with WebKitGTK 4.1 build prerequisites. |
| Browser scenarios | Academy, physical-capture IPC, storage/PANIC controls, skull animation, themes and the new lab's IPC contract exercised. Fixtures are explicitly separate from kernel validation. |
| Linux release packages | GUI/helper built; DEB and RPM generated. DEB 0.4.0 installed over 0.3.3 in the disposable Debian container. Installed helper SHA-256 matches the release build. |
| Native Linux GUI | Installed `/usr/bin/dead-air` launched as ordinary `deadair-test` under WebKitGTK/Xvfb. Calm theme and actual Linux dependency status visually inspected; normal `WM_DELETE_WINDOW` shutdown completed. Container-only WebKit sandbox override belongs to the test harness, not the desktop entry. |
| Actual virtual WLAN | Passed with the release helper under QEMU TCG and Debian kernel `6.1.0-54-amd64`. See details below. |

## Actual kernel integration

The test uses `scripts/verify-virtual-lab-vm.sh`, `verify-virtual-lab-guest.sh` and `verify-virtual-lab.py`. The guest has a read-only root, tmpfs runtime and no external NIC or host Wi-Fi device. This exercises actual kernel code and distribution hostapd/supplicant/airodump processes.

Verified:

- A pre-existing foreign hwsim AP remains outside the lab's scan results.
- The client finds the lab's actual BSSID and SSID.
- The real client reports `COMPLETED`; the AP independently reports `AUTHORIZED`.
- The observer receives M1, M2, M3 and M4 with a correlated exchange.
- Ping succeeds from the client namespace through the virtual WLAN to the AP.
- STOP removes owned processes, radios and runtime files, while preserving the original radio inventory.
- A second independent run succeeds; forced worker death is followed by same-UID stale-runtime recovery.
- The installed broker accepts a real UID 1000 client; closing that client's stdin performs orderly lab cleanup and broker shutdown removes its socket.

The final release run observed 64 target frames before cleanup and emitted `DEADAIR_VM_RESULT=PASS`. The frame count is an observation from this run, not a fixed success threshold. Debug and release runs both passed the protocol scenario. A missing regulatory database warning in the minimal guest did not prevent the tested channel-6 experiment.

During development the real VM exposed and helped fix: wiphy lookup across namespaces, the correct `SET_WIPHY_NETNS` netlink command, recognition of the `wpa_supplicant` executable name, and a race where the client could connect before the observer was ready. The final workflow waits for actual AP frames before enabling connection.

## Theme verification

The default is calm crimson with neutral text, zero configured bloom and no scanlines. The original phosphor colors remain selectable. Browser checks verify the default, color switch, body-text contrast of at least 4.5:1 for the sampled foreground/background pair, unchanged skull dimensions, and restoration of the calm effects defaults. This is not a complete accessibility audit or a medical claim about eye comfort.

## Still requiring acceptance

- Ubuntu, Fedora and Arch kernel/module combinations, package installation and desktop confinement policies; Fedora RPM installation and Arch package installation.
- A normal desktop's graphical polkit prompts and canceled authorization flow for virtual-lab startup.
- End-to-end lab creation through the GUI on the same full Linux desktop; the kernel runtime and unprivileged broker transport were tested separately from GUI IPC fixtures.
- Ten-minute expiry and size-limit paths under sustained load, repeated concurrent third-party hwsim workloads, and host suspend/resume.
- Physical adapter behavior, driver-specific monitor/MAC restoration and reconnection. Virtual radios do not validate physical RF reception or those driver paths.

One real scenario ships in this version. WPA3, Enterprise and multi-AP scenarios in the [roadmap](VIRTUAL-LABS.md#next-scenarios) are not implemented. The deauth lesson remains offline, without an attack transmitter.

Local logs, VM output, package files and screenshots are under ignored `artifacts/`. The README includes browser preview images; these do not depict a live radio assessment.
