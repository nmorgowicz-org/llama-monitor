import { test, expect } from '@playwright/test';

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

const TEST_TAB_PREFIX = '[TEST]';

async function createCompactionTestTab(page) {
  // Clean up any leftover test tabs from a previous run
  await page.evaluate(async (testTabPrefix) => {
    const { chat } = await import('/js/core/app-state.js');
    const { newChatTab } = await import('/js/features/chat-state.js');
    const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
    chat.tabs = chat.tabs.filter(t => !t.name.startsWith(testTabPrefix));
    if (!chat.tabs.length) {
      const fallback = newChatTab('Chat 1');
      chat.tabs = [fallback];
      chat.activeTabId = fallback.id;
    } else if (!chat.tabs.some(t => t.id === chat.activeTabId)) {
      chat.activeTabId = chat.tabs[0].id;
    }
    renderChatTabs();
    renderChatMessages();
  }, TEST_TAB_PREFIX);

  // Create a fresh test tab
  await page.evaluate(async () => {
    const { chat } = await import('/js/core/app-state.js');
    const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
    const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
    const tab = newChatTab('Test Compaction');
    tab.name = '[TEST] Compaction';
    tab.visible_message_limit = 100;
    chat.tabs.push(tab);
    switchChatTab(tab.id);
    renderChatTabs();
    renderChatMessages();
  });

  // Switch to the test tab and clear it for a clean slate
  await page.evaluate(async (testTabPrefix) => {
    const { chat } = await import('/js/core/app-state.js');
    const { switchChatTab } = await import('/js/features/chat-state.js');
    const { renderChatMessages } = await import('/js/features/chat-render.js');
    const testTab = chat.tabs.find(t => t.name.startsWith(testTabPrefix));
    if (testTab) {
      testTab.messages = [];
      testTab.visible_message_limit = 100;
      switchChatTab(testTab.id);
    }
    renderChatMessages();
  }, TEST_TAB_PREFIX);
}

async function cleanupCompactionTestTab(page) {
  // Remove all test-created tabs
  await page.evaluate(async (testTabPrefix) => {
    const { chat } = await import('/js/core/app-state.js');
    const { newChatTab } = await import('/js/features/chat-state.js');
    const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
    chat.tabs = chat.tabs.filter(t => !t.name.startsWith(testTabPrefix));
    if (!chat.tabs.length) {
      const fallback = newChatTab('Chat 1');
      chat.tabs = [fallback];
      chat.activeTabId = fallback.id;
    } else if (!chat.tabs.some(t => t.id === chat.activeTabId)) {
      chat.activeTabId = chat.tabs[0].id;
    }
    renderChatTabs();
    renderChatMessages();
  }, TEST_TAB_PREFIX);
}

async function confirmCompact(page) {
  // Wait for compact confirmation modal to appear
  await expect(page.locator('.compact-confirm-overlay')).toBeVisible({ timeout: 5000 });
  // Wait for OK button to be enabled (summary may fail in CI, but OK still enables)
  await expect(page.locator('.compact-confirm-ok:not([disabled])')).toBeVisible({ timeout: 10000 });
  // Confirm the compact
  await page.locator('.compact-confirm-ok').click();
}

test.describe('context compaction', () => {
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

    await createCompactionTestTab(page);
    // Short-circuit the summarization fetch so compaction completes immediately
    // regardless of whether a llama server is running in CI.
    await page.route('**/api/chat', route => route.fulfill({ status: 503 }));
  });

  test.afterEach(async ({ page }) => {
    await cleanupCompactionTestTab(page);
  });

  test('compact button removes old messages and creates tombstone', async ({ page }) => {
    // Inject 20 synthetic messages to simulate a long conversation.
    // Set visible_message_limit high so pagination doesn't hide messages from DOM counts.
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      tab.visible_message_limit = 100;
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Question ${i + 1}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `Answer ${i + 1}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });

    // Verify we have messages before compaction
    const msgCountBefore = await page.locator('.chat-message:not(.chat-compact-marker)').count();
    expect(msgCountBefore).toBeGreaterThanOrEqual(20);

    // Trigger compaction via the compact button
    await page.locator('#btn-compact').click();
    await confirmCompact(page);

    // Wait for compaction to complete (tombstone appears)
    await page.waitForSelector('.chat-compact-marker[data-compact-state="final"]', { timeout: 10000 });

    // Verify the final tombstone was created, not just the temporary loading placeholder
    const finalMarker = page.locator('.chat-compact-marker[data-compact-state="final"]');
    await expect(finalMarker).toBeVisible();
    await expect(page.locator('.chat-compact-marker[data-compact-state="loading"]')).toHaveCount(0);
    const tombstoneText = await finalMarker.locator('.compact-marker-label').textContent();
    expect(tombstoneText).toMatch(/context (summarized|trimmed)/i);

    // Verify old messages were removed (only keepTail=10 remain)
    const msgCountAfter = await page.locator('.chat-message:not(.chat-compact-marker)').count();
    expect(msgCountAfter).toBeLessThan(msgCountBefore);
  });

  test('multiple compactions preserve old tombstones', async ({ page }) => {
    // Uses the shared test tab (cleared by beforeEach).
    // First round: inject messages and compact
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Round 1 Q${i}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `Round 1 A${i}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });
    await page.locator('#btn-compact').click();
    await confirmCompact(page);
    // Wait for compaction to complete (tombstone appears)
    await page.waitForSelector('.chat-compact-marker[data-compact-state="final"]', { timeout: 10000 });
    await expect(page.locator('.chat-compact-marker[data-compact-state="final"]')).toHaveCount(1);

    // Second round: inject more messages and compact again
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Round 2 Q${i}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `Round 2 A${i}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });
    await page.locator('#btn-compact').click();
    await confirmCompact(page);
    // Wait for second tombstone
    const tombstones = page.locator('.chat-compact-marker[data-compact-state="final"]');
    await expect(tombstones).toHaveCount(2, { timeout: 10000 });

    // Both tombstones should exist
    await expect(page.locator('.chat-compact-marker[data-compact-state="final"]')).toHaveCount(2);
  });

  test('auto-compact settings persist on tab switch', async ({ page }) => {
    // New tabs have auto_compact on by default
    const newTabAutoCompact = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return !!activeChatTab().auto_compact;
    });
    expect(newTabAutoCompact).toBe(true);

    // Disable auto-compact on the test tab
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      tab.auto_compact = false;
      tab.updated_at = Date.now();
    });

    // Create a new tab and switch to it
    await page.locator('#csp-new-btn').click();

    // New tab should have auto-compact on by default
    const newTabAutoCompact2 = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return !!activeChatTab().auto_compact;
    });
    expect(newTabAutoCompact2).toBe(true);

    // Switch back to the test tab — settings should still be off (per-tab persistence)
    await page.evaluate(async (testTabPrefix) => {
      const { chat } = await import('/js/core/app-state.js');
      const { switchChatTab } = await import('/js/features/chat-state.js');
      const testTab = chat.tabs.find(t => t.name.startsWith(testTabPrefix));
      if (testTab) switchChatTab(testTab.id);
    }, TEST_TAB_PREFIX);
    const firstTabAutoCompact = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return !!activeChatTab().auto_compact;
    });
    expect(firstTabAutoCompact).toBe(false);

    // Clean up the extra tab created by this test
    await page.evaluate(async (testTabPrefix) => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
      chat.tabs = chat.tabs.filter(t => t.name.startsWith(testTabPrefix) || t.name === 'Chat 1');
      if (!chat.tabs.length) {
        const fallback = newChatTab('Chat 1');
        chat.tabs = [fallback];
        chat.activeTabId = fallback.id;
      } else if (!chat.tabs.some(t => t.id === chat.activeTabId)) {
        chat.activeTabId = chat.tabs[0].id;
      }
      renderChatTabs();
      renderChatMessages();
    }, TEST_TAB_PREFIX);
  });
});

test.describe('compact confirmation modal', () => {
  const TEST_TAB_PREFIX = '[TEST-CC]';

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

    // Create or switch to an isolated test tab so we don't pollute user data.
    await page.evaluate(async (testTabPrefix) => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, switchChatTab } = await import('/js/features/chat-state.js');
      const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');

      chat.tabs = chat.tabs.filter(t => !t.name.startsWith(testTabPrefix));
      if (!chat.tabs.length) {
        const fallback = newChatTab('Chat 1');
        chat.tabs = [fallback];
        chat.activeTabId = fallback.id;
      } else if (!chat.tabs.some(t => t.id === chat.activeTabId)) {
        chat.activeTabId = chat.tabs[0].id;
      }

      const tab = newChatTab('Test');
      tab.name = `${testTabPrefix} Features`;
      tab.visible_message_limit = 100;
      chat.tabs.push(tab);
      switchChatTab(tab.id);
      renderChatTabs();
      renderChatMessages();
    }, TEST_TAB_PREFIX);

    // Inject messages so compact button is meaningful
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const { renderChatMessages } = await import('/js/features/chat-render.js');
      const tab = activeChatTab();
      tab.visible_message_limit = 100;
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Q${i}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `A${i}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });

    // Short-circuit summarization (no AI server in CI)
    await page.route('**/api/chat', route => route.fulfill({ status: 503 }));
  });

  test.afterEach(async ({ page }) => {
    await page.evaluate(async (p) => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
      chat.tabs = chat.tabs.filter(t => !t.name.startsWith(p));
      if (!chat.tabs.length) {
        const fb = newChatTab('Chat 1');
        chat.tabs = [fb];
        chat.activeTabId = fb.id;
      } else if (!chat.tabs.some(t => t.id === chat.activeTabId)) {
        chat.activeTabId = chat.tabs[0].id;
      }
      renderChatTabs();
      renderChatMessages();
    }, TEST_TAB_PREFIX);
  });

  test('opens confirmation modal with stats when compact is clicked', async ({ page }) => {
    await page.locator('#btn-compact').click();

    // Modal overlay appears
    await expect(page.locator('.compact-confirm-overlay')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('.compact-confirm-modal')).toBeVisible();

    // Title
    await expect(page.locator('#compact-confirm-title')).toBeVisible();
    await expect(page.locator('#compact-confirm-title')).toContainText('Compact Context');

    // Stats grid present
    await expect(page.locator('.compact-confirm-stats')).toBeVisible();
    await expect(page.locator('.compact-stat')).toHaveCount(
      await page.locator('.compact-stat').count(),
      { timeout: 3000 }
    );
  });

  test('OK button enabled even if summary fails (CI-safe)', async ({ page }) => {
    await page.locator('#btn-compact').click();

    // OK button becomes enabled after summary attempt (even on failure)
    const okBtn = page.locator('.compact-confirm-ok:not([disabled])');
    await expect(okBtn).toBeVisible({ timeout: 10000 });
    await expect(okBtn).toContainText('Compact Now');
  });

  test('Cancel and Close dismiss without compacting', async ({ page }) => {
    const msgCountBefore = await page.locator('.chat-message:not(.chat-compact-marker)').count();

    await page.locator('#btn-compact').click();
    await expect(page.locator('.compact-confirm-overlay')).toBeVisible({ timeout: 5000 });

    // Cancel
    await page.locator('.compact-confirm-cancel').click();
    // Overlay gone
    await expect(page.locator('.compact-confirm-overlay')).not.toBeVisible({ timeout: 5000 });

    // Messages unchanged
    const msgCountAfter = await page.locator('.chat-message:not(.chat-compact-marker)').count();
    expect(msgCountAfter).toBe(msgCountBefore);
  });
});

test.describe('auto_compact_summarize default', () => {
  test('new tabs have auto_compact_summarize true', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.waitForSelector('#view-monitor', { state: 'attached', timeout: 5000 });
    await page.waitForFunction(() => {
      const monitor = document.getElementById('view-monitor');
      return monitor && getComputedStyle(monitor).display !== 'none';
    }, { timeout: 5000 });
    await page.getByRole('button', { name: /chat/i }).click();

    const value = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      return tab?.auto_compact_summarize ?? null;
    });

    expect(value).toBe(true);
  });
});
