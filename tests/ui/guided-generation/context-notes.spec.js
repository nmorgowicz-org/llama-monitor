// ── Context Notes Sidebar Tests ──────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures } from './fixtures.js';

test.describe('Context Notes Sidebar', () => {
  test.beforeEach(async ({ page, context }) => {
    // Set localStorage BEFORE page load so settingsState is initialized correctly
    await context.addInitScript(() => {
      const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
      settings.enabled_context_notes = true;
      settings.enabled_suggestions = true;
      settings.enabled_quick_guide = true;
      localStorage.setItem('llama_monitor_settings', JSON.stringify(settings));
    });

    await switchToChat(page);
    // Also update in-memory settingsState (defensive)
    await page.evaluate(async () => {
      const { settingsState } = await import('/js/core/app-state.js');
      settingsState.enabled_context_notes = true;
    });
  });

  test('toggle sidebar with button', async ({ page }) => {
    const toggle = page.locator('#context-sidebar-toggle');
    const sidebar = page.locator('#chat-sidebar');

    // Initially collapsed
    await expect(sidebar).not.toHaveClass(/sidebar-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    // Toggle open
    await toggle.click();
    await expect(sidebar).toHaveClass(/sidebar-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');

    // Toggle closed
    await toggle.click();
    await expect(sidebar).not.toHaveClass(/sidebar-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  });

test('predefined sections are rendered', async ({ page }) => {
    // Directly expand sidebar via JS
    await page.evaluate(async () => {
      const { settingsState } = await import('/js/core/app-state.js');
      settingsState.enabled_context_notes = true;
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
      const sidebar = document.getElementById('chat-sidebar');
      if (!sidebar?.classList.contains('sidebar-expanded')) {
        toggleContextSidebar();
      }
    });

    // Wait for toggle to report expanded via aria-expanded
    await expect(page.locator('#context-sidebar-toggle')).toHaveAttribute('aria-expanded', 'true', { timeout: 10000 });
    await page.waitForSelector('.sidebar-notes-list');

    // All 4 predefined sections should be present (note: "Plot/Scenario" is the actual name)
    for (const section of ['Character', 'Setting', 'Plot/Scenario', 'Tone']) {
      await expect(page.locator(`.sidebar-section-wrapper[data-section="${section}"]`)).toBeVisible();
    }
  });

test('add note to Character section', async ({ page }) => {
    // Directly expand sidebar via JS to avoid settingsState staleness issues
    await page.evaluate(async () => {
      const { settingsState } = await import('/js/core/app-state.js');
      settingsState.enabled_context_notes = true;
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
      const sidebar = document.getElementById('chat-sidebar');
      if (!sidebar?.classList.contains('sidebar-expanded')) {
        toggleContextSidebar();
      }
    });

    // Wait for toggle to report expanded via aria-expanded
    await expect(page.locator('#context-sidebar-toggle')).toHaveAttribute('aria-expanded', 'true', { timeout: 10000 });
    await page.waitForSelector('.sidebar-notes-list');

    // Click the add note button using JS dispatchEvent (pointer-events interception)
    await page.evaluate(() => {
      const btn = document.querySelector('.sidebar-add-note-btn[data-section="Character"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await page.waitForSelector('.sidebar-form-textarea[data-section="Character"]', { state: 'visible' });

    // Fill the note
    await page.locator('.sidebar-form-textarea[data-section="Character"]').fill('Alice: 28, noir detective');

    // Save the note using JS dispatchEvent
    await page.evaluate(() => {
      const btn = document.querySelector('[data-section-save="Character"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    // Note should appear in list
    await expect(page.locator('.sidebar-note-item')).toContainText('Alice: 28, noir detective');

    // Verify persisted in tab
    const persisted = await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        return tab.context_notes?.length;
      });
    });
    expect(persisted).toBeGreaterThan(0);
  });

  test('delete note from list', async ({ page }) => {
    // Ensure chat view is visible
    await expect(page.locator('#page-chat.active')).toBeVisible({ timeout: 5000 });

    // Seed note and open sidebar in one atomic evaluate so no async gap between
    // seeding and rendering (prevents bootstrap's initChatTabs resetting the tab).
    await page.evaluate(async () => {
      const { activeChatTab, addChatTab } = await import('/js/features/chat-state.js');
      const { settingsState } = await import('/js/core/app-state.js');
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');

      settingsState.enabled_context_notes = true;

      let tab = activeChatTab();
      if (!tab) {
        await addChatTab();
        tab = activeChatTab();
      }
      if (!tab) throw new Error('No active chat tab');
      tab.context_notes = [
        { section: 'Character', content: 'Test character note', created_at: Date.now() }
      ];

      // Open sidebar — this synchronously calls renderNotesList with the seeded note
      const sidebar = document.getElementById('chat-sidebar');
      if (sidebar?.classList.contains('sidebar-expanded')) {
        toggleContextSidebar(); // collapse first so next call expands + re-renders
      }
      toggleContextSidebar();
    });

    // Wait for toggle to report expanded via aria-expanded
    await expect(page.locator('#context-sidebar-toggle')).toHaveAttribute('aria-expanded', 'true', { timeout: 10000 });
    await page.waitForSelector('.sidebar-note-item', { timeout: 10000 });

    const beforeCount = await page.locator('.sidebar-note-item').count();
    // Use JavaScript dispatchEvent to bypass pointer-events interception from overlapping elements
    await page.evaluate(() => {
      const btn = document.querySelector('.sidebar-note-btn-delete');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    const afterCount = await page.locator('.sidebar-note-item').count();

    expect(afterCount).toBe(beforeCount - 1);
  });

  test('empty state shown when no notes', async ({ page }) => {
    // Directly expand sidebar via JS
    await page.evaluate(async () => {
      const { settingsState } = await import('/js/core/app-state.js');
      settingsState.enabled_context_notes = true;
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
      const sidebar = document.getElementById('chat-sidebar');
      if (!sidebar?.classList.contains('sidebar-expanded')) {
        toggleContextSidebar();
      }
    });

    await page.waitForSelector('.sidebar-notes-list');

    // Each predefined section shows placeholder text when empty
    await expect(page.locator('.sidebar-section-empty').first()).toBeVisible();
  });

  test('note composer form closes after save', async ({ page }) => {
    // Ensure chat view is visible
    await expect(page.locator('#page-chat')).toBeVisible({ timeout: 5000 });

    // Directly expand sidebar via JS to avoid settingsState staleness issues
    await page.evaluate(async () => {
      const { settingsState } = await import('/js/core/app-state.js');
      settingsState.enabled_context_notes = true;
      const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
      // Force expand if currently collapsed
      const sidebar = document.getElementById('chat-sidebar');
      if (!sidebar?.classList.contains('sidebar-expanded')) {
        toggleContextSidebar();
      }
    });

    // Wait for toggle to report expanded via aria-expanded
    await expect(page.locator('#context-sidebar-toggle')).toHaveAttribute('aria-expanded', 'true', { timeout: 10000 });
    await page.waitForSelector('.sidebar-notes-list');

    // Use JavaScript dispatchEvent to bypass pointer-events interception
    await page.evaluate(() => {
      const btn = document.querySelector('.sidebar-add-note-btn[data-section="Character"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    await page.waitForSelector('.sidebar-form-textarea[data-section="Character"]', { state: 'visible' });

    await page.locator('.sidebar-form-textarea[data-section="Character"]').fill('Test content');
    // Use JS dispatchEvent for save button too (same pointer-events issue)
    await page.evaluate(() => {
      const btn = document.querySelector('[data-section-save="Character"]');
      if (btn) btn.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    // Form should close after save
    await expect(page.locator('.sidebar-form-textarea[data-section="Character"]')).not.toBeVisible();
  });
});
