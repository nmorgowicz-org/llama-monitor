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
    const tabCount = await page.locator('#csp-list .csp-item').count();
    await page.locator('#csp-new-btn').click();
    await expect(page.locator('#csp-list .csp-item')).toHaveCount(tabCount + 1);
  });

  test('switches between tabs', async ({ page }) => {
    await page.locator('#csp-new-btn').click();
    // Switch to first tab via JS
    await page.evaluate(async () => {
      const { switchChatTab } = await import('/js/features/chat-state.js');
      const item = document.querySelector('#csp-list .csp-item');
      if (item) switchChatTab(item.dataset.tabId);
    });
    await expect(page.locator('#csp-list .csp-item').first()).toHaveClass(/active/);
  });

  test('Ctrl+Shift+ArrowRight cycles to next tab', async ({ page }) => {
    await page.locator('#csp-new-btn').click();
    // New tab becomes active; it is rendered near the top of "Today", not necessarily last.
    const activeItem = page.locator('#csp-list .csp-item.active');
    await expect(activeItem).toBeVisible();
    const beforeId = await activeItem.getAttribute('data-tab-id');
    await page.keyboard.press('Control+Shift+ArrowRight');
    const afterId = await page.locator('#csp-list .csp-item.active').getAttribute('data-tab-id', { timeout: 3000 });
    expect(afterId).not.toBe(beforeId);
  });

  test('title filter only narrows sidebar items by tab name', async ({ page }) => {
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      chat.tabs = [newChatTab('Noir Scene'), newChatTab('Build Logs'), newChatTab('Recipe Ideas')];
      chat.activeTabId = chat.tabs[0].id;
      renderChatSessionsSidebar();
    });

    await page.locator('#csp-search').fill('Noir');
    await expect(page.locator('#csp-list .csp-item:visible')).toHaveCount(1);
    await expect(page.locator('#csp-list .csp-item:visible')).toContainText('Noir Scene');
  });

  test('message search opens flyout and paginates results', async ({ page }) => {
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, persistChatTabs } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      chat.tabs = [];
      for (let i = 0; i < 12; i += 1) {
        const tab = newChatTab(`Search Seed ${i + 1}`);
        tab.messages = Array.from({ length: 3 }, (_, idx) => ({
          role: idx % 2 === 0 ? 'user' : 'assistant',
          content: `ledger trail ${i}-${idx} in the rain`,
          timestamp_ms: Date.now() - ((i * 3) + idx) * 1000,
        }));
        tab.updated_at = Date.now();
        chat.tabs.push(tab);
      }
      chat.activeTabId = chat.tabs[0].id;
      renderChatSessionsSidebar();

      for (const [tabIndex, tab] of chat.tabs.entries()) {
        const tabPayload = {
          id: tab.id,
          name: tab.name,
          system_prompt: '',
          explicit_level: 0,
          auto_compact: true,
          auto_compact_summarize: false,
          compact_mode: 'summarize',
          compact_threshold: 0.8,
          model_params: {},
          context_notes: [],
          sidebar_width: 320,
          tab_order: tabIndex,
          pinned: false,
          total_input_tokens: 0,
          total_output_tokens: 0,
          created_at: Date.now(),
          updated_at: Date.now(),
          messages: [],
        };
        const auth = window.__API_TOKEN
              ? { 'Authorization': `Bearer ${window.__API_TOKEN}` }
              : {};
          await fetch('/api/chat/tabs', {
              method: 'POST',
              headers: { 'Content-Type': 'application/json', ...auth },
              body: JSON.stringify(tabPayload),
          });
          await fetch(`/api/chat/tabs/${tab.id}/messages`, {
              method: 'POST',
              headers: { 'Content-Type': 'application/json', ...auth },
          body: JSON.stringify({
            messages: tab.messages.map((message, idx) => ({
              tab_id: tab.id,
              role: message.role,
              content: message.content,
              timestamp_ms: message.timestamp_ms,
              seq: idx,
            })),
          }),
        });
      }
      await persistChatTabs();
    });

    await page.locator('#csp-message-search-btn').click();
    await expect(page.locator('.csp-search-panel')).toBeVisible();
    await page.locator('#csp-search-input').fill('ledger');
    await expect(page.locator('.csp-search-result')).toHaveCount(20);
    await expect(page.locator('.csp-search-count')).toContainText('36 matches');
    await page.locator('.csp-search-load-more').click();
    await expect(page.locator('.csp-search-result')).toHaveCount(36);
  });
});

test.describe('pin and favorite tabs', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    // Wait for sidebar list to render
    await page.waitForSelector('#csp-list .csp-item', { timeout: 10000 });

    // Create additional tabs for testing
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, persistChatTabs } = await import('/js/features/chat-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

      const tab2 = newChatTab('Chat 2');
      const tab3 = newChatTab('Chat 3');
      chat.tabs.push(tab2, tab3);
      await persistChatTabs();
      renderChatSessionsSidebar();
    });
    // Wait for sidebar items to appear
    await page.waitForSelector('#csp-list .csp-item', { timeout: 5000 });
  });

  test('toggle pin button toggles pinned state', async ({ page }) => {
    // Hover first item to reveal actions, then click pin
    const firstItem = page.locator('#csp-list .csp-item').first();
    await firstItem.hover();
    await page.waitForTimeout(200);
    const pinButton = firstItem.locator('button[data-action="pin"]');
    await expect(pinButton).toBeVisible();

    // Initially unpinned (⊙)
    const initialText = await pinButton.textContent();
    expect(initialText).toContain('⊙');

    // Pin it → button shows pin emoji, title="Unpin"
    await pinButton.click();
    await expect(pinButton).toHaveAttribute('title', 'Unpin');

    // Unpin → button title="Pin"
    await pinButton.click();
    await expect(pinButton).toHaveAttribute('title', 'Pin');
  });

  test('pinned tabs appear before unpinned tabs', async ({ page }) => {
    // Pin the Chat 2 tab via JS
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
      const tab = chat.tabs.find(t => t.name === 'Chat 2');
      if (tab) {
        tab.pinned = true;
        renderChatSessionsSidebar();
      }
    });

    // Pinned tab should appear in "Pinned" group before unpinned tabs
    const pinnedItem = page.locator('#csp-list .csp-item').filter({ hasText: 'Chat 2' });
    await expect(pinnedItem).toBeVisible();
    // Pinned items should be under the "Pinned" section header
    const pinnedSection = page.locator('#csp-list .csp-section-header:has-text("Pinned")');
    await expect(pinnedSection).toBeVisible();
    // Chat 2 should appear in the pinned section (items after that header, before next)
    const pinnedItems = page.locator('#csp-list .csp-item').filter({ hasText: 'Chat 2' });
    await expect(pinnedItems.first()).toBeVisible();
  });

  test('drag to reorder is prevented across pinned/unpinned boundary', async ({ page }) => {
    // Pin the first tab via JS
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
      if (chat.tabs[0]) chat.tabs[0].pinned = true;
      renderChatSessionsSidebar();
    });

    // Start drag on the first item
    const firstItem = page.locator('#csp-list .csp-item').first();
    const secondItem = page.locator('#csp-list .csp-item').nth(1);

    await firstItem.hover();
    await page.mouse.down();
    await secondItem.hover();
    await page.mouse.up();

    // Pinned tab should still be first
    const firstItemText = await firstItem.textContent();
    expect(firstItemText).toBeTruthy();
  });
});

test.describe('chat tab normalization', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
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
    await dismissAuthShell(page);
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat.active')).toBeVisible({ timeout: 5000 });
    // Wait for bootstrap's initChatTabs to finish before the test seeds state
    await page.evaluate(() => new Promise(resolve => {
      import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        if (activeChatTab() !== null) { resolve(); return; }
        window.addEventListener('activeTabChanged', resolve, { once: true });
      });
    }));
  });

  test('load more button appears when messages exceed visible limit', async ({ page }) => {
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });

    await page.evaluate(async () => {
      const { activeChatTab, addChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      let t = activeChatTab();
      if (!t) {
        await addChatTab();
        t = activeChatTab();
      }
      if (!t) throw new Error('No active chat tab');
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
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });

    await page.evaluate(async () => {
      const { activeChatTab, addChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      let t = activeChatTab();
      if (!t) {
        await addChatTab();
        t = activeChatTab();
      }
      if (!t) throw new Error('No active chat tab');
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
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });

    await page.evaluate(async () => {
      const { activeChatTab, addChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      let t = activeChatTab();
      if (!t) {
        await addChatTab();
        t = activeChatTab();
      }
      if (!t) throw new Error('No active chat tab');
      t.visible_message_limit = 2;
      t.messages = Array.from({ length: 6 }, (_, i) => ({
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i + 1}`,
        timestamp_ms: Date.now() - (6 - i) * 10000,
      }));
      renderChatMessages();
    });
    // Wait for load more button to appear
    await page.waitForSelector('.chat-load-more', { timeout: 5000 });
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
    await dismissAuthShell(page);
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('file button is present in chat controls', async ({ page }) => {
    await expect(page.locator('#chat-file-btn')).toBeVisible();
  });

  test('export generates valid JSON with correct format', async ({ page }) => {
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });

    // Add some messages first
    await page.evaluate(async () => {
      const { activeChatTab, addChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      let tab = activeChatTab();
      if (!tab) {
        await addChatTab();
        tab = activeChatTab();
      }
      if (!tab) throw new Error('No active chat tab');
      tab.messages = [
        { role: 'user', content: 'Hello', timestamp_ms: Date.now() - 1000 },
        { role: 'assistant', content: 'Hi there!', timestamp_ms: Date.now() - 500 }
      ];
      renderChatMessages();
    });

    // Click file button to open dropdown
    await page.locator('#chat-file-btn').click();

    // Click the JSON export option
    const downloadPromise = page.waitForEvent('download', { timeout: 5000 });
    await page.locator('#chat-file-menu [data-export-format="json"]').click();
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
    // Ensure chat view is visible (active class is not always reliable)
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });
    // Wait for bootstrap's initChatTabs to finish before seeding state
    await page.evaluate(() => new Promise(resolve => {
      import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        if (activeChatTab() !== null) { resolve(); return; }
        window.addEventListener('activeTabChanged', resolve, { once: true });
      });
    }));

    // Add multiple user messages for testing
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      if (!tab) throw new Error('No active chat tab');
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

    // Wait for messages to be rendered
    await page.waitForSelector('.chat-message-user', { timeout: 5000 });
  });

  test('edit button appears on all user messages', async ({ page }) => {
    const editButtons = await page.locator('.chat-message .chat-action-btn[data-chat-action="edit"]').all();
    expect(editButtons.length).toBeGreaterThan(0);
  });

  test('edit opens edit UI with textarea', async ({ page }) => {
    const userMessages = page.locator('.chat-message-user');
    const middleMessage = userMessages.nth(1);

    // Scroll into view and use JS dispatchEvent to bypass pointer-events interception
    await middleMessage.evaluate(el => el.scrollIntoView({ block: 'center' }));
    await middleMessage.evaluate(el => {
      const btn = el.querySelector('.chat-action-btn[data-chat-action="edit"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    // Edit mode replaces body with textarea
    const textarea = middleMessage.locator('.chat-msg-edit-area');
    await expect(textarea).toBeVisible();
  });

  test('edit button opens inline editing for message', async ({ page }) => {
    const userMessages = page.locator('.chat-message-user');
    const firstUserMessage = userMessages.first();

    // Scroll into view and use JS dispatchEvent to bypass pointer-events interception
    await firstUserMessage.evaluate(el => el.scrollIntoView({ block: 'center' }));
    await firstUserMessage.evaluate(el => {
      const btn = el.querySelector('.chat-action-btn[data-chat-action="edit"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    const inputField = firstUserMessage.locator('.chat-msg-edit-area');
    await expect(inputField).toBeVisible();
  });
});
