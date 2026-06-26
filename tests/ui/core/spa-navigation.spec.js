// ── SPA Navigation & History ──────────────────────────────────────────────────
// Validates:
// - SPA routes load the shell
// - /chat/:id deep-links
// - modals dismiss when navigating away via Router

import { test, expect } from '@playwright/test';

test.describe('SPA navigation & history', () => {
  test('SPA routes load the shell', async ({ page }) => {
    for (const path of ['/', '/chat', '/logs', '/server', '/spawn']) {
      const resp = await page.goto(path);
      expect(resp.status()).toBe(200);
      await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    }
  });

  test('/chat/:id deep-links to specific conversation', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Ensure we are in chat view
    await page.evaluate(async () => {
      const Router = (await import('/js/features/router.js')).default;
      Router.navigate('/chat');
    });

    // Get the active tab ID (if any)
    const tabId = await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      return chat.activeTabId || null;
    });

    if (!tabId) {
      // No explicit tab; ensure /chat/:id route does not crash.
      await page.goto('/chat/invalid-uuid-not-real');
      await expect(page.locator('.top-nav-bar')).toBeVisible();
      return;
    }

    const encodedId = encodeURIComponent(tabId);
    await page.goto('/chat/' + encodedId);

    // Page should be on chat with matching URL
    await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    expect(page.url()).toContain('/chat/' + encodedId);
  });

  test('settings modal dismisses when navigating away', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Open settings via JS (same pattern as app-shell tests)
    await page.evaluate(async () => {
      const { openSettingsModal } = await import('/js/features/settings.js');
      openSettingsModal();
    });
    const modal = page.locator('#settings-modal');
    await expect(modal).toHaveClass(/open/);

    // Navigate away using Router (avoids sidebar click issues)
    await page.evaluate(async () => {
      const Router = (await import('/js/features/router.js')).default;
      Router.navigate('/chat');
    });

    await expect(modal).not.toHaveClass(/open/);
    expect(page.url()).toContain('/chat');
  });

  test('Escape key closes settings modal', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    await page.evaluate(async () => {
      const { openSettingsModal } = await import('/js/features/settings.js');
      openSettingsModal();
    });
    const modal = page.locator('#settings-modal');
    await expect(modal).toHaveClass(/open/);

    await page.keyboard.press('Escape');
    await expect(modal).not.toHaveClass(/open/);
  });
});
