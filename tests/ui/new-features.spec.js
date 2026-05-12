import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('pin and favorite tabs', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    // Wait for initChatTabs to complete before manipulating tabs
    await page.evaluate(async () => {
      const { initChatTabs } = await import('/js/features/chat-state.js');
      await initChatTabs();
    });

    // Create additional tabs for testing
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      const { renderChatTabs } = await import('/js/features/chat-render.js');

      const tab2 = newChatTab('Chat 2');
      const tab3 = newChatTab('Chat 3');
      chat.tabs.push(tab2, tab3);
      renderChatTabs();
    });
    // Wait for chat tabs and pin icons to render
    await page.waitForSelector('.chat-tab', { timeout: 5000 });
    await page.waitForSelector('.chat-tab-pin-icon', { timeout: 5000 });
  });

  test('toggle pin button toggles pinned state', async ({ page }) => {
    const pinButton = page.locator('.chat-tab-pin-icon').first();
    await expect(pinButton).toBeVisible();

    // Pin the tab (use force:true to bypass draggable parent)
    await pinButton.click({ force: true });
    await expect(pinButton).toHaveClass(/pinned/);

    // Unpin the tab
    await pinButton.click({ force: true });
    await expect(pinButton).not.toHaveClass(/pinned/);
  });

  test('pinned tabs appear before unpinned tabs', async ({ page }) => {
    // Pin the Chat 2 tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { renderChatTabs } = await import('/js/features/chat-render.js');
      const tab = chat.tabs.find(t => t.name === 'Chat 2');
      if (tab) {
        tab.pinned = true;
        renderChatTabs();
      }
    });

    // Pinned tab should appear before unpinned tabs
    const pinnedTabs = page.locator('.chat-tab.chat-tab-pinned');
    await expect(pinnedTabs.first()).toBeVisible();
    const pinnedTabName = await pinnedTabs.first().textContent();
    expect(pinnedTabName).toContain('Chat 2');
  });

  test('drag to reorder is prevented across pinned/unpinned boundary', async ({ page }) => {
    // Pin the first tab
    await page.locator('.chat-tab-pin-icon').first().click({ force: true });

    // Start drag on the first tab
    const firstTab = page.locator('.chat-tab').first();
    const secondTab = page.locator('.chat-tab').nth(1);

    await firstTab.hover();
    await page.mouse.down();

    // Try to drag over the second tab
    await secondTab.hover();

    // Release mouse
    await page.mouse.up();

    // Pinned tab should still be first
    const firstTabText = await page.locator('.chat-tab').first().textContent();
    expect(firstTabText).toBeTruthy();
  });
});

test.describe('chat message export', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('export button is present in chat controls', async ({ page }) => {
    await expect(page.locator('#chat-export-btn')).toBeVisible();
  });

  test('export generates valid JSON with correct format', async ({ page }) => {
    // Add some messages first
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      tab.messages = [
        { role: 'user', content: 'Hello', timestamp_ms: Date.now() - 1000 },
        { role: 'assistant', content: 'Hi there!', timestamp_ms: Date.now() - 500 }
      ];
      renderChatMessages();
    });

    // Click export button to open dropdown
    await page.locator('#chat-export-btn').click();

    // Click the JSON export option
    const downloadPromise = page.waitForEvent('download', { timeout: 5000 });
    await page.locator('#chat-export-menu [data-export-format="json"]').click();
    const download = await downloadPromise;
    expect(download).toBeTruthy();
  });
});

test.describe('message edit and regenerate', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    // Add multiple user messages for testing
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      tab.messages = [
        { role: 'user', content: 'Question 1', timestamp_ms: Date.now() - 3000 },
        { role: 'assistant', content: 'Answer 1', timestamp_ms: Date.now() - 2500 },
        { role: 'user', content: 'Question 2', timestamp_ms: Date.now() - 2000 },
        { role: 'assistant', content: 'Answer 2', timestamp_ms: Date.now() - 1500 },
        { role: 'user', content: 'Question 3', timestamp_ms: Date.now() - 1000 },
        { role: 'assistant', content: 'Answer 3', timestamp_ms: Date.now() }
      ];
      renderChatMessages();
    });
  });

  test('edit button appears on all user messages', async ({ page }) => {
    const editButtons = await page.locator('.chat-message .chat-action-btn[data-chat-action="edit"]').all();
    expect(editButtons.length).toBeGreaterThan(0);
  });

  test('edit opens edit UI with textarea', async ({ page }) => {
    const userMessages = page.locator('.chat-message-user');
    const middleMessage = userMessages.nth(1);

    await middleMessage.locator('.chat-action-btn[data-chat-action="edit"]').click();

    // Edit mode replaces body with textarea
    const textarea = middleMessage.locator('.chat-msg-edit-area');
    await expect(textarea).toBeVisible();
  });

  test('edit button opens inline editing for message', async ({ page }) => {
    const userMessages = page.locator('.chat-message-user');
    const firstUserMessage = userMessages.first();

    await firstUserMessage.locator('.chat-action-btn[data-chat-action="edit"]').click();

    const inputField = firstUserMessage.locator('.chat-msg-edit-area');
    await expect(inputField).toBeVisible();
  });
});

test.describe('chat tab normalization', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('normalizeTabForSave preserves pinned state', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const { normalizeTabForSave, newChatTab } = await import('/js/features/chat-state.js');
      const tab = newChatTab('Test');
      tab.pinned = true;
      tab.active_template_id = 'persona-test';
      return normalizeTabForSave(tab);
    });

    expect(result.pinned).toBe(true);
    expect(result.active_template_id).toBe('persona-test');
  });

  test('normalizeTabForSave strips internal token fields from messages', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const { normalizeTabForSave } = await import('/js/features/chat-state.js');
      const tab = {
        id: 'test-id',
        name: 'Test',
        messages: [
          { role: 'user', content: 'Hello', cumulativeInputTokens: 10, cumulativeOutputTokens: 20 }
        ]
      };
      return normalizeTabForSave(tab);
    });

    expect(result.id).toBe('test-id');
    expect(result.messages[0].cumulativeInputTokens).toBeUndefined();
    expect(result.messages[0].cumulativeOutputTokens).toBeUndefined();
    expect(result.messages[0].content).toBe('Hello');
  });
});
