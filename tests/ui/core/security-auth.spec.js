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
  await page.evaluate(() => {
    document.getElementById('btn-save-dashboard-auth')?.click();
  });

  await page.reload();
  await expect(page.locator('#auth-shell')).toBeVisible();
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
  await page.evaluate(() => {
    document.getElementById('btn-save-dashboard-auth')?.click();
  });

  await page.reload();
  await expect(page.locator('#auth-shell')).toBeHidden();
});
