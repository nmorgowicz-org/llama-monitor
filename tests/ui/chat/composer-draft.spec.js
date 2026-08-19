// ── Composer Draft Persistence Tests ─────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('composer draft persistence', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.waitForSelector('#view-monitor', { state: 'attached', timeout: 5000 });
    await page.waitForFunction(() => {
      const monitor = document.getElementById('view-monitor');
      return monitor && getComputedStyle(monitor).display !== 'none';
    }, { timeout: 5000 });
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
    await page.waitForSelector('#csp-list .csp-item', { timeout: 10000 });
  });

  test('draft text is restored after tab switch', async ({ page }) => {
    const draftText = 'This is my draft message';

    // Type text into composer
    await page.locator('#chat-input').fill(draftText);
    await expect(page.locator('#chat-input')).toHaveValue(draftText);

    // Create and switch to another tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
      const tab2 = newChatTab('Other Tab');
      chat.tabs.push(tab2);
      await switchChatTab(tab2.id);
    });

    // Switch back to first tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { switchChatTab } = await import('/js/features/chat-state.js');
      await switchChatTab(chat.tabs[0].id);
    });

    // Draft should be restored
    await expect(page.locator('#chat-input')).toHaveValue(draftText, { timeout: 3000 });
  });

  test('draft persists per tab independently', async ({ page }) => {
    // Type draft in first tab
    await page.locator('#chat-input').fill('Draft for tab 1');
    await expect(page.locator('#chat-input')).toHaveValue('Draft for tab 1');

    // Create second tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
      const tab2 = newChatTab('Tab 2');
      chat.tabs.push(tab2);
      await switchChatTab(tab2.id);
    });

    // Second tab should have empty composer
    await expect(page.locator('#chat-input')).toHaveValue('');

    // Type draft in second tab
    await page.locator('#chat-input').fill('Draft for tab 2');
    await expect(page.locator('#chat-input')).toHaveValue('Draft for tab 2');

    // Switch back to first tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { switchChatTab } = await import('/js/features/chat-state.js');
      await switchChatTab(chat.tabs[0].id);
    });

    // First tab draft should still be there
    await expect(page.locator('#chat-input')).toHaveValue('Draft for tab 1', { timeout: 3000 });
  });

  test('draft stored on input event', async ({ page }) => {
    await page.locator('#chat-input').fill('Test draft');

    const draftValue = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return activeChatTab()?.composer_draft ?? null;
    });

    expect(draftValue).toBe('Test draft');
  });
});
