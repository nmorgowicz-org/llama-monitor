import { test, expect } from '@playwright/test';

test.describe('chat UI shell', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    // Switch to monitor view so chat page is accessible
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('renders empty chat state with input bar', async ({ page }) => {
    await expect(page.locator('#chat-messages')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeEditable();
  });

  test('renders personalized empty state with suggested prompts', async ({ page }) => {
    await expect(page.locator('.chat-empty')).toBeVisible();
    await expect(page.locator('.chat-empty-title')).toBeVisible();
    await expect(page.locator('.chat-empty-prompts')).toBeVisible();
    const prompts = await page.locator('.chat-empty-prompt').count();
    expect(prompts).toBeGreaterThan(0);
  });

  test('creates new tab on + button click', async ({ page }) => {
    const tabCount = await page.locator('.chat-tab').count();
    await page.locator('.chat-tab-add').click();
    await expect(page.locator('.chat-tab')).toHaveCount(tabCount + 1);
  });

  test('switches between tabs', async ({ page }) => {
    await page.locator('.chat-tab-add').click();
    const tabs = await page.locator('.chat-tab').all();
    await tabs[0].click();
    await expect(tabs[0]).toHaveClass(/active/);
  });

  test('Ctrl+Shift+ArrowRight cycles to next tab', async ({ page }) => {
    await page.locator('.chat-tab-add').click();
    const tabs = page.locator('.chat-tab');
    // New tab becomes active automatically
    await expect(tabs.nth(1)).toHaveClass(/active/);
    await page.keyboard.press('Control+Shift+ArrowRight');
    // Cycles back to first tab
    await expect(tabs.first()).toHaveClass(/active/);
  });

  test('shows chat header controls', async ({ page }) => {
    await expect(page.locator('#btn-system-prompt')).toBeVisible();
    await expect(page.locator('#btn-model-params')).toBeVisible();
    await expect(page.locator('#chat-explicit-toggle-footer')).toBeVisible();
  });

  test('scroll-to-bottom button and badge are present', async ({ page }) => {
    await expect(page.locator('#chat-scroll-bottom')).toBeAttached();
    await expect(page.locator('#chat-scroll-badge')).toBeAttached();
  });
});

test.describe('system prompt panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('opens and closes via .open class (not display:none)', async ({ page }) => {
    // Panel starts without .open — not visible
    await expect(page.locator('#chat-system-panel')).not.toHaveClass(/open/);
    await page.locator('#btn-system-prompt').click();
    // CSS transition: wait for panel to be visible
    await expect(page.locator('#chat-system-panel')).toHaveClass(/open/);
    await expect(page.locator('#chat-system-panel')).toBeVisible();
    await page.locator('#btn-system-prompt').click();
    await expect(page.locator('#chat-system-panel')).not.toHaveClass(/open/);
  });

  test('allows editing system prompt', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.locator('#chat-system-input').fill('You are a test assistant.');
    await expect(page.locator('#chat-system-input')).toHaveValue('You are a test assistant.');
    // Indicator should appear when prompt is set
    await expect(page.locator('#system-prompt-indicator')).toBeVisible();
  });

  test('shows template dropdown', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await expect(page.locator('#chat-template-select')).toBeVisible();
    await expect(page.locator('.chat-template-mgmt-btn')).toBeVisible();
  });
});

test.describe('explicit mode toggle', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    // Ensure clean state: disable explicit mode if it was left on
    await page.evaluate(() => {
      const tab = activeChatTab();
      if (tab && tab.explicit_mode) toggleExplicitMode();
    });
  });

  test('toggles explicit mode state', async ({ page }) => {
    // Initially not active
    await expect(page.locator('#chat-explicit-toggle-footer')).not.toHaveClass(/active/);
    // Click to enable
    await page.locator('#chat-explicit-toggle-footer').click();
    await expect(page.locator('#chat-explicit-toggle-footer')).toHaveClass(/active/);
    // Click to disable
    await page.locator('#chat-explicit-toggle-footer').click();
    await expect(page.locator('#chat-explicit-toggle-footer')).not.toHaveClass(/active/);
  });

  test('toggle in settings panel mirrors footer toggle', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    // Enable via settings panel toggle
    await page.locator('#chat-explicit-toggle-settings').click();
    await expect(page.locator('#chat-explicit-toggle-settings')).toHaveClass(/active/);
    // Footer should also be active
    await expect(page.locator('#chat-explicit-toggle-footer')).toHaveClass(/active/);
  });
});

test.describe('template manager', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    await page.locator('#btn-system-prompt').click();
  });

  test('opens modal on manage button click', async ({ page }) => {
    await expect(page.locator('#template-manager-modal')).not.toHaveClass(/active/);
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('#template-manager-modal')).toHaveClass(/active/);
  });

  test('lists default templates', async ({ page }) => {
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('#template-list')).toBeVisible();
    // Should have at least the default templates
    const items = await page.locator('.template-list-item').count();
    expect(items).toBeGreaterThan(0);
  });

  test('explicit policy section is present', async ({ page }) => {
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('.explicit-policy-section')).toBeVisible();
    await expect(page.getByText('Explicit Mode Policy')).toBeVisible();
    // Expand the collapsed details element to verify textarea exists
    await page.getByText('Explicit Mode Policy').click();
    await expect(page.locator('#explicit-policy-input')).toBeVisible();
  });
});

test.describe('model params panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
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

  test('dirty indicator activates on non-default temperature', async ({ page }) => {
    await page.locator('#btn-model-params').click();
    // Default temperature is 0.7 — button should not have dirty indicator
    await expect(page.locator('#btn-model-params')).not.toHaveClass(/has-active-params/);
    // Set a non-default value
    await page.locator('#param-temperature').fill('0.4');
    await page.locator('#param-temperature').dispatchEvent('input');
    await expect(page.locator('#btn-model-params')).toHaveClass(/has-active-params/);
  });
});

test.describe('token count display', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });
});

test.describe('chat history pagination', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('load more button appears when messages exceed visible limit', async ({ page }) => {
    await page.evaluate(() => {
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
    await page.evaluate(() => {
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
    await page.evaluate(() => {
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

test.describe('app update UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
  });

  test('update pill is present but hidden by default', async ({ page }) => {
    await expect(page.locator('#update-pill')).toBeAttached();
    await expect(page.locator('#update-pill')).not.toBeVisible();
  });

  test('app version is displayed in sidebar', async ({ page }) => {
    await expect(page.locator('#app-version')).toBeAttached();
    const version = await page.locator('#app-version').textContent();
    // Should be non-empty (e.g. "v0.10.2")
    expect(version?.trim().length).toBeGreaterThan(0);
  });

  test('release notes panel is present but off-screen', async ({ page }) => {
    await expect(page.locator('#release-notes-panel')).toBeAttached();
    await expect(page.locator('#release-notes-panel')).not.toHaveClass(/open/);
  });

  test('showUpdatePill makes pill visible', async ({ page }) => {
    await page.evaluate(() => {
      // Clear any dismissal state
      localStorage.removeItem('update-dismissed');
      showUpdatePill({ tag_name: 'v99.0.0', html_url: '#', body: 'Test release', assets: [] });
    });
    await expect(page.locator('#update-pill')).toBeVisible();
    await expect(page.locator('#update-pill-text')).toContainText('v99.0.0');
  });

  test('opening release notes panel shows version diff', async ({ page }) => {
    await page.evaluate(() => {
      localStorage.removeItem('update-dismissed');
      showUpdatePill({ tag_name: 'v99.0.0', html_url: '#', body: '## What is new\nGreat things.', assets: [] });
    });
    await page.locator('#update-pill').click();
    await expect(page.locator('#release-notes-panel')).toHaveClass(/open/);
    await expect(page.locator('#release-notes-title')).toContainText('v99.0.0');
    await expect(page.locator('#release-notes-version-from')).toContainText('from v');
  });

  test('dismiss hides pill and closes panel', async ({ page }) => {
    await page.evaluate(() => {
      localStorage.removeItem('update-dismissed');
      showUpdatePill({ tag_name: 'v99.0.0', html_url: '#', body: '', assets: [] });
    });
    await page.locator('#update-pill').click();
    await expect(page.locator('#release-notes-panel')).toHaveClass(/open/);
    await page.locator('button[onclick="dismissUpdate()"]').click();
    await expect(page.locator('#update-pill')).not.toBeVisible();
    await expect(page.locator('#release-notes-panel')).not.toHaveClass(/open/);
  });
});

test.describe('context compaction', () => {
  // Isolate test data from user's real chat tabs.
  // All compaction tests use a dedicated tab prefixed with "[TEST]" so it can
  // be identified and cleaned up. The beforeEach creates the test tab; afterEach
  // removes it, leaving the user's chat history untouched.
  const TEST_TAB_PREFIX = '[TEST]';

  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();

    // Clean up any leftover test tabs from a previous run
    await page.evaluate((testTabPrefix) => {
      chatTabs = chatTabs.filter(t => !t.name.startsWith(testTabPrefix));
      if (!chatTabs.length) chatTabs = [newChatTab('Chat 1')];
    }, TEST_TAB_PREFIX);

    // Create a fresh test tab
    await page.evaluate(() => {
      const tab = newChatTab('Test Compaction');
      tab.name = '[TEST] Compaction';
      tab.visible_message_limit = 100;
      chatTabs.push(tab);
      switchChatTab(tab.id);
      renderChatMessages();
    });

    // Switch to the test tab and clear it for a clean slate
    await page.evaluate((testTabPrefix) => {
      const testTab = chatTabs.find(t => t.name.startsWith(testTabPrefix));
      if (testTab) {
        testTab.messages = [];
        testTab.visible_message_limit = 100;
        switchChatTab(testTab.id);
      }
      renderChatMessages();
    }, TEST_TAB_PREFIX);
    // Short-circuit the summarization fetch so compaction completes immediately
    // regardless of whether a llama server is running in CI.
    await page.route('**/api/chat', route => route.fulfill({ status: 503 }));
  });

  test.afterEach(async ({ page }) => {
    // Remove all test-created tabs
    await page.evaluate((testTabPrefix) => {
      chatTabs = chatTabs.filter(t => !t.name.startsWith(testTabPrefix));
      if (!chatTabs.length) chatTabs = [newChatTab('Chat 1')];
      renderChatTabs();
      renderChatMessages();
    }, TEST_TAB_PREFIX);
  });

  test('compact button removes old messages and creates tombstone', async ({ page }) => {
    // Inject 20 synthetic messages to simulate a long conversation.
    // Set visible_message_limit high so pagination doesn't hide messages from DOM counts.
    await page.evaluate(() => {
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

    // Trigger compaction directly and assert on the rendered result.
    // This avoids racing the header-button click path while still validating the UI outcome.
    await page.evaluate(async () => {
      const tab = activeChatTab();
      await compactChatTab(tab);
    });

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
    await page.evaluate(() => {
      const tab = activeChatTab();
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Round 1 Q${i}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `Round 1 A${i}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });
    await page.evaluate(async () => {
      const tab = activeChatTab();
      await compactChatTab(tab);
    });
    await expect(page.locator('.chat-compact-marker[data-compact-state="final"]')).toHaveCount(1);

    // Second round: inject more messages and compact again
    await page.evaluate(() => {
      const tab = activeChatTab();
      for (let i = 0; i < 20; i++) {
        tab.messages.push({ role: 'user', content: `Round 2 Q${i}`, timestamp_ms: Date.now() });
        tab.messages.push({ role: 'assistant', content: `Round 2 A${i}`, timestamp_ms: Date.now() });
      }
      renderChatMessages();
    });
    await page.evaluate(async () => {
      const tab = activeChatTab();
      await compactChatTab(tab);
    });

    // Both tombstones should exist
    await expect(page.locator('.chat-compact-marker[data-compact-state="final"]')).toHaveCount(2);
  });

  test('auto-compact settings persist on tab switch', async ({ page }) => {
    // New tabs have auto_compact on by default
    const newTabAutoCompact = await page.evaluate(() => !!activeChatTab().auto_compact);
    expect(newTabAutoCompact).toBe(true);

    // Disable auto-compact on the test tab
    await page.evaluate(() => {
      const tab = activeChatTab();
      tab.auto_compact = false;
      tab.updated_at = Date.now();
    });

    // Create a new tab and switch to it
    await page.locator('.chat-tab-add').click();

    // New tab should have auto-compact on by default
    const newTabAutoCompact2 = await page.evaluate(() => !!activeChatTab().auto_compact);
    expect(newTabAutoCompact2).toBe(true);

    // Switch back to the test tab — settings should still be off (per-tab persistence)
    await page.evaluate((testTabPrefix) => {
      const testTab = chatTabs.find(t => t.name.startsWith(testTabPrefix));
      if (testTab) switchChatTab(testTab.id);
    }, TEST_TAB_PREFIX);
    const firstTabAutoCompact = await page.evaluate(() => !!activeChatTab().auto_compact);
    expect(firstTabAutoCompact).toBe(false);

    // Clean up the extra tab created by this test
    await page.evaluate((testTabPrefix) => {
      chatTabs = chatTabs.filter(t => t.name.startsWith(testTabPrefix) || t.name === 'Chat 1');
      if (!chatTabs.length) chatTabs = [newChatTab('Chat 1')];
    }, TEST_TAB_PREFIX);
  });
});
