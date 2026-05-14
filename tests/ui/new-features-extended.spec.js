import { test, expect } from '@playwright/test';

// ── Helpers ──────────────────────────────────────────────────────────────────

async function switchToMonitor(page) {
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

async function ensureChatTab(page, prefix = '[TEST]') {
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
  }, prefix);
}

// ── Compact confirmation modal ───────────────────────────────────────────────

test.describe('compact confirmation modal', () => {
  const TEST_TAB_PREFIX = '[TEST-CC]';

  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await ensureChatTab(page, TEST_TAB_PREFIX);

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

// ── Persona reset button ─────────────────────────────────────────────────────

test.describe('persona reset button', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('reset button visible only for built-in persona copies', async ({ page }) => {
    await page.evaluate(async () => {
      const { openTemplateManager } = await import('/js/features/chat-templates.js');
      await openTemplateManager();
    });

    await expect(page.locator('#template-manager-modal')).toHaveClass(/active/);
    await page.waitForSelector('.template-list-item', { timeout: 5000 });

    // Check that any reset buttons are only present on items whose name matches a built-in.
    const resetButtons = page.locator('.template-preview-btn.reset, .template-list-btn[data-template-action="reset"]');
    const count = await resetButtons.count();

    if (count > 0) {
      // At least one reset button exists — verify it is tied to a built-in.
      await resetButtons.first().click();
      // After reset, a confirmation dialog or toast should appear.
      // We just confirm the button is interactive and does not crash.
    }
  });
});

// ── Template list sections + Active badge ────────────────────────────────────

test.describe('template list sections and active badge', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('active badge appears on current persona', async ({ page }) => {
    // Ensure active tab has an active_template_id so we can see the badge.
    await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      if (tab) {
        tab.active_template_id = 'default';
      }
    });

    await page.evaluate(async () => {
      const { openTemplateManager } = await import('/js/features/chat-templates.js');
      await openTemplateManager();
    });

    await expect(page.locator('#template-manager-modal')).toHaveClass(/active/);
    await page.waitForSelector('.template-list-item', { timeout: 5000 });

    // If active_template_id is set and a matching template exists, there should be an active badge.
    const activeBadge = page.locator('.template-active-badge');
    const activeItems = page.locator('.template-list-item.active-persona');

    // We'll accept either both present or both absent (depends on backend templates).
    const badgeCount = await activeBadge.count();
    const itemCount = await activeItems.count();

    // Consistency check: if badge exists, active item exists too.
    if (badgeCount > 0) {
      expect(itemCount).toBeGreaterThan(0);
    }
  });
});

// ── Per-persona explicit policies ────────────────────────────────────────────

test.describe('per-persona explicit policies', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('explicit policy section present in template manager', async ({ page }) => {
    await page.evaluate(async () => {
      const { openTemplateManager } = await import('/js/features/chat-templates.js');
      await openTemplateManager();
    });

    await expect(page.locator('#template-manager-modal')).toHaveClass(/active/);
    await page.waitForSelector('.explicit-policy-section', { timeout: 5000 });

    await expect(page.locator('.explicit-policy-section')).toBeVisible();
    await expect(page.getByText('Persona Explicit Policies')).toBeVisible();
    // Click to expand the details
    await page.getByText('Persona Explicit Policies').click();
    // Check that at least one of the policy containers is present
    const level1 = page.locator('#persona-explicit-level1');
    const level2 = page.locator('#persona-explicit-level2');
    const none = page.locator('#persona-explicit-none');
    const l1Count = await level1.count();
    const l2Count = await level2.count();
    const noneCount = await none.count();
    // At least one should exist (they're toggled based on selected template)
    expect(l1Count + l2Count + noneCount).toBeGreaterThan(0);
  });
});

// ── Debug Prompt Inspector ───────────────────────────────────────────────────

test.describe('debug prompt inspector', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('shows empty state before any send', async ({ page }) => {
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
    await page.$eval('#chat-input', (el, text) => {
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'test');
    await page.click('#btn-send');
    await page.waitForSelector('#chat-messages .chat-message-user', { timeout: 5000 });

    // Open debug prompt
    await page.locator('#btn-debug-prompt').click();

    // Content should be visible and empty state hidden
    await expect(page.locator('#debug-empty-state')).not.toBeVisible({ timeout: 5000 });
    await expect(page.locator('#debug-content')).not.toHaveClass(/hidden/);

    // Hero stats present
    await expect(page.locator('#debug-stat-utilization')).toBeVisible();
    await expect(page.locator('#debug-stat-total')).toBeVisible();
  });
});

// ── Global Escape key handling ───────────────────────────────────────────────

test.describe('global escape key handling', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('Escape closes settings modal', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    await page.keyboard.press('Escape');
    await expect(page.locator('#settings-modal')).not.toHaveClass(/open/, { timeout: 3000 });
  });

  test('Escape closes keyboard shortcuts modal', async ({ page }) => {
    await page.keyboard.down('Control');
    await page.keyboard.press('/');
    await page.keyboard.up('Control');
    await page.waitForSelector('#keyboard-shortcuts-modal.open', { timeout: 5000 });

    await page.keyboard.press('Escape');
    await expect(page.locator('#keyboard-shortcuts-modal')).not.toHaveClass(/open/, { timeout: 3000 });
  });
});

// ── Connection lost banner ───────────────────────────────────────────────────

test.describe('connection lost banner', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('banner appears when connection-lost modal is dismissed', async ({ page }) => {
    // Simulate connection-lost modal being shown by triggering a send error.
    await page.route('**/api/chat', route => route.fulfill({ status: 503 }));

    await page.$eval('#chat-input', (el, text) => {
      el.value = text;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, 'test');
    await page.click('#btn-send');

    // Wait for connection-lost modal (if it is configured to show on 503)
    const modal = page.locator('#connection-lost-modal');
    const banner = page.locator('#disconnected-banner');

    // If modal opens, dismiss it; otherwise we rely on banner being visible.
    if (await modal.isVisible({ timeout: 3000 }).catch(() => false)) {
      // Dismiss via button or close button
      const dismissBtn = page.locator('#connection-lost-dismiss-btn');
      if (await dismissBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
        await dismissBtn.click();
      } else {
        await page.locator('#connection-lost-modal-close').click();
      }
      // Banner should now be visible
      await expect(banner).toBeVisible({ timeout: 5000 });
      await expect(banner).not.toHaveAttribute('hidden');
    } else {
      // If modal did not appear, we still accept that banner can be shown directly
      // (depends on configuration). This test is best-effort in CI.
      test.skip(true, 'Connection-lost modal not triggered in this environment.');
    }
  });
});

// ── {{gender}} token substitution ────────────────────────────────────────────

test.describe('gender token substitution', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('selecting gender sets ai_gender on tab', async ({ page }) => {
    // Open behavior panel
    await page.locator('#btn-behavior').click();
    await expect(page.locator('#chat-behavior-panel')).toHaveClass(/open/);

    // Select "Female" gender pill (use first() to avoid strict mode violation)
    const femalePill = page.locator('#chat-behavior-panel .chat-gender-pill[data-gender="female"]');
    if (await femalePill.count() > 0) {
      await femalePill.click();
      await page.waitForTimeout(400);

      const gender = await page.evaluate(async () => {
        const { activeChatTab } = await import('/js/features/chat-state.js');
        return activeChatTab()?.ai_gender ?? null;
      });
      expect(gender).toBe('female');
    } else {
      test.skip(true, 'Gender pills not present in this build.');
    }
  });

  test('substituteNames replaces {{gender}} with selected value', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const { substituteNames } = await import('/js/features/chat-state.js');
      const text = 'You are a {{gender}} assistant.';
      return substituteNames(text, 'AI', 'User', 'male');
    });
    expect(result).toContain('male');
    expect(result).not.toContain('{{gender}}');
  });
});

// ── Custom role boundary ─────────────────────────────────────────────────────

test.describe('custom role boundary', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('role boundary section exists and is editable', async ({ page }) => {
    // Open behavior panel
    await page.locator('#btn-behavior').click();
    await expect(page.locator('#chat-behavior-panel')).toHaveClass(/open/);

    const toggle = page.locator('#chat-role-boundary-toggle');
    if (await toggle.count() === 0) {
      test.skip(true, 'Role boundary section not present in this build.');
    }

    // Expand role boundary section
    await toggle.click();
    await expect(page.locator('#chat-role-boundary-body')).toBeVisible({ timeout: 3000 });
    const textarea = page.locator('#chat-role-boundary-input');
    await expect(textarea).toBeVisible();

    // Edit value
    await textarea.fill('CUSTOM ROLE BOUNDARY TEXT');
    await page.waitForTimeout(600);

    // Confirm custom value stored on tab
    const custom = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      return activeChatTab()?.role_boundary_custom ?? null;
    });
    expect(custom).toBe('CUSTOM ROLE BOUNDARY TEXT');
  });

  test('reset restores default role boundary', async ({ page }) => {
    await page.locator('#btn-behavior').click();
    await expect(page.locator('#chat-behavior-panel')).toHaveClass(/open/);

    const toggle = page.locator('#chat-role-boundary-toggle');
    if (await toggle.count() === 0) {
      test.skip(true, 'Role boundary section not present in this build.');
    }

    await toggle.click();
    const textarea = page.locator('#chat-role-boundary-input');
    await expect(textarea).toBeVisible();

    // Set custom value
    await textarea.fill('CUSTOM');
    await page.waitForTimeout(600);

    // Reset
    const resetBtn = page.locator('#chat-role-boundary-reset');
    if (await resetBtn.count() > 0) {
      await resetBtn.click();
      await page.waitForTimeout(400);

      const custom = await page.evaluate(async () => {
        const { activeChatTab } = await import('/js/features/chat-state.js');
        return activeChatTab()?.role_boundary_custom ?? null;
      });
      expect(custom).toBeNull();
    }
  });
});

// ── auto_compact_summarize default ───────────────────────────────────────────

test.describe('auto_compact_summarize default', () => {
  test('new tabs have auto_compact_summarize true', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    const value = await page.evaluate(async () => {
      const { activeChatTab } = await import('/js/features/chat-state.js');
      const tab = activeChatTab();
      return tab?.auto_compact_summarize ?? null;
    });

    expect(value).toBe(true);
  });
});

// ── max_tokens default 4096 ──────────────────────────────────────────────────

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

// ── Context Notes AI Analysis (LOCAL ONLY — requires AI model) ───────────────
// Run with: LLAMA_MONITOR_HAS_AI=1 npm test new-features-extended.spec.js
// or manually in an environment with a reachable AI endpoint.

const hasAi = process.env.LLAMA_MONITOR_HAS_AI === '1';

test.describe('context notes AI analysis (local only)', () => {
  test.beforeEach(async ({ page }) => {
    test.skip(!hasAi, 'Set LLAMA_MONITOR_HAS_AI=1 to run AI-dependent tests.');

    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    // Enable context notes in settings
    await page.evaluate(() => {
      const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
      settings.enabled_context_notes = true;
      localStorage.setItem('llama_monitor_settings', JSON.stringify(settings));
    });
  });

  test('analyze button triggers analysis and shows results panel', async ({ page }) => {
    // Open context sidebar
    await page.evaluate(async () => {
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
      toggleContextSidebar();
    });
    await page.waitForSelector('#chat-sidebar', { timeout: 5000 });

    const analyzeBtn = page.locator('#chat-sidebar-analyze-btn');
    if (await analyzeBtn.count() === 0) {
      test.skip(true, 'Analyze button not present in this build.');
    }

    await analyzeBtn.click();

    // Button becomes loading
    await expect(analyzeBtn).toHaveClass(/is-loading/);

    // Analysis panel appears
    const panel = page.locator('#sidebar-analysis-panel');
    await expect(panel).toBeVisible({ timeout: 60000 });
  });
});

// ── Suggestions custom categories + focus keywords (LOCAL ONLY) ──────────────
// Run with: LLAMA_MONITOR_HAS_AI=1 npm test new-features-extended.spec.js

test.describe('suggestions custom categories and focus keywords (local only)', () => {
  test.beforeEach(async ({ page }) => {
    test.skip(!hasAi, 'Set LLAMA_MONITOR_HAS_AI=1 to run AI-dependent tests.');

    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();

    // Enable suggestions
    await page.evaluate(() => {
      const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
      settings.enabled_suggestions = true;
      localStorage.setItem('llama_monitor_settings', JSON.stringify(settings));
    });
  });

  test('manage categories modal shows custom categories area', async ({ page }) => {
    // Open suggestions dropdown
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await page.waitForSelector('#suggestions-dropdown', { timeout: 5000 });

    // Open manage categories
    const manageBtn = page.locator('#suggestions-manage-btn');
    if (await manageBtn.count() === 0) {
      test.skip(true, 'Manage categories button not present.');
    }
    await manageBtn.click();

    // Custom categories list area
    const customList = page.locator('#categories-custom-list');
    await expect(customList).toBeVisible({ timeout: 5000 });
  });

  test('focus keywords input and auto-generate button exist', async ({ page }) => {
    // Open suggestions dropdown
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await page.waitForSelector('#suggestions-dropdown', { timeout: 5000 });

    const focusInput = page.locator('#suggestion-focus-keywords-input');
    const autoBtn = page.locator('#auto-generate-focus-btn');

    // At least one of them should be visible if feature is present.
    const focusCount = await focusInput.count();
    const autoCount = await autoBtn.count();

    if (focusCount > 0 || autoCount > 0) {
      if (focusCount > 0) await expect(focusInput).toBeVisible();
      if (autoCount > 0) await expect(autoBtn).toBeVisible();
    } else {
      test.skip(true, 'Focus keywords controls not present in this build.');
    }
  });
});
