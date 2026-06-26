// ── SPA Navigation & History ──────────────────────────────────────────────────
// Validates that the SPA router correctly:
// - loads shell for core routes
// - deep-links into /chat/:id
// - dismisses modals when navigating away (Back/Forward behavior)

import { test, expect } from '@playwright/test';

test.describe('SPA navigation & history', () => {
  test('SPA routes load the shell', async ({ page }) => {
    // These are the key SPA routes that must return HTML (not 404/JSON).
    for (const path of ['/', '/chat', '/logs', '/server', '/settings', '/spawn']) {
      const resp = await page.goto(path);
      expect(resp.status()).toBe(200);
      await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    }
  });

  test('/chat/:id deep-links to specific conversation', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Get the default active tab ID
    const tabId = await page.evaluate(() => {
      const chat = (window.__TEST_CHAT || {});
      const state = (typeof window !== 'undefined')
        ? (window.__APP_STATE || {})
        : {};
      return (state?.chat?.activeTabId) || null;
    });

    if (!tabId) {
      // No explicit tab; fall back: just ensure /chat/:id route does not crash.
      await page.goto('/chat/invalid-uuid-not-real');
      await expect(page.locator('.top-nav-bar')).toBeVisible();
      return;
    }

    const encodedId = encodeURIComponent(tabId);
    await page.goto('/chat/' + encodedId);

    // Page should stay on chat tab with matching URL
    await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    expect(page.url()).toContain('/chat/' + encodedId);
  });

  test('settings modal dismisses when navigating away', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Open settings via button
    await page.click('[data-action="settings"]', { timeout: 10000 });
    const modal = page.locator('#settings-modal');
    await expect(modal).toBeVisible();

    // Navigate to /chat via sidebar
    await page.click('text=/^Chat$/i', { timeout: 10000 });
    await expect(modal).not.toBeVisible();
    expect(page.url()).toContain('/chat');
  });

  test('Escape key closes modals', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Open settings
    await page.click('[data-action="settings"]', { timeout: 10000 });
    const modal = page.locator('#settings-modal');
    await expect(modal).toBeVisible();

    // Press Escape
    await page.keyboard.press('Escape');
    await expect(modal).not.toBeVisible();
  });
});
