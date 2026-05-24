// ── Command Palette Tests ──────────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('command palette', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('opens on Ctrl+K keyboard shortcut', async ({ page }) => {
    await expect(page.locator('#command-palette-overlay')).not.toBeVisible();
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('#command-palette-input')).toBeFocused();
  });

  test('shows quick actions when input is empty', async ({ page }) => {
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible();

    const items = page.locator('.command-palette-item');
    await expect(items).toHaveCount(2);

    const titles = page.locator('.command-palette-item-title');
    await expect(titles.nth(0)).toContainText('New Chat');
    await expect(titles.nth(1)).toContainText('Search Messages');
  });

  test('closes on Escape', async ({ page }) => {
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('#command-palette-overlay')).not.toBeVisible({ timeout: 3000 });
  });

  test('Ctrl+K toggles open and closed', async ({ page }) => {
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible();
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).not.toBeVisible({ timeout: 3000 });
  });

  test('filters conversations by title', async ({ page }) => {
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab } = await import('/js/features/chat-state.js');
      chat.tabs.push(newChatTab('Alpha Chat'), newChatTab('Beta Chat'), newChatTab('Gamma Chat'));
    });

    await page.keyboard.press('Control+K');
    await page.locator('#command-palette-input').fill('Beta');

    // Wait for search results to render (performSearch is async — awaits FTS HTTP call)
    const titles = page.locator('.command-palette-item-title');
    // Wait until at least one title contains "Beta"
    await expect(titles.filter({ hasText: 'Beta' })).toHaveCount(1, { timeout: 5000 });
  });

  test('keyboard navigation with arrow keys and Enter', async ({ page }) => {
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible();

    await page.keyboard.press('ArrowDown');
    await expect(page.locator('.command-palette-item.active')).toHaveCount(1);
    await expect(page.locator('.command-palette-item.active .command-palette-item-title')).toContainText('New Chat');

    await page.keyboard.press('ArrowDown');
    await expect(page.locator('.command-palette-item.active .command-palette-item-title')).toContainText('Search Messages');

    await page.keyboard.press('ArrowUp');
    await expect(page.locator('.command-palette-item.active .command-palette-item-title')).toContainText('New Chat');
  });

  test('click on overlay background closes palette', async ({ page }) => {
    await page.keyboard.press('Control+K');
    await expect(page.locator('#command-palette-overlay')).toBeVisible();

    // Get overlay box and click outside the inner palette area
    const overlayBox = await page.locator('#command-palette-overlay').boundingBox();
    // Click near the edge of the overlay (outside the centered palette)
    await page.mouse.click(overlayBox.x + 10, overlayBox.y + 10);
    await expect(page.locator('#command-palette-overlay')).not.toBeVisible({ timeout: 3000 });
  });

  test('New Chat action creates a new tab', async ({ page }) => {
    const beforeCount = await page.locator('#csp-list .csp-item').count();

    await page.keyboard.press('Control+K');
    await page.locator('.command-palette-item-title:has-text("New Chat")').click();

    await expect(page.locator('#csp-list .csp-item')).toHaveCount(beforeCount + 1, { timeout: 5000 });
  });
});
