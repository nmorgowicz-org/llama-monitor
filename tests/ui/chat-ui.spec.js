import { test, expect } from '@playwright/test';

test.describe('chat UI shell', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.top-nav-bar');
    // Switch to monitor view so chat page is accessible
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    await expect(page.locator('#page-chat')).toBeVisible();
  });

  test('renders empty chat state with input bar', async ({ page }) => {
    await expect(page.locator('#chat-messages')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeVisible();
    await expect(page.locator('#chat-input')).toBeEditable();
  });

  test('creates new tab on + button click', async ({ page }) => {
    const tabCount = await page.locator('.chat-tab').count();
    await page.locator('.chat-tab-add').click();
    await expect(page.locator('.chat-tab')).toHaveCount(tabCount + 1);
  });

  test('switches between tabs', async ({ page }) => {
    await page.locator('.chat-tab-add').click();
    const tabs = await page.locator('.chat-tab').all();
    // Click the first tab
    await tabs[0].click();
    await expect(tabs[0]).toHaveClass(/active/);
  });

  test('shows chat header controls', async ({ page }) => {
    await expect(page.locator('#btn-system-prompt')).toBeVisible();
    await expect(page.locator('#btn-model-params')).toBeVisible();
    await expect(page.locator('#chat-explicit-toggle-footer')).toBeVisible();
  });
});

test.describe('system prompt panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.top-nav-bar');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('opens and closes on button click', async ({ page }) => {
    await expect(page.locator('#chat-system-panel')).not.toBeVisible();
    await page.locator('#btn-system-prompt').click();
    await expect(page.locator('#chat-system-panel')).toBeVisible();
    await page.locator('#btn-system-prompt').click();
    await expect(page.locator('#chat-system-panel')).not.toBeVisible();
  });

  test('allows editing system prompt', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await page.locator('#chat-system-input').fill('You are a test assistant.');
    await expect(page.locator('#chat-system-input')).toHaveValue('You are a test assistant.');
    // Indicator should appear when prompt is set
    await expect(page.locator('#system-prompt-indicator')).toBeVisible();
  });

  test('shows template dropdown', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    await expect(page.locator('#chat-template-select')).toBeVisible();
    await expect(page.locator('.chat-template-mgmt-btn')).toBeVisible();
  });
});

test.describe('explicit mode toggle', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.top-nav-bar');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('toggles explicit mode state', async ({ page }) => {
    // Initially not active
    await expect(page.locator('#chat-explicit-toggle-footer')).not.toHaveClass(/active/);
    // Click to enable
    await page.locator('#chat-explicit-toggle-footer').click();
    await expect(page.locator('#chat-explicit-toggle-footer')).toHaveClass(/active/);
    // Click to disable
    await page.locator('#chat-explicit-toggle-footer').click();
    await expect(page.locator('#chat-explicit-toggle-footer')).not.toHaveClass(/active/);
  });

  test('toggle in settings panel mirrors footer toggle', async ({ page }) => {
    await page.locator('#btn-system-prompt').click();
    // Enable via settings panel toggle
    await page.locator('#chat-explicit-toggle-settings').click();
    await expect(page.locator('#chat-explicit-toggle-settings')).toHaveClass(/active/);
    // Footer should also be active
    await expect(page.locator('#chat-explicit-toggle-footer')).toHaveClass(/active/);
  });
});

test.describe('template manager', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.top-nav-bar');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
    await page.locator('#btn-system-prompt').click();
  });

  test('opens modal on manage button click', async ({ page }) => {
    await expect(page.locator('#template-manager-modal')).not.toHaveClass(/active/);
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('#template-manager-modal')).toHaveClass(/active/);
  });

  test('lists default templates', async ({ page }) => {
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('#template-list')).toBeVisible();
    // Should have at least the default templates
    const items = await page.locator('.template-list-item').count();
    expect(items).toBeGreaterThan(0);
  });

  test('explicit policy section is present', async ({ page }) => {
    await page.locator('.chat-template-mgmt-btn').click();
    await expect(page.locator('.explicit-policy-section')).toBeVisible();
    await expect(page.getByText('Explicit Mode Policy')).toBeVisible();
    await expect(page.locator('#explicit-policy-input')).toBeVisible();
  });
});

test.describe('model params panel', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('.top-nav-bar');
    await page.evaluate(() => switchView('monitor'));
    await page.getByRole('button', { name: /chat/i }).click();
  });

  test('opens and closes on button click', async ({ page }) => {
    await expect(page.locator('#chat-params-panel')).not.toBeVisible();
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#chat-params-panel')).toBeVisible();
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#chat-params-panel')).not.toBeVisible();
  });

  test('shows temperature and top_p controls', async ({ page }) => {
    await page.locator('#btn-model-params').click();
    await expect(page.locator('#chat-temperature')).toBeVisible();
    await expect(page.locator('#chat-top-p')).toBeVisible();
  });
});
