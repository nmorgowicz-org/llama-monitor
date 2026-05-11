// ── Quick Guide Tests ────────────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures, seedChatMessages } from './fixtures.js';

test.describe('Quick Guide', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
    await enableGuidedGenFeatures(page, { enabled_quick_guide: true });
  });

  test('toggle inline input', async ({ page }) => {
    const toggle = page.locator('#quick-guide-toggle');
    const container = page.locator('#quick-guide-container');

    // Initially collapsed
    await expect(container).not.toHaveClass(/quick-guide-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    // Toggle open
    await toggle.click();
    await expect(container).toHaveClass(/quick-guide-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');

    // Should show textarea
    await expect(page.locator('#quick-guide-input')).toBeVisible();

    // Toggle closed
    await toggle.click();
    await expect(container).not.toHaveClass(/quick-guide-expanded/);
  });

  test('submit instruction with Enter', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Describe the abandoned station platform.' },
      { role: 'assistant', content: 'Rain ghosted across the cracked tiles as the platform lights buzzed in uneven intervals.' },
    ]);
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Make the tone more mysterious');
    await page.locator('#quick-guide-input').press('Enter');

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);

    // Input should be cleared
    await expect(page.locator('#quick-guide-input')).toHaveValue('');

    await expect(page.locator('.chat-error')).toHaveCount(0);
    await expect.poll(async () => await page.locator('.chat-message-assistant').count()).toBeGreaterThan(1);
  });

  test('submit instruction with button', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Write a tense hallway confrontation.' },
      { role: 'assistant', content: 'The hallway hummed with bad fluorescent light while neither of them stepped aside.' },
    ]);
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Add more dialogue');
    await page.locator('#quick-guide-submit-btn').click();

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);
    await expect(page.locator('.chat-error')).toHaveCount(0);
  });

  test('last used instruction displayed', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Show, don\'t tell');
    await page.locator('#quick-guide-submit-btn').click();

    // Toggle again to see last used
    await page.locator('#quick-guide-toggle').click();
    await expect(page.locator('.quick-guide-last-used')).toContainText('Show, don\'t tell');
  });

  test('empty submit clears active guide', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Test instruction');
    await page.locator('#quick-guide-submit-btn').click();

    // Toggle again
    await page.locator('#quick-guide-toggle').click();

    // Submit empty input to clear the active guide
    await page.locator('#quick-guide-input').fill('');
    await page.locator('#quick-guide-submit-btn').click();

    // Toggle again to inspect cleared state
    await page.locator('#quick-guide-toggle').click();
    await expect(page.locator('#quick-guide-input')).toHaveValue('');
    await expect(page.locator('#quick-guide-status')).toContainText('Idle');
  });

  test('Escape closes without submit', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Test instruction');
    await page.keyboard.press('Escape');

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);
  });

  test('click outside closes', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    // Click chat input
    await page.locator('#chat-input').click();

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);
  });

});
