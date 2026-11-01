# Contributing to DEAD//AIR

DEAD//AIR combines native Linux diagnostics with clearly separated educational simulations. Preserve the distinction between observed results, generated teaching fixtures, and unavailable information.

## Work locally

See [the build guide](docs/BUILDING.md). For frontend work, install Node.js 22.12+, run `npm ci`, then `npm run dev`. Use an ordinary user account for the desktop application.

Run the checks relevant to your change:

```sh
npm test
npm run build
npm run test:e2e
```

For native changes on Linux:

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked --no-default-features
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
```

## Design constraints

- Never fill LIVE results with demonstration data or report success without evidence.
- Keep simulated labs isolated from radio, privileged operations, and LIVE counters. Real deauth transmission is outside the current product scope.
- Keep privileged requests narrow. Do not add arbitrary command execution, password storage, or a root GUI.
- Preserve explicit consent, cancellation, bounded parsing, storage ownership checks, and adapter restoration.
- Respect reduced motion and sound settings. Atmospheric messages remain separate from technical status.
- Explain what was tested and what still needs hardware. A mocked IPC test is not a hardware test.

## Issues and pull requests

Include the version, distribution, desktop/session type, relevant tool versions, expected behavior, and actual behavior. For adapter issues, include driver/chipset details if available. Remove SSIDs, MAC addresses, tokens, credentials, and personal paths before sharing logs. Do not upload captures from other people's networks.

Describe the resulting behavior, checks run, and material limitations in a pull request. Include a screenshot for visible UI changes. Use synthetic minimal packet fixtures for parser tests.

For suspected vulnerabilities, use GitHub's private vulnerability reporting if available on the repository. Do not put credentials, exploit details affecting users, or private captures into a public issue. If private reporting is unavailable, ask for a private contact without disclosing those details.
