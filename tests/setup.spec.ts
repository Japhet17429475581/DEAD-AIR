import { test, expect } from '@playwright/test';

// IPC contract tests; these explicitly substitute a backend and do not test Linux installation.
test('first run previews packages, reports installation failure and allows retry', async ({ page }) => {
  await page.addInitScript(() => {
    let attempts = 0;
    const calls: {cmd: string; args: Record<string, unknown>}[] = [];
    const plan = {distro: 'debian', manager: 'apt-get', packages: ['network-manager', 'iw', 'iproute2', 'aircrack-ng', 'hashcat'], missing: ['airodump-ng'], helperAvailable: true, profile: 'all'};
    Object.assign(window, { testCalls: calls, __TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__: {
      transformCallback: () => 1, unregisterCallback: () => {},
      invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
        calls.push({cmd, args});
        if (cmd === 'dependency_plan') return plan;
        if (cmd === 'install_dependencies') {
          if (++attempts === 1) throw 'Package manager is locked. Retry after the other operation finishes.';
          return {...plan, missing: []};
        }
        if (cmd === 'system_check') return {tools: [], adapters: [], errors: ['No radio hardware in test fixture']};
        if (cmd.startsWith('plugin:event|')) return 1;
        throw `Unexpected IPC command: ${cmd}`;
      }
    }});
  });
  await page.goto('/');
  await expect(page.getByRole('heading', {name: 'FIRST RUN // DEPENDENCY SETUP'})).toBeVisible();
  await expect(page.locator('.setup-packages')).toContainText('aircrack-ng');
  expect(await page.evaluate(() => (window as any).testCalls.some((c: any) => c.cmd === 'install_dependencies'))).toBe(false);
  await page.getByRole('button', {name: 'INSTALL DEPENDENCIES', exact: true}).click();
  await expect(page.getByRole('alert')).toContainText('Package manager is locked');
  await page.getByRole('button', {name: 'INSTALL DEPENDENCIES', exact: true}).click();
  await expect(page.getByText('Dependencies installed and checked. You can start a session.')).toBeVisible();
  expect(await page.evaluate(() => (window as any).testCalls.filter((c: any) => c.cmd === 'install_dependencies').every((c: any) => c.args.authorized === true && c.args.profile === 'all'))).toBe(true);
  await page.getByRole('button', {name: 'CONTINUE', exact: true}).click();
  await expect(page.getByRole('heading', {name: 'FIRST RUN // DEPENDENCY SETUP'})).toHaveCount(0);
});

test('failed automatic dependency detection stays visible', async ({ page }) => {
  await page.addInitScript(() => Object.assign(window, {__TAURI_EVENT_PLUGIN_INTERNALS__:{unregisterListener:()=>{}},__TAURI_INTERNALS__: {
    transformCallback: () => 1, unregisterCallback: () => {},
    invoke: async (cmd: string) => {
      if (cmd === 'dependency_plan') throw 'Automatic installation is not supported on test-distro';
      if (cmd === 'system_check') return {tools: [], adapters: [], errors: []};
      return 1;
    }
  }}));
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('not supported on test-distro');
  await expect(page.getByRole('button', {name: 'INSTALL DEPENDENCIES', exact: true})).toBeDisabled();
});
