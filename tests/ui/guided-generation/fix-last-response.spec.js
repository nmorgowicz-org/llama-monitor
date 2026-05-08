// ── Fix Last Response Tests ──────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat, seedChatMessages } from './fixtures.js';

test.describe('Fix Last Response', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
  });

  test('button visible only with assistant message', async ({ page }) => {
    // Seed with assistant message
    await seedChatMessages(page, [
      { role: 'user', content: 'What is 2+2?', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: '4', timestamp_ms: Date.now() }
    ]);

    await expect(page.locator('#btn-fix-last')).toBeVisible();
  });

  test('button hidden with user message last', async ({ page }) => {
    // Seed with user message last
    await seedChatMessages(page, [
      { role: 'user', content: 'Hello', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Hi!', timestamp_ms: Date.now() - 500 },
      { role: 'user', content: 'How are you?', timestamp_ms: Date.now() }
    ]);

    await expect(page.locator('#btn-fix-last')).not.toBeVisible();
  });

  test('button hidden with empty chat', async ({ page }) => {
    await seedChatMessages(page, []);

    await expect(page.locator('#btn-fix-last')).not.toBeVisible();
  });

  test('open modal with fix button', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'What is the capital of France?', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Berlin', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();

    // Modal should open
    await expect(page.locator('#fix-last-modal')).toHaveClass(/active/);
    await expect(page.locator('#fix-last-instruction')).toBeVisible();
  });

  test('close modal with X button', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Test', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Test response', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();
    await page.waitForSelector('#fix-last-modal', { state: 'visible' });

    await page.locator('#fix-last-close').click();

    // Should add closing class
    await expect(page.locator('#fix-last-modal')).toHaveClass(/closing/);
    
    // Then should close
    await expect(page.locator('#fix-last-modal')).not.toHaveClass(/active/, { timeout: 1000 });
  });

  test('close modal with Escape', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Test', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Test response', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();
    await page.waitForSelector('#fix-last-modal', { state: 'visible' });

    await page.keyboard.press('Escape');

    await expect(page.locator('#fix-last-modal')).toHaveClass(/closing/);
  });

  test('submit validation requires correction', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Test', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Test response', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();
    await page.waitForSelector('#fix-last-modal', { state: 'visible' });

    // Leave empty and try to submit
    await page.locator('#fix-last-submit').click();

    // Should show error toast
    await expect(page.locator('.toast', { hasText: /correction/i })).toBeVisible({ timeout: 5000 });
  });

  test('submit with Ctrl+Enter', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'What is the capital of France?', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Berlin', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();
    await page.waitForSelector('#fix-last-modal', { state: 'visible' });

    await page.locator('#fix-last-instruction').fill('It is Paris, not Berlin');
    await page.locator('#fix-last-instruction').press('Control+Enter');

    // Should show success toast
    await expect(page.locator('.toast', { hasText: /regenerating/i })).toBeVisible({ timeout: 5000 });
  });

  test('tab switch closes modal', async ({ page }) => {
    await seedChatMessages(page, [
      { role: 'user', content: 'Test', timestamp_ms: Date.now() - 1000 },
      { role: 'assistant', content: 'Test response', timestamp_ms: Date.now() }
    ]);

    await page.locator('#btn-fix-last').click();
    await page.waitForSelector('#fix-last-modal', { state: 'visible' });

    // Create new tab
    await page.locator('.chat-tab-add').click();

    // Modal should close
    await expect(page.locator('#fix-last-modal')).not.toHaveClass(/active/, { timeout: 1000 });
  });
});
