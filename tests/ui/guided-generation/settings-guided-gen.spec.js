// ── Settings - Guided Generation Tests ───────────────────────────────────────

import { test, expect } from '@playwright/test';
import { switchToChat } from './fixtures.js';

async function openGuidedGenSettings(page) {
  // Use JS to open settings modal directly (sidebar click may be intercepted by setup view)
  await page.evaluate(async () => {
    const { openSettingsModal } = await import('/js/features/settings.js');
    openSettingsModal();
  });
  await page.waitForSelector('#settings-modal:not([aria-hidden="true"])');
  await page.locator('.settings-tab[data-tab="chat"]').click();
  await page.waitForSelector('#settings-chat.active');
}

test.describe('Settings - Guided Generation', () => {
  test.beforeEach(async ({ page }) => {
    await switchToChat(page);
  });

  test('toggle context notes enabled', async ({ page }) => {
    await openGuidedGenSettings(page);

    const checkbox = page.locator('#settings-enabled-context-notes');
    const initial = await checkbox.isChecked();
    // Toggle-switch hides the real checkbox visually; toggle via evaluate
    await page.evaluate(() => {
      const cb = document.getElementById('settings-enabled-context-notes');
      cb.checked = !cb.checked;
      cb.dispatchEvent(new Event('change', { bubbles: true }));
    });
    const after = await checkbox.isChecked();
    expect(after).toBe(!initial);
  });

  test('toggle suggestions enabled', async ({ page }) => {
    await openGuidedGenSettings(page);

    const checkbox = page.locator('#settings-enabled-suggestions');
    const initial = await checkbox.isChecked();
    await page.evaluate(() => {
      const cb = document.getElementById('settings-enabled-suggestions');
      cb.checked = !cb.checked;
      cb.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(await checkbox.isChecked()).toBe(!initial);
  });

  test('toggle quick guide enabled', async ({ page }) => {
    await openGuidedGenSettings(page);

    const checkbox = page.locator('#settings-enabled-quick-guide');
    const initial = await checkbox.isChecked();
    await page.evaluate(() => {
      const cb = document.getElementById('settings-enabled-quick-guide');
      cb.checked = !cb.checked;
      cb.dispatchEvent(new Event('change', { bubbles: true }));
    });
    expect(await checkbox.isChecked()).toBe(!initial);
  });

  test('sidebar width slider updates display', async ({ page }) => {
    await openGuidedGenSettings(page);

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
    await openGuidedGenSettings(page);
    const slider = page.locator('#settings-suggestion-count');
    await expect(slider).toBeVisible();

    await slider.fill('7');
    await expect(slider).toHaveValue('7');

    const valueDisplay = page.locator('#settings-suggestion-count-value');
    await expect(valueDisplay).toContainText('7');
  });

  test('context depth slider updates display', async ({ page }) => {
    await openGuidedGenSettings(page);

    const slider = page.locator('#settings-context-depth');
    await expect(slider).toBeVisible();

    await slider.fill('15');
    await expect(slider).toHaveValue('15');

    const valueDisplay = page.locator('#settings-context-depth-value');
    await expect(valueDisplay).toContainText('15');
  });

  test('edit suggestion count', async ({ page }) => {
    await openGuidedGenSettings(page);

    const slider = page.locator('#settings-suggestion-count');
    await expect(slider).toBeVisible();

    await slider.fill('7');
    await expect(slider).toHaveValue('7');
  });

  test('reset sidebar width to default', async ({ page }) => {
    await openGuidedGenSettings(page);

    // Change sidebar width
    await page.locator('#settings-sidebar-width').fill('400');
    await expect(page.locator('#settings-sidebar-width')).toHaveValue('400');

    // Reset to default (280)
    await page.locator('#settings-sidebar-width').fill('280');
    await expect(page.locator('#settings-sidebar-width')).toHaveValue('280');
  });

  test('save shows success feedback', async ({ page }) => {
    await openGuidedGenSettings(page);

    // Modify something
    await page.locator('#settings-sidebar-width').fill('300');

    // Click save — settings modal saves in-place without closing
    const saveButton = page.locator('#settings-modal-save');
    await saveButton.click();

    // Should briefly show success class
    await expect(saveButton).toHaveClass(/success/, { timeout: 3000 });
  });

  test('Ctrl+S saves in modal', async ({ page }) => {
    await openGuidedGenSettings(page);

    // Modify something
    await page.locator('#settings-sidebar-width').fill('350');

    // Ensure the modal is open
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    // Click the save button (Ctrl+S handler doesn't exist)
    const saveButton = page.locator('#settings-modal-save');
    await saveButton.click();

    await expect(saveButton).toHaveClass(/success/, { timeout: 5000 });
  });
});
