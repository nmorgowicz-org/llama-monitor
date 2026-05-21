import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
  await expect(page.locator('body')).not.toHaveClass(/setup-active/);
  await expect(page.locator('#view-monitor')).toBeVisible();
}

test.describe('debug prompt inspector', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });
  });

  test('shows empty state before any send', async ({ page }) => {
    // Open the Tools dropdown, then click Prompt Debug
    await page.locator('#btn-debug-dropdown').click();
    await page.waitForSelector('#debug-dropdown-menu', { state: 'visible' });
    await page.locator('#btn-debug-prompt').click();

    await expect(page.locator('#debug-prompt-modal')).toHaveClass(/debug-modal-overlay/);
    await expect(page.locator('#debug-empty-state')).toBeVisible();
    await expect(page.locator('#debug-content')).toHaveClass(/hidden/);
  });

  test('populated after sending a message', async ({ page }) => {
    // Route chat API to return a minimal payload with debug data
    await page.route('**/api/chat', async route => {
      const req = route.request();
      const postData = req.postDataJSON || {};
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          message: { role: 'assistant', content: 'OK' },
          debug: {
            system_prompt_parts: [
              { label: 'System', tokens: 20, text: 'You are an assistant.' }
            ],
            history_tokens: 10,
            total_tokens: 30,
            capacity: 8192,
            model_params: { temperature: 0.7 },
            prompt_ms: 10,
            gen_ms: 20,
          },
        }),
      });
    });

    // Send a message
    const sendBtn = page.locator('#btn-send');
    await sendBtn.waitFor({ state: 'visible', timeout: 5000 });
    await page.$eval('#chat-input', (el, text) => {
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'test');
    await sendBtn.click();
    await page.waitForSelector('#chat-messages .chat-message-user', { timeout: 5000 });

    // Open the Tools dropdown, then click Prompt Debug
    await page.locator('#btn-debug-dropdown').click();
    await page.waitForSelector('#debug-dropdown-menu', { state: 'visible' });
    await page.locator('#btn-debug-prompt').click();

    // Content should be visible and empty state hidden
    await expect(page.locator('#debug-empty-state')).not.toBeVisible({ timeout: 5000 });
    await expect(page.locator('#debug-content')).not.toHaveClass(/hidden/);

    // Hero stats present
    await expect(page.locator('#debug-stat-utilization')).toBeVisible();
    await expect(page.locator('#debug-stat-total')).toBeVisible();
  });
});
