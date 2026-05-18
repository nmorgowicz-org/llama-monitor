import { test, expect } from '@playwright/test';

test('form auth shell can unlock the dashboard when enabled', async ({ page }) => {
  await page.goto('/');

  const authShell = page.locator('#auth-shell');
  if (await authShell.isVisible().catch(() => false)) {
    await page.locator('#auth-username').fill('admin');
    await page.locator('#auth-password').fill('secret123');
    await page.locator('#auth-submit').click();
  }

  await page.waitForSelector('html.modules-ready');
  await expect(page.locator('.top-nav-bar')).toBeVisible();
});
