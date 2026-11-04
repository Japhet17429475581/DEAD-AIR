import { expect, test } from '@playwright/test';

test('all four rendered poses preserve grid size and visible cell alignment', async ({ page }) => {
  await page.goto('/?skull-preview=1');
  const poses = page.locator('.skull-contact-sheet .skull');
  await expect(poses).toHaveCount(4);
  const measurements = await poses.evaluateAll(elements => elements.map(element => {
    const box = element.getBoundingClientRect();
    const grid = element.querySelector('.skull-grid')!;
    return { width: box.width, height: box.height, overflow: grid.scrollWidth > grid.clientWidth + 1, rows: [...element.querySelectorAll('.skull-row')].map(row => ({ length: row.textContent!.length, height: row.getBoundingClientRect().height })),
      bone: getComputedStyle(element.querySelector('.skull-cell-bone')!).color, eye: getComputedStyle(element.querySelector('.skull-cell-eye')!).color };
  }));
  for (const item of measurements) {
    expect(item.width).toBe(measurements[0].width); expect(item.height).toBe(measurements[0].height);
    expect(item.overflow).toBe(false); expect(item.rows).toHaveLength(48);
    item.rows.forEach(row => { expect(row.length).toBe(72); expect(row.height).toBe(item.rows[0].height); });
    expect(item.bone).toBe('rgb(57, 255, 120)'); expect(item.eye).toBe('rgb(255, 53, 69)');
  }
});

test('laugh actually opens and closes without moving its bounding box', async ({ page }) => {
  const time = new Date('2026-10-29T12:00:00Z');
  await page.clock.install({ time });
  // Pause ahead of installation time: the protocol round-trip can advance the
  // clock while a release build competes for CPU on the test host.
  await page.clock.pauseAt(new Date(time.getTime() + 60_000));
  await page.goto('/?skull-preview=1');
  const skull = page.locator('.skull-live-stage .skull');
  await page.getByRole('button', { name: 'REPLAY STATE' }).click();
  const before = await skull.boundingBox();
  await expect(skull).toHaveAttribute('data-pose', '0');
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '1');
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '2');
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '3');
  expect(await skull.boundingBox()).toEqual(before);
  await page.clock.runFor(240); await expect(skull).toHaveAttribute('data-pose', '2');
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '1');
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '0');
  await page.clock.runFor(1700); await expect(skull).toHaveAttribute('data-pose', '0');
});

test('application errors, scanning, success, and session shutdown drive the character', async ({ page }) => {
  await page.goto('/');
  const skull = page.locator('.hero-art .skull');
  await expect(skull).toHaveAttribute('data-state', 'idle');
  await page.getByRole('button', { name: '[ RECHECK DEPENDENCIES ]', exact: true }).click();
  await expect(skull).toHaveAttribute('data-state', 'error');
  await page.getByRole('button', { name: 'Dismiss error' }).click();
  await page.getByRole('button', { name: 'ENTER DEMO', exact: true }).click();
  await page.getByRole('button', { name: 'ENABLE DEMO MODE', exact: true }).click();
  await page.getByRole('button', { name: 'START SESSION', exact: false }).click();
  await expect(skull).toHaveAttribute('data-state', 'idle');
  // Keyboard rescan keeps the central overview visible during the operation.
  await page.locator('h1').click(); await page.keyboard.press('r');
  await expect(skull).toHaveAttribute('data-state', 'scanning');
  await expect(skull).toHaveAttribute('data-state', 'grin');
  await page.getByRole('button', { name: 'END SESSION', exact: false }).first().click();
  await page.getByRole('button', { name: 'STOP AND END SESSION', exact: true }).click();
  await expect(skull).toHaveAttribute('data-state', 'shutdown');
  await expect.poll(async () => (await skull.textContent())!.trim()).toBe('');
});

test('system reduced motion freezes the laugh and state controls remain usable', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?skull-preview=1');
  const skull = page.locator('.skull-live-stage .skull');
  await expect(skull).toHaveClass(/skull-static/);
  await expect(skull).toHaveAttribute('data-pose', '0');
  await page.getByLabel('Skull state').selectOption('shutdown');
  await expect.poll(async () => (await skull.textContent())!.trim()).toBe('');
});

test('hidden tabs pause animation rather than burning through the laugh', async ({ page }) => {
  const time = new Date('2026-10-29T12:00:00Z');
  await page.clock.install({ time }); await page.clock.pauseAt(new Date(time.getTime() + 60_000));
  await page.goto('/?skull-preview=1');
  const skull = page.locator('.skull-live-stage .skull');
  await page.getByRole('button', { name: 'REPLAY STATE' }).click();
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '1');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.clock.runFor(5000); await expect(skull).toHaveAttribute('data-pose', '1');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: false }); document.dispatchEvent(new Event('visibilitychange')); });
  await page.clock.runFor(160); await expect(skull).toHaveAttribute('data-pose', '2');
});

test('the central character fits at 1280 × 720 without horizontal overflow', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 720 }); await page.goto('/');
  const box = await page.locator('.hero-art .skull').boundingBox();
  const panel = await page.locator('.hero-art').boundingBox();
  expect(box).not.toBeNull(); expect(panel).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(panel!.x); expect(box!.x + box!.width).toBeLessThanOrEqual(panel!.x + panel!.width);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});
