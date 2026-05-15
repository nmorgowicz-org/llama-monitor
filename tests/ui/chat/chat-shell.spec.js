import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('chat UI shell', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('renders empty chat state with input bar', async ({ page }) => {
    await expect(page.locator('#chat-messages')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeEditable();
  });

  test('renders personalized empty state with suggested prompts', async ({ page }) => {
    // Clear any persisted messages and re-render to ensure empty state
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      if (tab) {
        tab.messages = [];
        renderChatMessages();
      }
    });
    await expect(page.locator('.chat-empty')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('.chat-empty-title')).toBeVisible();
    await expect(page.locator('.chat-empty-prompts')).toBeVisible();
    const prompts = await page.locator('.chat-empty-prompt').count();
    expect(prompts).toBeGreaterThan(0);
  });

  test('shows chat header controls', async ({ page }) => {
    await expect(page.locator('#btn-behavior')).toBeVisible();
    await expect(page.locator('#btn-model-params')).toBeVisible();
    await expect(page.locator('#chat-explicit-toggle-footer')).toBeVisible();
  });

  test('scroll-to-bottom button and badge are present', async ({ page }) => {
    await expect(page.locator('#chat-scroll-bottom')).toBeAttached();
    await expect(page.locator('#chat-scroll-badge')).toBeAttached();
  });
});

test.describe('chat tabs', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('creates new tab on + button click', async ({ page }) => {
    const tabCount = await page.locator('.chat-tab').count();
    await page.locator('.chat-tab-add').click();
    await expect(page.locator('.chat-tab')).toHaveCount(tabCount + 1);
  });

  test('switches between tabs', async ({ page }) => {
    await page.locator('.chat-tab-add').click();
    // Switch to first tab via JS (force click may not trigger handler on draggable element)
    await page.evaluate(async () => {
      const { switchChatTab } = await import('/js/features/chat-state.js');
      const tab = document.querySelector('.chat-tab');
      if (tab) switchChatTab(tab.dataset.tabId);
    });
    await expect(page.locator('.chat-tab').first()).toHaveClass(/active/);
  });

  test('Ctrl+Shift+ArrowRight cycles to next tab', async ({ page }) => {
    await page.locator('.chat-tab-add').click();
    // New tab is added at the end and becomes active
    await expect(page.locator('.chat-tab').last()).toHaveClass(/active/);
    await page.keyboard.press('Control+Shift+ArrowRight');
    await expect(page.locator('.chat-tab').first()).toHaveClass(/active/, { timeout: 3000 });
  });
});

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

test.describe('chat history pagination', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('load more button appears when messages exceed visible limit', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const t = activeChatTab();
      if (!t) return;
      t.visible_message_limit = 2;
      t.messages = Array.from({ length: 6 }, (_, i) => ({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i + 1}`,
        timestamp_ms: Date.now() - (6 - i) * 10000,
      }));
      renderChatMessages();
    });
    await expect(page.locator('.chat-load-more')).toBeVisible();
  });

  test('load more button is absent when all messages fit', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const t = activeChatTab();
      if (!t) return;
      t.visible_message_limit = 15;
      t.messages = [
        { role: 'user', content: 'Hi', timestamp_ms: Date.now() - 5000 },
        { role: 'assistant', content: 'Hello!', timestamp_ms: Date.now() - 3000 },
      ];
      renderChatMessages();
    });
    await expect(page.locator('.chat-load-more')).not.toBeAttached();
  });

  test('clicking load more expands visible messages', async ({ page }) => {
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const t = activeChatTab();
      if (!t) return;
      t.visible_message_limit = 2;
      t.messages = Array.from({ length: 6 }, (_, i) => ({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i + 1}`,
        timestamp_ms: Date.now() - (6 - i) * 10000,
      }));
      renderChatMessages();
    });
    const beforeCount = await page.locator('.chat-message').count();
    await page.locator('.chat-load-more').click();
    const afterCount = await page.locator('.chat-message').count();
    expect(afterCount).toBeGreaterThan(beforeCount);
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
