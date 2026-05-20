import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('focus mode', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('focus mode button toggles body class', async ({ page }) => {
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
    await page.locator('#chat-focus-mode-btn').click();
    await expect(page.locator('body')).toHaveClass(/chat-focus-mode/);
    await page.locator('#chat-focus-mode-btn').click();
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
  });

  test('Cmd/Ctrl+Shift+F toggles focus mode', async ({ page }) => {
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
    await page.keyboard.press('Control+Shift+F');
    await expect(page.locator('body')).toHaveClass(/chat-focus-mode/);
    await page.keyboard.press('Control+Shift+F');
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
  });

  test('switching tabs exits focus mode', async ({ page }) => {
    await page.locator('#chat-focus-mode-btn').click();
    await expect(page.locator('body')).toHaveClass(/chat-focus-mode/);
    // Sidebar is hidden in focus mode — switch programmatically as keyboard shortcuts do
    await page.evaluate(async () => {
      const { switchTab } = await import('/js/features/nav.js');
      switchTab('logs');
    });
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
  });

  test('exit pill is visible in focus mode', async ({ page }) => {
    await page.locator('#chat-focus-mode-btn').click();
    await expect(page.locator('#focus-mode-exit-pill')).toBeVisible();
    await page.locator('#focus-mode-exit-beacon').click();
    await expect(page.locator('body')).not.toHaveClass(/chat-focus-mode/);
  });
});

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
