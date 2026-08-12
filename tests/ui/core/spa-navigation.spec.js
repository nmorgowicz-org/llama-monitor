// ── SPA Navigation & History ──────────────────────────────────────────────────
// Validates:
// - SPA routes load the shell
// - /chat/:id deep-links
// - modals dismiss when navigating away via Router

import { test, expect } from '@playwright/test';
import { openSettings } from '../helpers.js';

test.describe('SPA navigation & history', () => {
  test('SPA routes load the shell', async ({ page }) => {
    for (const path of ['/', '/chat', '/logs', '/server', '/spawn']) {
      const resp = await page.goto(path);
      expect(resp.status()).toBe(200);
      await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    }
  });

  test('session-dependent routes with no active session fall back to welcome', async ({ page }) => {
    // Pin the session state to "none" so the assertion exercises the routing
    // fallback rather than the backend's (load-dependent) live status.
    await page.route('**/api/sessions/active', route =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'No active session' }),
      })
    );

    // Hard-load session-dependent URLs with nothing attached/spawned. Each should
    // reconcile to the welcome screen (and normalize the URL to /) instead of
    // showing an empty monitor view — there is nothing the user can do on a
    // dashboard, log, or chat page without a live session.
    for (const path of ['/server', '/logs', '/chat', '/chat/some-session-id']) {
      await page.goto(path);
      await page.waitForSelector('html.modules-ready');

      // The URL reconciles to '/' once the (async) session check completes —
      // poll rather than asserting eagerly (modules-ready fires before it).
      await expect.poll(() => new URL(page.url()).pathname, { timeout: 10000 }).toBe('/');
      await expect(page.locator('body.setup-active')).toBeVisible();
      await expect(page.locator('#view-monitor')).toBeHidden();
    }
  });

  test('/chat/:id deep-links to specific conversation when a session is active', async ({ page }) => {
    // Deep-link reload-safety only applies when a session exists — otherwise the
    // app falls back to welcome (covered above). Mock an active session so the
    // router honors the /chat/:id route on a hard load.
    await page.route('**/api/sessions/active', route =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'test-session',
          name: 'Test',
          mode: 'Attach:http://127.0.0.1:9',
          status: 'Running',
        }),
      })
    );

    // Keep the deep-link assertion independent of the test server's persistent
    // chat database. The route under test is the browser URL reconciliation,
    // so provide one authenticated tab and its empty message history directly.
    const tab = {
      id: 'spa-navigation-tab',
      name: 'SPA navigation test',
      system_prompt: '',
      explicit_level: 0,
      auto_compact: true,
      auto_compact_summarize: false,
      compact_mode: 'summarize',
      compact_threshold: 0.8,
      model_params: {},
      context_notes: [],
      sidebar_width: 320,
      tab_order: 0,
      pinned: false,
      total_input_tokens: 0,
      total_output_tokens: 0,
      created_at: Date.now(),
      updated_at: Date.now(),
    };
    await page.route('**/api/chat/tabs*', async route => {
      const url = new URL(route.request().url());
      if (route.request().method() !== 'GET') return route.continue();
      const body = url.pathname.endsWith('/messages') ? [] : url.pathname === '/api/chat/tabs' ? [tab] : tab;
      await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(body) });
    });

    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    // Ensure we are in chat view
    await page.evaluate(async () => {
      const Router = (await import('/js/features/router.js')).default;
      Router.navigate('/chat');
    });

    // A default chat tab always exists, so an active tab id is available.
    await expect.poll(() => page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      return chat.activeTabId || null;
    })).toBe('spa-navigation-tab');
    const tabId = 'spa-navigation-tab';

    const encodedId = encodeURIComponent(tabId);
    await page.goto('/chat/' + encodedId);

    // Page should be on chat with the deep-link URL preserved.
    await expect(page.locator('html.modules-ready')).toBeVisible({ timeout: 15000 });
    expect(page.url()).toContain('/chat/' + encodedId);
  });

  test('settings modal dismisses when navigating away', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    await openSettings(page);
    const modal = page.locator('#settings-modal');
    await expect(modal).toHaveClass(/open/);

    // Navigate away using Router (avoids sidebar click issues). /logs, not /chat: the chat
    // route falls back to welcome when no session is active (see the test above), and whether
    // one is active depends on which specs ran earlier against this shared server. That made
    // the URL assertion pass or fail on test order rather than on the behaviour under test.
    await page.evaluate(async () => {
      const Router = (await import('/js/features/router.js')).default;
      Router.navigate('/logs');
    });

    await expect(modal).not.toHaveClass(/open/);
    expect(page.url()).toContain('/logs');
  });

  test('Escape key closes settings modal', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    await openSettings(page);
    const modal = page.locator('#settings-modal');
    await expect(modal).toHaveClass(/open/);

    await page.keyboard.press('Escape');
    await expect(modal).not.toHaveClass(/open/);
  });
});
