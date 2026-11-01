# DEAD//AIR character-cell skull

The mascot is original editable text artwork, not an image, emoji, or randomized character field. Its three 72-column × 48-row layers are in `src/components/skull/art/`:

- `upper.txt`: cranial vault, orbital rims and cavities, nasal opening, zygomatic arches, maxilla, upper teeth.
- `jaw.txt`: lower teeth, mandibular body, and rami.
- `eyes.txt`: small crimson embers contained inside the orbital cavities.

The anatomical shapes and directional lighting are authored in `scripts/design-skull.mjs`. `node scripts/design-skull.mjs` regenerates the text layers deterministically. Alternatively, edit the text layers directly, preserving every row's width. Regeneration replaces manual edits, so choose one source of editing at a time.

## Choreography

The upper skull is fixed. Four discrete mandible poses move it down by 0, 1, 3, and 6 character rows. Small articulated rami retain the attachment to the skull. The cycle is closed → slightly open → wide → maximum → wide → slight → closed. Success runs two cycles (2.56 seconds), then rests closed. Red coloration applies to the eye layer and mandible, preserving the green upper anatomy.

`SkullAnimator` supports startup, idle, scanning, discovery flicker, completion grin, success, error/alert, critical failure, laboratory activity, disappointment, cleanup, shutdown, and decorative ambient states. The application drives operational reactions from lifecycle callbacks. Success/error major reactions have a 30-second cooldown; repeated errors still retain a static alert. Critical failure and shutdown take precedence over ambient behavior. Demo actions remain explicitly labelled. Animation never generates system events.

The occasional glitch displaces only two cranial rows for 80 ms. The grid box remains invariant. Animation clocks pause in hidden tabs and respect OS reduced motion. Shutdown dissolves the skull over 2.4 seconds, with eyes disappearing last; reduced motion clears immediately. Idle behavior is scheduled every 45–135 seconds and deferred during recent input, focused fields, dialogs or hidden tabs. Optional sound defaults off. Decorative archive phrases appear separately from system logs.

## Development preview

Run `npm run dev`, then open:

http://127.0.0.1:1420/?skull-preview=1

This separate development screen has state selection, replay, motion control, and a four-pose contact sheet. The query does not enable the preview in production builds. No radio session or hardware event is needed to preview animation.

## Validation performed on Windows

- TypeScript and Vite production build.
- Vitest checks grid dimensions, upper-skull invariance, mandible travel, orbital placement, state transitions, reduced motion, shutdown, and glitch boundaries.
- Playwright with installed Google Chrome checks actual rendered cell alignment, green/red colors, the complete laugh cycle and constant bounding box, application-state integration using explicit demo operations, reduced motion, and 1280 × 720 layout.
- Rendered screenshots inspected against the supplied green CRT / DEAD//AIR references. The contact sheet is `artifacts/skull-poses.png`; full preview is `artifacts/skull-contact-sheet.png`.

The real Tauri window was also launched and visually inspected in Debian 12/WebKitGTK under Xvfb, including the fixed character grid, real dependency setup and a native temporary session. Screenshots: `artifacts/linux-dashboard.png`, `artifacts/linux-session.png`. Hardware-backed scan and capture reactions still require the Linux radio acceptance tests in `docs/LINUX.md`. Backend verification is reported separately in `docs/VERIFICATION.md`.

Commands: `npm test`, `npm run test:e2e`, `npm run build`, `node scripts/preview-skull.mjs` (last command expects the dev server running and Chrome installed).
