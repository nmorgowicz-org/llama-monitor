import { test, expect } from '@playwright/test';

test('dashboard access can be enabled, used, and disabled from settings', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('html.modules-ready');

  // Reset auth state via API to avoid interference from parallel tests
  const resetResult = await page.evaluate(async () => {
    try {
      const statusRes = await fetch('/api/auth/status');
      const status = await statusRes.json();
      if (!status.enabled) {
        return { wasEnabled: false };
      }

      const shell = document.getElementById('auth-shell');
      if (shell && !shell.hasAttribute('aria-hidden')) {
        await fetch('/api/auth/login', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ username: 'admin', password: 'secret1234' }),
        });
      }

      const headers = window.authHeaders
        ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
        : { 'Content-Type': 'application/json' };
      const res = await fetch('/api/auth/config', {
        method: 'PUT',
        headers,
        body: JSON.stringify({ basic_enabled: false, form_enabled: false, username: '' }),
      });
      return { wasEnabled: true, status: res.status };
    } catch (e) {
      return { wasEnabled: false, error: e.message };
    }
  });

  if (resetResult.wasEnabled) {
    expect(resetResult.status, `Reset auth failed`).toBe(200);
    await page.waitForTimeout(500);
  }

  await page.reload();
  await page.waitForSelector('html.modules-ready');

  // Enable form auth via API
  const enableResult = await page.evaluate(async () => {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/auth/config', {
      method: 'PUT',
      headers,
      body: JSON.stringify({
        basic_enabled: false,
        form_enabled: true,
        username: 'admin',
        current_password: '',
        new_password: 'secret1234',
      }),
    });
    return { status: res.status, body: await res.json() };
  });
  expect(
    enableResult.status,
    `Enable auth failed: ${JSON.stringify(enableResult.body)}`,
  ).toBe(200);

  // Verify auth status reflects the new config
  const preReloadStatus = await page.evaluate(async () => {
    const res = await fetch('/api/auth/status', { cache: 'no-store' });
    return await res.json();
  });
  expect(
    preReloadStatus.methods?.form,
    `form auth not active: ${JSON.stringify(preReloadStatus)}`,
  ).toBe(true);

  // Reload and verify auth shell appears
  await page.reload();
  await expect(page.locator('#auth-shell')).toBeVisible({ timeout: 10000 });
  await expect(page.locator('#auth-shell-recovery')).toContainText('clear-auth-config');
  await page.locator('#auth-username').fill('admin');
  await page.locator('#auth-password').fill('secret1234');
  await page.locator('#auth-submit').click();

  await page.waitForSelector('html.modules-ready');
  await expect(page.locator('.top-nav-bar')).toBeVisible();

  // Wait for page to stabilize after login navigation
  await page.waitForTimeout(1000);

  // Disable form auth via API
  const disableResult = await page.evaluate(async () => {
    const headers = window.authHeaders
      ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
      : { 'Content-Type': 'application/json' };
    const res = await fetch('/api/auth/config', {
      method: 'PUT',
      headers,
      body: JSON.stringify({ basic_enabled: false, form_enabled: false, username: '' }),
    });
    return { status: res.status, body: await res.json() };
  });
  expect(
    disableResult.status,
    `Disable auth failed: ${JSON.stringify(disableResult.body)}`,
  ).toBe(200);

  // Reload and verify auth shell is gone
  await page.reload();
  await expect(page.locator('#auth-shell')).toBeHidden();
});
