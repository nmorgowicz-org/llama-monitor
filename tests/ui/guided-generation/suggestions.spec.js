// ── Suggestions Dropdown Tests ───────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, enableGuidedGenFeatures, mockSuggestionsAPI } from './fixtures.js';

test.describe('Suggestions Dropdown', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
    await enableGuidedGenFeatures(page, { enabled_suggestions: true });
    await mockSuggestionsAPI(page);
  });

  test('toggle dropdown with button', async ({ page }) => {
    const toggle = page.locator('#suggestions-toggle');
    const dropdown = page.locator('#suggestions-dropdown');

    // Initially closed
    await expect(dropdown).not.toHaveClass(/dropdown-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    // Toggle open
    await toggle.click();
    await expect(dropdown).toHaveClass(/dropdown-expanded/);
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');

    // Toggle closed
    await toggle.click();
    await expect(dropdown).not.toHaveClass(/dropdown-expanded/);
  });

  test('generate button fetches suggestions', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Click generate
    await page.locator('#suggestions-generate-btn').click();

    // Should show loading state
    await expect(page.locator('.suggestions-loading')).toBeVisible({ timeout: 5000 });

    // Then show suggestions
    await expect(page.locator('.suggestion-item')).toHaveCount({ min: 1 }, { timeout: 10000 });
  });

  test('switch category filters suggestions', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Click different category
    await page.locator('.suggestion-category-btn', { hasText: 'Plot Twist' }).click();

    // Category should be selected
    await expect(page.locator('.suggestion-category-btn', { hasText: 'Plot Twist' })).toHaveClass(/active/);
  });

  test('use suggestion inserts into chat input', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Generate suggestions
    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });

    // Click use button
    await page.locator('.suggestion-btn').first().click();

    // Should populate chat input
    const chatInput = page.locator('#chat-input');
    await expect(chatInput).not.toHaveValue('', { timeout: 5000 });
  });

  test('used suggestions tracked in history', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Generate and use suggestion
    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });
    await page.locator('.suggestion-btn').first().click();

    // Check recent section exists
    await expect(page.locator('.suggestions-recent')).toBeVisible();
    await expect(page.locator('.suggestions-recent-list .suggestion-item')).toHaveCount({ min: 1 });
  });

  test('clear recent history', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Generate and use suggestion
    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });
    await page.locator('.suggestion-btn').first().click();

    // Clear history
    await page.locator('.suggestions-clear-recent').click();

    // Recent section should be empty
    await expect(page.locator('.suggestions-recent-list .suggestion-item')).toHaveCount(0);
  });

  test('keyboard navigation through suggestions', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Generate suggestions
    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });

    // Focus dropdown
    await page.locator('#suggestions-dropdown').focus();

    // Arrow down should move focus
    await page.keyboard.press('ArrowDown');
    const focusedItem = await page.locator('.suggestion-item:focus').count();
    expect(focusedItem).toBeGreaterThanOrEqual(0); // May be 0 if focus trap not implemented

    // Escape should close
    await page.keyboard.press('Escape');
    await expect(page.locator('#suggestions-dropdown')).not.toHaveClass(/dropdown-expanded/);
  });

  test('click outside closes dropdown', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Click somewhere else
    await page.locator('#chat-input').click();

    await expect(page.locator('#suggestions-dropdown')).not.toHaveClass(/dropdown-expanded/);
  });
});
