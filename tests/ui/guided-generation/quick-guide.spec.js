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

  test('submit instruction with Enter collapses and clears input', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Make the tone more mysterious');
    await page.locator('#quick-guide-input').press('Enter');

    // Guide should collapse synchronously
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);

    // Input should be cleared
    await expect(page.locator('#quick-guide-input')).toHaveValue('');
  });

  test('submit instruction with button collapses guide', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Add more dialogue');
    await page.locator('#quick-guide-submit-btn').click();

    // Guide should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);
    // Input should be cleared
    await expect(page.locator('#quick-guide-input')).toHaveValue('');
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

  test('empty submit clears draft', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    // Fill then clear the draft
    await page.locator('#quick-guide-input').fill('Test instruction');
    await page.locator('#quick-guide-input').fill('');

    // Input should be empty
    await expect(page.locator('#quick-guide-input')).toHaveValue('');

    // Status should show Idle when draft is empty and guide is open
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
