// ── Quick Guide Revise Last Tests ──────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures, seedChatMessages } from './fixtures.js';

test.describe('Quick Guide Revise Last', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
    await enableGuidedGenFeatures(page, { enabled_quick_guide: true });
  });

  test('Revise Last button exists in quick guide', async ({ page }) => {
    // Seed a message and set _quickGuideLastRun so the button is visible
    await seedChatMessages(page, [
      { role: 'user', content: 'Hello' },
      { role: 'assistant', content: 'Hi there!' },
    ]);
    await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        tab._quickGuideLastRun = {
          instruction: 'Make it more formal',
          targetIndex: 1,
          targetRole: 'assistant',
        };
      });
    });

    // Open the quick guide
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    // The restore button should be visible
    const restoreBtn = page.locator('#quick-guide-restore-btn');
    await expect(restoreBtn).toBeVisible();
    await expect(restoreBtn).not.toBeDisabled();
  });

  test('Revise Last button is hidden when no messages', async ({ page }) => {
    // No messages seeded, no _quickGuideLastRun
    const restoreBtn = page.locator('#quick-guide-restore-btn');

    // Button should be hidden with no messages and no last run
    await expect(restoreBtn).not.toBeVisible();
  });

  test('Revise Last button appears after a message with last run', async ({ page }) => {
    // Seed a message
    await seedChatMessages(page, [
      { role: 'user', content: 'Write a story' },
      { role: 'assistant', content: 'Once upon a time...' },
    ]);

    // Set _quickGuideLastRun to simulate a previous guided reply
    await page.evaluate(() => {
      return import('/js/features/chat-state.js').then(({ activeChatTab }) => {
        const tab = activeChatTab();
        tab._quickGuideLastRun = {
          instruction: 'Add more tension',
          targetIndex: 1,
          targetRole: 'assistant',
        };
      });
    });

    // Open the quick guide - updateQuickGuideUI will run and show the button
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    // Button should now be visible
    const restoreBtn = page.locator('#quick-guide-restore-btn');
    await expect(restoreBtn).toBeVisible();
  });
});
