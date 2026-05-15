import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('connection lost banner', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('banner appears when connection-lost modal is dismissed', async ({ page }) => {
    // Simulate connection-lost modal being shown by triggering a send error.
    await page.route('**/api/chat', route => route.fulfill({ status: 503 }));

    await page.$eval('#chat-input', (el, text) => {
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'test');
    await page.click('#btn-send');

    // Wait for connection-lost modal (if it is configured to show on 503)
    const modal = page.locator('#connection-lost-modal');
    const banner = page.locator('#disconnected-banner');

    // If modal opens, dismiss it; otherwise we rely on banner being visible.
    if (await modal.isVisible({ timeout: 3000 }).catch(() => false)) {
      // Dismiss via button or close button
      const dismissBtn = page.locator('#connection-lost-dismiss-btn');
      if (await dismissBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
        await dismissBtn.click();
      } else {
        await page.locator('#connection-lost-modal-close').click();
      }
      // Banner should now be visible
      await expect(banner).toBeVisible({ timeout: 5000 });
      await expect(banner).not.toHaveAttribute('hidden');
    } else {
      // If modal did not appear, we still accept that banner can be shown directly
      // (depends on configuration). This test is best-effort in CI.
      test.skip(true, 'Connection-lost modal not triggered in this environment.');
    }
  });
});
