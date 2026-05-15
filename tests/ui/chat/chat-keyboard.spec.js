import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('global escape key handling', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('Escape closes settings modal', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-modal')).not.toHaveClass(/open/, { timeout: 3000 });
  });

  test('Escape closes keyboard shortcuts modal', async ({ page }) => {
    await page.keyboard.down('Control');
    await page.keyboard.press('/');
    await page.keyboard.up('Control');
    await page.waitForSelector('#keyboard-shortcuts-modal.open', { timeout: 5000 });

    await page.keyboard.press('Escape');
    await expect(page.locator('#keyboard-shortcuts-modal')).not.toHaveClass(/open/, { timeout: 3000 });
  });
});
