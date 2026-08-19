import { test, expect } from '@playwright/test';

test.describe('@in-memory-test shared evidence drawer', () => {
  test('is singleton, expandable, and restores focus on Escape', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await page.evaluate(() => {
      const opener = document.createElement('button');
      opener.id = 'evidence-test-opener';
      opener.textContent = 'Explain';
      document.body.appendChild(opener);
      opener.focus();
      window.openEvidenceDrawer({
        title: 'Memory evidence',
        status: 'caution',
        summary: 'This configuration is near the memory limit.',
        evidence: ['Formula-based Rapid overhead'],
        adjustments: ['KV: int4 → int8'],
      }, opener);
      window.openEvidenceDrawer({
        title: 'Memory evidence',
        status: 'caution',
        summary: 'This configuration is near the memory limit.',
        evidence: ['Formula-based Rapid overhead'],
        adjustments: ['KV: int4 → int8'],
      }, opener);
    });

    await expect(page.locator('#evidence-drawer')).toHaveCount(1);
    await expect(page.locator('#evidence-drawer')).toHaveClass(/open/);
    await expect(page.locator('#evidence-drawer-title')).toHaveText('Memory evidence');
    await page.locator('.evidence-drawer-details > summary').click();
    await expect(page.locator('.evidence-drawer-technical')).toContainText('KV: int4 → int8');
    await page.keyboard.press('Escape');
    await expect(page.locator('#evidence-drawer')).toBeHidden();
    await expect(page.locator('#evidence-test-opener')).toBeFocused();
  });
});
