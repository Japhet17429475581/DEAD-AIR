import { chromium } from '@playwright/test';
import { mkdirSync } from 'node:fs';

mkdirSync('artifacts', { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 1600, height: 1080 }, deviceScaleFactor: 1 });
const errors = [];
page.on('pageerror', error => errors.push(error.message));
await page.goto('http://127.0.0.1:1420/?skull-preview=1');
await page.getByRole('heading', { name: 'SKULL / ANATOMY & MOTION' }).waitFor();
await page.getByLabel('Skull state').selectOption('idle');
await page.screenshot({ path: 'artifacts/skull-contact-sheet.png', fullPage: true });
await page.locator('.skull-contact-sheet').screenshot({ path: 'artifacts/skull-poses.png' });
await page.goto('http://127.0.0.1:1420/');
await page.getByRole('heading', { name: 'OVERVIEW' }).waitFor();
await page.screenshot({ path: 'artifacts/dashboard.png', fullPage: true });
console.log(JSON.stringify({ errors, preview: 'artifacts/skull-contact-sheet.png', poses: 'artifacts/skull-poses.png' }));
await browser.close();
