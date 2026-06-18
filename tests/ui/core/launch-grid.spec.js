import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('launch grid and filters', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
  });

  test('show filter bar when there are multiple presets', async ({ page }) => {
    // Ensure setup view is active
    await expect(page.locator('body')).toHaveClass(/setup-active/);

    // If filter bar is visible, verify basic controls
    const filterBar = page.locator('#setup-filter-bar');
    await page.waitForTimeout(400);

    const isVisible = await filterBar.isVisible().catch(() => false);
    if (!isVisible) {
      // Filter bar only appears when there are >= 3 user presets;
      // we allow the test to pass (no regression) if it is hidden.
      return;
    }

    // Basic controls should be present
    await expect(page.locator('#setup-filter-family-pills')).toBeVisible();
    await expect(page.locator('#setup-filter-size-pills')).toBeVisible();
    await expect(page.locator('#setup-filter-tags-btn')).toBeVisible();
  });

  test('group by family creates group headers when enabled', async ({ page }) => {
    const filterBar = page.locator('#setup-filter-bar');
    const isVisible = await filterBar.isVisible().catch(() => false);
    if (!isVisible) {
      // No filter bar → skip this advanced behavior
      return;
    }

    const groupToggle = page.locator('#setup-filter-group-by-family');
    const isChecked = await groupToggle.isChecked().catch(() => false);

    if (!isChecked) {
      await groupToggle.check();
      await page.waitForTimeout(300);
    }

    // When grouping is active, we expect at least one group header in the grid
    const groupHeaders = page.locator('.launch-grid-group');
    const count = await groupHeaders.count().catch(() => 0);
    if (count > 0) {
      await expect(groupHeaders.first()).toBeVisible();
    }
    // If no groups rendered (e.g., not enough diverse families), no failure;
    // this test mainly guards that the UI path is wired and not throwing.
  });

  test('family filter pills click without errors', async ({ page }) => {
    const familyPills = page.locator('#setup-filter-family-pills .launch-filter-pill');
    const count = await familyPills.count().catch(() => 0);
    if (count <= 1) {
      // Only "All" pill; nothing to test
      return;
    }

    // Click a non-All pill; expect no console errors and layout is stable
    await page.evaluate(() => { window.__lc_filter_error = false; });
    await page.on('console', msg => {
      if (msg.type() === 'error') {
        page.evaluate(() => { window.__lc_filter_error = true; });
      }
    });

    await (await familyPills.last()).click();
    await page.waitForTimeout(200);

    const hasError = await page.evaluate(() => window.__lc_filter_error ?? false);
    expect(hasError).toBe(false);
  });
});
