import { test, expect } from '@playwright/test';

// The preset editor reads a set of Rapid-MLX controls on save. This spec asks the only
// question that matters about them: with a Rapid-MLX preset open, can a user actually see
// and change them? A control the save path reads but nobody can reach does not persist a
// user's choice -- it persists its own default, overwriting whatever the wizard set.
//
// The list is derived from presets.js rather than hardcoded, because the CSS in
// modal-premium.css hides Rapid controls by default and exempts them one id at a time.
// Deriving it means a control added tomorrow is covered without anyone updating this file.
test.describe('Rapid-MLX preset editor control reachability', () => {
  test('@in-memory-test every control the save path reads is visible in a Rapid preset', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    const ids = await page.evaluate(async () => {
      const src = await (await fetch('/js/features/presets.js')).text();
      return [...new Set(src.match(/modal-rapid-[a-z0-9-]+/g) || [])];
    });
    expect(ids.length, 'expected presets.js to reference Rapid-MLX controls').toBeGreaterThan(8);

    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      mod.openPresetModal('new', null, { name: 'probe', backend: 'rapid_mlx', rapid_mlx: {} });
    });
    // All of these live in the advanced section; select it before measuring.
    await page.locator('#preset-modal .preset-nav-item[data-section="advanced"]').click();

    const hidden = [];
    for (const id of ids) {
      const el = page.locator(`#${id}`);
      // A referenced id with no element is a separate defect, out of scope here.
      if (await el.count() === 0) continue;
      if (!(await el.isVisible())) hidden.push(id);
    }
    expect(hidden, 'controls read on save but never rendered').toEqual([]);
  });
});
