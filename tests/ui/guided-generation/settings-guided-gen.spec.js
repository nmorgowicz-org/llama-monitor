// ── Settings - Guided Generation Tests ───────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat } from './fixtures.js';

test.describe('Settings - Guided Generation', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
  });

  test('toggle context notes enabled', async ({ page }) => {
    // Open settings
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    // Find context notes toggle in settings
    const contextNotesToggle = page.locator('label', { hasText: 'Context Notes' });
    await expect(contextNotesToggle).toBeVisible();

    // Toggle checkbox
    const checkbox = contextNotesToggle.locator('input[type="checkbox"]');
    await checkbox.click();

    // Should be checked
    await expect(checkbox).toBeChecked();
  });

  test('toggle suggestions enabled', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const suggestionsToggle = page.locator('label', { hasText: 'Suggestions' });
    await expect(suggestionsToggle).toBeVisible();

    const checkbox = suggestionsToggle.locator('input[type="checkbox"]');
    await checkbox.click();

    await expect(checkbox).toBeChecked();
  });

  test('toggle quick guide enabled', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const quickGuideToggle = page.locator('label', { hasText: 'Quick Guide' });
    await expect(quickGuideToggle).toBeVisible();

    const checkbox = quickGuideToggle.locator('input[type="checkbox"]');
    await checkbox.click();

    await expect(checkbox).toBeChecked();
  });

  test('sidebar width slider updates display', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const slider = page.locator('#settings-sidebar-width');
    await expect(slider).toBeVisible();

    // Set to 350
    await slider.fill('350');
    await expect(slider).toHaveValue('350');

    // Value display should update
    const valueDisplay = page.locator('#settings-sidebar-width-value');
    await expect(valueDisplay).toContainText('350px');
  });

  test('suggestion count slider updates display', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const slider = page.locator('#settings-suggestion-count');
    await expect(slider).toBeVisible();

    await slider.fill('7');
    await expect(slider).toHaveValue('7');

    const valueDisplay = page.locator('#settings-suggestion-count-value');
    await expect(valueDisplay).toContainText('7');
  });

  test('context depth slider updates display', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const slider = page.locator('#settings-context-depth');
    await expect(slider).toBeVisible();

    await slider.fill('15');
    await expect(slider).toHaveValue('15');

    const valueDisplay = page.locator('#settings-context-depth-value');
    await expect(valueDisplay).toContainText('15');
  });

  test('edit suggestion prompt', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    const promptTextarea = page.locator('#settings-prompt-general');
    await expect(promptTextarea).toBeVisible();

    await promptTextarea.fill('Custom prompt here');
    await expect(promptTextarea).toHaveValue('Custom prompt here');
  });

  test('reset prompts to defaults', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    // Modify a prompt first
    await page.locator('#settings-prompt-general').fill('Modified prompt');

    // Click reset
    await page.locator('#settings-reset-prompts').click();

    // Should show confirmation toast
    await expect(page.locator('.toast', { hasText: /reset/i })).toBeVisible({ timeout: 5000 });

    // Prompt should be restored
    const promptValue = await page.locator('#settings-prompt-general').inputValue();
    expect(promptValue).toContain('creative brainstorming');
  });

  test('save shows success feedback', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    // Modify something
    await page.locator('#settings-sidebar-width').fill('300');

    // Click save
    const saveButton = page.locator('#settings-save-btn');
    await saveButton.click();

    // Should show saved state
    await expect(saveButton).toContainText('✓ Saved', { timeout: 5000 });
  });

  test('Ctrl+S saves in modal', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.waitForSelector('#chat-system-panel', { state: 'visible' });

    // Modify something
    await page.locator('#settings-prompt-general').fill('Test');

    // Ctrl+S
    await page.keyboard.press('Control+S');

    // Should show saved
    await expect(page.locator('#settings-save-btn')).toContainText('✓ Saved', { timeout: 5000 });
  });
});
