// ── Context Notes Sidebar Tests ──────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures } from './fixtures.js';

test.describe('Context Notes Sidebar', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
    await enableGuidedGenFeatures(page, { enabled_context_notes: true });
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
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    // All 4 predefined sections should be present (note: "Plot/Scenario" is the actual name)
    for (const section of ['Character', 'Setting', 'Plot/Scenario', 'Tone']) {
      await expect(page.locator(`.sidebar-section-wrapper[data-section="${section}"]`)).toBeVisible();
    }
  });

  test('add note to Character section', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    // Click the add note button for Character section
    await page.locator('.sidebar-add-note-btn[data-section="Character"]').click();
    await page.waitForSelector('.sidebar-form-textarea[data-section="Character"]', { state: 'visible' });

    // Fill the note
    await page.locator('.sidebar-form-textarea[data-section="Character"]').fill('Alice: 28, noir detective');

    // Save the note
    await page.locator('[data-section-save="Character"]').click();

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
    // Seed a note directly into tab state
    await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        tab.context_notes = [
          { section: 'Character', content: 'Test character note', created_at: Date.now() }
        ];
      });
    });

    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-note-item');

    const beforeCount = await page.locator('.sidebar-note-item').count();
    await page.locator('.sidebar-note-btn-delete').first().click();
    const afterCount = await page.locator('.sidebar-note-item').count();

    expect(afterCount).toBe(beforeCount - 1);
  });

  test('empty state shown when no notes', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    // Each predefined section shows placeholder text when empty
    await expect(page.locator('.sidebar-section-empty').first()).toBeVisible();
  });

  test('note composer form closes after save', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    await page.locator('.sidebar-add-note-btn[data-section="Character"]').click();
    await page.waitForSelector('.sidebar-form-textarea[data-section="Character"]', { state: 'visible' });

    await page.locator('.sidebar-form-textarea[data-section="Character"]').fill('Test content');
    await page.locator('[data-section-save="Character"]').click();

    // Form should close after save
    await expect(page.locator('.sidebar-form-textarea[data-section="Character"]')).not.toBeVisible();
  });
});
