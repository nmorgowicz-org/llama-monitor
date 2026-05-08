// ── Quick Guide Tests ────────────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures } from './fixtures.js';

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
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Make the tone more mysterious');
    await page.locator('#quick-guide-input').press('Enter');

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);

    // Input should be cleared
    await expect(page.locator('#quick-guide-input')).toHaveValue('');
  });

  test('submit instruction with button', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Add more dialogue');
    await page.locator('#quick-guide-submit').click();

    // Should collapse
    await expect(page.locator('#quick-guide-container')).not.toHaveClass(/quick-guide-expanded/);
  });

  test('last used instruction displayed', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Show, don\'t tell');
    await page.locator('#quick-guide-submit').click();

    // Toggle again to see last used
    await page.locator('#quick-guide-toggle').click();
    await expect(page.locator('.quick-guide-last-used')).toContainText('Show, don\'t tell');
  });

  test('clear button resets state', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    await page.locator('#quick-guide-input').fill('Test instruction');
    await page.locator('#quick-guide-submit').click();

    // Toggle again
    await page.locator('#quick-guide-toggle').click();

    // Click clear
    await page.locator('#quick-guide-clear').click();

    // Should be cleared
    await expect(page.locator('#quick-guide-input')).toHaveValue('');
    await expect(page.locator('.quick-guide-last-used')).not.toBeVisible();
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

  test('empty instruction does not submit', async ({ page }) => {
    await page.locator('#quick-guide-toggle').click();
    await page.waitForSelector('#quick-guide-input', { state: 'visible' });

    // Leave empty and try to submit
    await page.locator('#quick-guide-submit').click();

    // Should remain open
    await expect(page.locator('#quick-guide-container')).toHaveClass(/quick-guide-expanded/);
  });
});
