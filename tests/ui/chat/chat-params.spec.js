import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
  await expect(page.locator('body')).not.toHaveClass(/setup-active/);
  await expect(page.locator('#view-monitor')).toBeVisible();
}

test.describe('model params panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });
  });

  test('opens and closes via .open class', async ({ page }) => {
    await expect(page.locator('#chat-params-panel')).not.toHaveClass(/open/);
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#chat-params-panel')).toHaveClass(/open/);
    await expect(page.locator('#chat-params-panel')).toBeVisible();
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#chat-params-panel')).not.toHaveClass(/open/);
  });

  test('shows temperature and top_p controls', async ({ page }) => {
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#param-temperature')).toBeVisible();
    await expect(page.locator('#param-top-p')).toBeVisible();
  });

  test('temperature slider is interactive', async ({ page }) => {
    await page.locator('#btn-model-params').click();
    const tempSlider = page.locator('#param-temperature');
    await expect(tempSlider).toBeVisible();
    // Set a specific value
    await tempSlider.fill('0.5');
    await expect(tempSlider).toHaveValue('0.5');
  });
});

test.describe('max_tokens default', () => {
  test('new tabs default max_tokens to 4096', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    const value = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      return tab?.model_params?.max_tokens ?? null;
    });

    expect(value).toBe(4096);
  });
});
