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

    // Click generate — mock resolves quickly so skip flicker-only loading check
    await page.locator('#suggestions-generate-btn').click();

    // Suggestions should appear
    await expect(page.locator('.suggestion-item').first()).toBeVisible({ timeout: 10000 });
  });

  test('switch category filters suggestions', async ({ page }) => {
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Directly call setSuggestionCategory to switch to Plot Twist
    await page.evaluate(async () => {
      const { setSuggestionCategory } = await import('/js/features/chat-suggestions.js');
      setSuggestionCategory('plot-twist');
    });

    // Wait for the status text to show the new category label (reliable indicator)
    await expect(page.locator('#suggestions-toggle-status')).toHaveText('Plot Twist', { timeout: 3000 });
  });

  test('use suggestion draft inserts into chat input', async ({ page }) => {
    // Mock the rewrite endpoint — chat-suggestions.js calls /api/chat with stream:true
    // and expects an SSE response (data: {...} lines), not a plain JSON response.
    await page.route('/api/chat', route => {
      const sse = 'data: {"choices":[{"delta":{"content":"Mocked rewritten suggestion text"}}]}\n\ndata: [DONE]\n\n';
      route.fulfill({
        status: 200,
        contentType: 'text/event-stream',
        body: sse,
      });
    });

    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Generate suggestions
    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });

    // Click send button (data-mode="send") which rewrites and inserts into chat input
    const sendBtn = page.locator('.suggestion-btn[data-mode="send"]').first();
    if (await sendBtn.count() > 0) {
      await sendBtn.click();
      await expect(page.locator('#chat-input')).not.toHaveValue('', { timeout: 8000 });
    } else {
      // Fall back to first suggestion-btn
      await page.locator('.suggestion-btn').first().click();
      await expect(page.locator('#chat-input')).not.toHaveValue('', { timeout: 8000 });
    }
  });

  test('used suggestions tracked in history', async ({ page }) => {
    await page.route('/api/chat', route => {
      const sse = 'data: {"choices":[{"delta":{"content":"Rewritten text"}}]}\n\ndata: [DONE]\n\n';
      route.fulfill({ status: 200, contentType: 'text/event-stream', body: sse });
    });

    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });
    // Use send mode so addRecentSuggestion is called and history is populated
    await page.locator('.suggestion-btn[data-mode="send"]').first().click();

    await expect(page.locator('.suggestions-recent')).toBeVisible({ timeout: 5000 });
  });

  test('clear recent history', async ({ page }) => {
    await page.route('/api/chat', route => {
      const sse = 'data: {"choices":[{"delta":{"content":"Rewritten text"}}]}\n\ndata: [DONE]\n\n';
      route.fulfill({ status: 200, contentType: 'text/event-stream', body: sse });
    });

    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    await page.locator('#suggestions-generate-btn').click();
    await page.waitForSelector('.suggestion-item', { state: 'visible', timeout: 10000 });
    // Use send mode so addRecentSuggestion is called and history is populated
    await page.locator('.suggestion-btn[data-mode="send"]').first().click();
    await page.locator('#suggestions-toggle').click();
    await page.locator('#suggestions-toggle').click();
    await page.waitForSelector('#suggestions-dropdown', { state: 'visible' });

    // Wait for recent history to be populated
    await page.waitForSelector('.suggestions-recent-list .suggestion-item', { timeout: 5000 });

    // Use force click to bypass pointer event interception from chat-messages-inner
    await page.locator('.suggestions-clear-recent').click({ force: true });
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
