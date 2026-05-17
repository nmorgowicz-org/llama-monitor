import { test, expect } from '@playwright/test';

test.describe('TLS / Certificates settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
  });

  test('Certificates tab exists and is selectable', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Certificates');
    await expect(certsTab).toBeVisible();

    await certsTab.click();
    await expect(page.locator('#settings-certificates')).toBeVisible();
  });

  test('TLS mode controls exist', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Certificates');
    await certsTab.click();
    await expect(page.locator('#settings-certificates')).toBeVisible();

    // No TLS button
    await expect(page.locator('#btn-disable-tls')).toBeVisible();

    // Self-signed button
    await expect(page.locator('#btn-generate-self-signed')).toBeVisible();

    // Custom certificate apply button
    await expect(page.locator('#btn-apply-custom-cert')).toBeVisible();
  });

  test('ACME section exists with required controls', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Certificates');
    await certsTab.click();
    await expect(page.locator('#settings-certificates')).toBeVisible();

    // Domain input
    const fqdnInput = page.locator('#acme-fqdn');
    await expect(fqdnInput).toBeVisible();
    await expect(fqdnInput).toHaveAttribute('placeholder', /llama-monitor\.example\.com/i);

    // Environment controls
    const stagingRadio = page.locator('#acme-env-staging');
    const prodRadio = page.locator('#acme-env-production');
    await expect(stagingRadio).toBeVisible();
    await expect(prodRadio).toBeVisible();

    // DNS provider dropdown with multiple options
    const providerSelect = page.locator('#acme-dns-provider');
    await expect(providerSelect).toBeVisible();
    const options = await providerSelect.locator('option').all();
    expect(options.length).toBeGreaterThan(3);

    // Credentials grid and add button
    const grid = page.locator('#acme-credentials-grid');
    await expect(grid).toBeVisible();
    const addBtn = page.locator('#acme-add-credential');
    await expect(addBtn).toBeVisible();
    await expect(addBtn).toContainText(/add field/i);
  });

  test('switching TLS mode updates UI', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Certificates');
    await certsTab.click();
    await expect(page.locator('#settings-certificates')).toBeVisible();

    // Confirm baseline: TLS status area is present
    const tlsStatusText = page.locator('#tls-status-text');
    await expect(tlsStatusText).toBeVisible();

    // Click "Disable TLS" and ensure no crash; status area still visible
    await page.locator('#btn-disable-tls').click();
    await expect(tlsStatusText).toBeVisible();

    // Click "Generate self-signed" and ensure no crash; status area still visible
    await page.locator('#btn-generate-self-signed').click();
    await expect(tlsStatusText).toBeVisible();

    // ACME section remains visible when interacting with other controls
    await expect(page.locator('#acme-fqdn')).toBeVisible();
    await expect(page.locator('#acme-dns-provider')).toBeVisible();
  });
});
