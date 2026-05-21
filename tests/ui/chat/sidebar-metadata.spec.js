// ── Sidebar Message Count Tests ────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('sidebar message count', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
    await page.waitForSelector('#csp-list .csp-item', { timeout: 10000 });
  });

  test('shows message count for inactive tab', async ({ page }) => {
    // Create a second tab and add messages to the first tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      // Add messages to the first tab
      const firstTab = chat.tabs[0];
      firstTab.messages = [
        { role: 'user', content: 'Hello', timestamp_ms: Date.now() - 2000 },
        { role: 'assistant', content: 'Hi there!', timestamp_ms: Date.now() - 1000 },
      ];

      // Create and switch to second tab
      const tab2 = newChatTab('Second Tab');
      chat.tabs.push(tab2);
      await switchChatTab(tab2.id);
      renderChatSessionsSidebar();
    });

    // First tab should show message count in sidebar
    const firstItem = page.locator('#csp-list .csp-item').first();
    const countEl = firstItem.locator('.csp-item-count');
    await expect(countEl).toBeVisible();
    await expect(countEl).toContainText('2 msgs');
  });

  test('inactive tab uses message_count from backend when not loaded', async ({ page }) => {
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      // Simulate unloaded tab with backend message_count
      const firstTab = chat.tabs[0];
      firstTab.messages = null;
      firstTab._loaded = false;
      firstTab.message_count = 5;

      // Switch to second tab
      const tab2 = newChatTab('Tab 2');
      chat.tabs.push(tab2);
      await switchChatTab(tab2.id);
      renderChatSessionsSidebar();
    });

    const firstItem = page.locator('#csp-list .csp-item').first();
    const countEl = firstItem.locator('.csp-item-count');
    await expect(countEl).toBeVisible();
    await expect(countEl).toContainText('5 msgs');
  });

  test('archived tab shows message count', async ({ page }) => {
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { archiveChatTab } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      // Add messages to first tab
      const firstTab = chat.tabs[0];
      firstTab.messages = [
        { role: 'user', content: 'Message 1', timestamp_ms: Date.now() - 3000 },
        { role: 'assistant', content: 'Reply 1', timestamp_ms: Date.now() - 2000 },
        { role: 'user', content: 'Message 2', timestamp_ms: Date.now() - 1000 },
      ];

      // Archive the first tab
      archiveChatTab(firstTab.id);
      renderChatSessionsSidebar();
    });

    // Find archived section and check message count
    const archivedItem = page.locator('.csp-item-archived');
    await expect(archivedItem).toBeVisible();
    const countEl = archivedItem.locator('.csp-item-count');
    await expect(countEl).toBeVisible();
    const countText = await countEl.textContent();
    expect(parseInt(countText)).toBe(3);
  });
});
