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

  test('add note with section and content', async ({ page }) => {
    // Open sidebar
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    // Add note
    await page.locator('#sidebar-section-input').fill('Characters');
    await page.locator('#sidebar-content-input').fill('Alice: 25, detective');
    await page.locator('#sidebar-add-btn').click();

    // Verify note appears
    await expect(page.locator('.sidebar-note-item')).toContainText('Alice: 25, detective');
    await expect(page.locator('.sidebar-note-section')).toContainText('CHARACTERS');

    // Verify persisted in tab
    const persisted = await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        return tab.context_notes?.length;
      });
    });
    expect(persisted).toBeGreaterThan(0);
  });

  test('add note defaults section to General', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    // Add note with empty section
    await page.locator('#sidebar-section-input').fill('');
    await page.locator('#sidebar-content-input').fill('General note');
    await page.locator('#sidebar-add-btn').click();

    // Should default to General
    await expect(page.locator('.sidebar-note-section')).toContainText('GENERAL');
  });

  test('delete note from list', async ({ page }) => {
    // Seed a note
    await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        tab.context_notes = [
          { section: 'Test', content: 'Test note', created_at: Date.now() }
        ];
      });
    });

    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-note-item');

    const beforeCount = await page.locator('.sidebar-note-item').count();
    await page.locator('.sidebar-note-delete').first().click();
    const afterCount = await page.locator('.sidebar-note-item').count();

    expect(afterCount).toBe(beforeCount - 1);
  });

  test('empty state shown when no notes', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    await expect(page.locator('.sidebar-empty-state')).toBeVisible();
    await expect(page.locator('.sidebar-empty-state')).toContainText('No context notes');
  });

  test('note form clears after submit', async ({ page }) => {
    await page.locator('#context-sidebar-toggle').click();
    await page.waitForSelector('.sidebar-notes-list');

    await page.locator('#sidebar-section-input').fill('Test');
    await page.locator('#sidebar-content-input').fill('Test content');
    await page.locator('#sidebar-add-btn').click();

    await expect(page.locator('#sidebar-section-input')).toHaveValue('');
    await expect(page.locator('#sidebar-content-input')).toHaveValue('');
  });
});
