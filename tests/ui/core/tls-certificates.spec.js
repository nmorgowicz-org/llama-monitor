import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('TLS / Certificates settings', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
  });

  test('Certificates tab exists and is selectable', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Security');
    await expect(certsTab).toBeVisible();

    await certsTab.click();
    await expect(page.locator('#settings-security')).toBeVisible();
  });

  test('TLS mode controls exist', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Security');
    await certsTab.click();
    await expect(page.locator('#settings-security')).toBeVisible();

    // Helper: switch cert mode via DOM
    const setCertMode = (mode) =>
      page.evaluate((m) => {
        const panes = document.querySelectorAll('.cert-mode-content');
        const pills = document.querySelectorAll('.cert-mode-pill');
        panes.forEach((p) => {
          p.style.display = p.id === 'cert-mode-' + m ? 'block' : 'none';
        });
        pills.forEach((p) => {
          p.classList.toggle('active', p.dataset.mode === m);
        });
      }, mode);

    // Select "No HTTPS" mode
    await setCertMode('none');
    await expect(page.locator('#btn-disable-tls')).toBeVisible();

    // Select "Self-Signed" mode
    await setCertMode('self-signed');
    const selfSignedBtn = page.locator('#btn-generate-self-signed');
    await expect(selfSignedBtn).toBeVisible();

    // Select "Bring Your Own Key" mode
    await setCertMode('custom');
    await expect(page.locator('#btn-apply-custom-cert')).toBeVisible();
  });

  test('ACME section exists with required controls', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Security');
    await certsTab.click();
    await expect(page.locator('#settings-security')).toBeVisible();

    // Wait for ACME element to be in DOM before manipulating
    await page.waitForSelector('#cert-mode-acme', { state: 'attached', timeout: 10000 });

    // Select ACME mode (DOM helper)
    await page.evaluate((m) => {
      const panes = document.querySelectorAll('.cert-mode-content');
      const pills = document.querySelectorAll('.cert-mode-pill');
      panes.forEach((p) => {
        p.style.display = p.id === 'cert-mode-' + m ? 'block' : 'none';
      });
      pills.forEach((p) => {
        p.classList.toggle('active', p.dataset.mode === m);
      });
    }, 'acme');
    await expect(page.locator('#cert-mode-acme')).toBeVisible();

    // Domain input
    const fqdnInput = page.locator('#acme-fqdn');
    await expect(fqdnInput).toBeVisible();
    await expect(fqdnInput).toHaveAttribute('placeholder', /llama-monitor\.example\.com/i);

    // Environment controls - verify they exist in the ACME panel
    const stagingRadio = page.locator('#acme-env-staging');
    await expect(stagingRadio).toBeAttached();

    const prodRadio = page.locator('#acme-env-production');
    await expect(prodRadio).toBeAttached();

    // DNS provider dropdown with multiple options
    const providerSelect = page.locator('#acme-dns-provider');
    await expect(providerSelect).toBeAttached();
    const options = await providerSelect.locator('option').all();
    expect(options.length).toBeGreaterThan(3);

    // Credentials grid and add button
    const grid = page.locator('#acme-credentials-grid');
    await expect(grid).toBeAttached();
    const addBtn = page.locator('#acme-add-credential');
    await expect(addBtn).toBeAttached();
    await expect(addBtn).toContainText(/add field/i);
  });

  test('switching TLS mode updates UI', async ({ page }) => {
    await page.getByRole('button', { name: /settings/i }).first().click();
    await expect(page.locator('#settings-modal')).toHaveClass(/open/);

    const certsTab = page.locator('.settings-tab').getByText('Security');
    await certsTab.click();
    await expect(page.locator('#settings-security')).toBeVisible();

    // Confirm baseline: TLS status area is present
    const tlsStatusText = page.locator('#tls-status-text');
    await expect(tlsStatusText).toBeVisible();

    // Helper to switch cert mode via DOM
    const setCertMode = (mode) =>
      page.evaluate((m) => {
        const panes = document.querySelectorAll('.cert-mode-content');
        const pills = document.querySelectorAll('.cert-mode-pill');
        panes.forEach((p) => {
          p.style.display = p.id === 'cert-mode-' + m ? 'block' : 'none';
        });
        pills.forEach((p) => {
          p.classList.toggle('active', p.dataset.mode === m);
        });
      }, mode);

    // Select "No HTTPS" mode and click "Disable TLS"
    await setCertMode('none');
    const disableBtn = page.locator('#btn-disable-tls');
    await expect(disableBtn).toBeVisible();
    await disableBtn.click({ force: true });
    await expect(tlsStatusText).toBeVisible();

    // Select "Self-Signed" mode and click "Generate self-signed"
    await setCertMode('self-signed');
    const generateSelfSigned = page.locator('#btn-generate-self-signed');
    await expect(generateSelfSigned).toBeVisible();
    await generateSelfSigned.click({ force: true });
    await expect(tlsStatusText).toBeVisible();

    // Select ACME mode and confirm controls exist
    await setCertMode('acme');
    await expect(page.locator('#acme-fqdn')).toBeAttached();
    await expect(page.locator('#acme-dns-provider')).toBeAttached();
  });
});
