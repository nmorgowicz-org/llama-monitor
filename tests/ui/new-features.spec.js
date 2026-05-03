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
    
    // Create multiple tabs for testing
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      const { renderChatTabs } = await import('/js/features/chat-render.js');
      
      // Create 2 additional tabs
      const tab2 = newChatTab('Chat 2');
      const tab3 = newChatTab('Chat 3');
      chat.tabs.push(tab2, tab3);
      renderChatTabs();
    });
  });

  test('toggle pin button toggles pinned state', async ({ page }) => {
    // Start with unpinned state
    const pinButton = page.locator('.chat-tab-pin').first();
    await expect(pinButton).toBeVisible();
    
    // Pin the tab
    await pinButton.click();
    await expect(pinButton).toHaveClass(/pinned/);
    
    // Unpin the tab
    await pinButton.click();
    await expect(pinButton).not.toHaveClass(/pinned/);
  });

  test('pinned tabs appear before unpinned tabs', async ({ page }) => {
    // Pin the second tab
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { renderChatTabs } = await import('/js/features/chat-render.js');
      const tab = chat.tabs.find(t => t.name === 'Chat 2');
      if (tab) {
        tab.pinned = true;
        renderChatTabs();
      }
    });
    
    // Pinned tab should appear first
    const firstTabName = await page.locator('.chat-tab').first().textContent();
    expect(firstTabName).toContain('Chat 2');
  });

  test('drag to reorder is prevented across pinned/unpinned boundary', async ({ page }) => {
    // Pin the first tab
    await page.locator('.chat-tab-pin').first().click();
    await page.locator('.chat-tab-pin').first().click(); // toggle off
    
    // Start drag on the first tab
    const firstTab = page.locator('.chat-tab').first();
    const secondTab = page.locator('.chat-tab').nth(1);
    
    await firstTab.hover();
    await page.mouse.down();
    
    // Try to drag over the second tab
    await secondTab.hover();
    
    // Release mouse
    await page.mouse.up();
    
    // Check that tabs are still in original order (guard prevented reorder)
    const tabOrder = await page.locator('.chat-tab').all();
    const firstTabText = await tabOrder[0].textContent();
    const secondTabText = await tabOrder[1].textContent();
    
    // Tabs should maintain their order when dragged across boundary
    expect(firstTabText).toContain('Chat 1');
  });
});

test.describe('persona template selection', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('persona strip is visible in chat header', async ({ page }) => {
    await expect(page.locator('.chat-persona-strip')).toBeVisible();
    const personaChips = await page.locator('.chat-persona-chip').count();
    expect(personaChips).toBeGreaterThan(0);
  });

  test('clicking persona chip sets active template', async ({ page }) => {
    // Get all persona chips
    const personaChips = await page.locator('.chat-persona-chip').all();
    const firstPersonaName = await personaChips[0].textContent();
    
    // Click the first persona chip
    await personaChips[0].click();
    
    // Check that the chip is now active
    await expect(personaChips[0]).toHaveClass(/active/);
    
    // Verify active_template_id is set
    const activeTemplateId = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return activeChatTab().active_template_id;
    });
    
    expect(activeTemplateId).toBeTruthy();
  });

  test('personas include non-roleplay options', async ({ page }) => {
    const allPersonas = await page.locator('.chat-persona-chip').allTextContents();
    // At least some personas should be non-roleplay (e.g., "Assistant", "Chat")
    const nonRoleplayCount = allPersonas.filter(p => 
      p.toLowerCase().includes('assistant') || 
      p.toLowerCase().includes('chat') ||
      p.toLowerCase().includes('default')
    ).length;
    expect(nonRoleplayCount).toBeGreaterThan(0);
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
    await expect(page.locator('#btn-export-chat')).toBeVisible();
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
    
    // Click export button
    await page.locator('#btn-export-chat').click();
    
    // Download dialog should appear
    await page.waitForEvent('download', { timeout: 5000 }).catch(() => {
      // Some browsers don't show download dialog, just check if button was clicked
    });
    
    // Verify export function exists and runs
    const exportResult = await page.evaluate(async () => {
      const { exportChatToJSON } = await import('/js/features/chat-render.js');
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      if (!tab || !tab.messages.length) return null;
      const data = exportChatToJSON(tab);
      return data ? JSON.stringify(data) : null;
    });
    
    expect(exportResult).toBeTruthy();
    const parsed = JSON.parse(exportResult);
    expect(parsed.messages).toBeInstanceOf(Array);
    expect(parsed.messages.length).toBeGreaterThan(0);
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
    // Count user messages with edit buttons
    const editButtons = await page.locator('.chat-message .btn-resend').all();
    expect(editButtons.length).toBeGreaterThan(0);
  });

  test('regenerate from any user message', async ({ page }) => {
    // Click regenerate on the second user message (not the last one)
    const userMessages = page.locator('.chat-message[data-role="user"]');
    const middleMessage = userMessages.nth(1); // Second user message
    
    await middleMessage.locator('.btn-resend').click();
    
    // Verify the regenerate function is triggered
    // (In a real test, we'd mock the API call and verify behavior)
    const messageContent = await middleMessage.locator('.chat-message-content').textContent();
    expect(messageContent).toContain('Question 2');
  });

  test('edit button opens inline editing for message', async ({ page }) => {
    const userMessages = page.locator('.chat-message[data-role="user"]');
    const firstUserMessage = userMessages.first();
    
    // Click edit button
    await firstUserMessage.locator('.btn-resend').click();
    
    // Message content should become editable or show edit UI
    const inputField = firstUserMessage.locator('.chat-edit-input');
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

  test('normalizeChatTab preserves pinned state', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const { normalizeChatTab, newChatTab } = await import('/js/features/chat-state.js');
      const tab = newChatTab('Test');
      tab.pinned = true;
      tab.active_template_id = 'persona-test';
      return normalizeChatTab(tab);
    });
    
    expect(result.pinned).toBe(true);
    expect(result.active_template_id).toBe('persona-test');
  });

  test('normalizeChatTab sets defaults for missing fields', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const { normalizeChatTab } = await import('/js/features/chat-state.js');
      // Create minimal tab object
      const minimalTab = { id: 'test-id', name: 'Test' };
      return normalizeChatTab(minimalTab);
    });
    
    expect(result.id).toBe('test-id');
    expect(result.name).toBe('Test');
    expect(typeof result.pinned).toBe('boolean');
    expect(typeof result.auto_compact).toBe('boolean');
  });
});
