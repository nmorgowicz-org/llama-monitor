import { test, expect } from '@playwright/test';

test('dashboard access can be enabled, used, and disabled from settings', async ({ page }) => {
  await page.goto('/');
  await page.waitForSelector('html.modules-ready');

  await page.getByRole('button', { name: /settings/i }).first().click();
  await expect(page.locator('#settings-modal')).toHaveClass(/open/);
  await page.locator('.settings-tab[data-tab="security"]').click();
  await expect(page.locator('#dashboard-auth-status')).not.toHaveText(/Checking dashboard access/i);

  await page.evaluate(() => {
    document.querySelector('#dashboard-auth-mode-pills [data-auth-mode="form"]')?.click();
  });
  await page.locator('#dashboard-auth-username').fill('admin');
  await page.locator('#dashboard-auth-new-password').fill('secret1234');
  await page.locator('#dashboard-auth-confirm-password').fill('secret1234');

  // Capture the PUT response body to verify form auth was actually enabled.
  let saveResponseBody = null;
  const [saveResponse] = await Promise.all([
    page.waitForResponse(resp => resp.url().includes('/api/auth/config') && resp.request().method() === 'PUT'),
    page.evaluate(() => { document.getElementById('btn-save-dashboard-auth')?.click(); }),
  ]);
  saveResponseBody = await saveResponse.json().catch(() => null);
  // If the save did not return 200 OK (e.g. backend rejected it), fail fast with context.
  expect(saveResponse.status(), `PUT /api/auth/config failed: ${JSON.stringify(saveResponseBody)}`).toBe(200);

  // Verify auth status reflects the new config before we reload.
  const preReloadStatus = await page.evaluate(() =>
    fetch('/api/auth/status', { cache: 'no-store' }).then(r => r.json()),
  );
  expect(preReloadStatus.methods?.form, `form auth not active before reload: ${JSON.stringify(preReloadStatus)}`).toBe(true);

  await page.reload();
  await expect(page.locator('#auth-shell')).toBeVisible({ timeout: 10000 });
  await expect(page.locator('#auth-shell-recovery')).toContainText('clear-auth-config');
  await page.locator('#auth-username').fill('admin');
  await page.locator('#auth-password').fill('secret1234');
  await page.locator('#auth-submit').click();

  await page.waitForSelector('html.modules-ready');
  await expect(page.locator('.top-nav-bar')).toBeVisible();

  await page.getByRole('button', { name: /settings/i }).first().click();
  await expect(page.locator('#settings-modal')).toHaveClass(/open/);
  await page.locator('.settings-tab[data-tab="security"]').click();
  await expect(page.locator('#dashboard-auth-status')).not.toHaveText(/Checking dashboard access/i);

  await page.evaluate(() => {
    document.querySelector('#dashboard-auth-mode-pills [data-auth-mode="none"]')?.click();
  });
  await Promise.all([
    page.waitForResponse(resp => resp.url().includes('/api/auth/config') && resp.status() === 200 && resp.request().method() === 'PUT'),
    page.evaluate(() => { document.getElementById('btn-save-dashboard-auth')?.click(); }),
  ]);

  await page.reload();
  await expect(page.locator('#auth-shell')).toBeHidden();
});
