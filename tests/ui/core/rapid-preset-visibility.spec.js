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

    // Brace-match the Rapid branch of _buildFormPreset and take *every* modal-* id it reads,
    // not just `modal-rapid-*`. The narrower pattern missed the six sampling inputs
    // (modal-temperature, modal-top-p, ...), which the save path writes into
    // default_temperature/top_p/... and which lived in a section Rapid presets could not open.
    const ids = await page.evaluate(async () => {
      const src = await (await fetch('/js/features/presets.js')).text();
      const start = src.indexOf("if (existing.backend === 'rapid_mlx') {");
      if (start < 0) throw new Error('Rapid branch of _buildFormPreset not found');
      let depth = 0, end = start;
      for (let i = src.indexOf('{', start); i < src.length; i++) {
        if (src[i] === '{') depth++;
        else if (src[i] === '}' && --depth === 0) { end = i; break; }
      }
      const branch = src.slice(start, end);
      return [...new Set(branch.match(/modal-[a-z0-9-]+/g) || [])];
    });
    expect(ids.length, 'expected presets.js to reference Rapid-MLX controls').toBeGreaterThan(8);

    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      mod.openPresetModal('new', null, { name: 'probe', backend: 'rapid_mlx', rapid_mlx: {} });
    });
    // They are spread across sections now, so measure each id under every section a Rapid
    // preset can open. Visible in any one of them is enough -- unreachable in all of them
    // is the defect.
    const sections = await page.locator('#preset-modal .preset-nav-item:visible').evaluateAll(
      (els) => els.map((el) => el.dataset.section),
    );
    expect(sections.length, 'expected a Rapid preset to expose at least one section')
      .toBeGreaterThan(0);

    const hidden = [];
    for (const id of ids) {
      const el = page.locator(`#${id}`);
      // A referenced id with no element is a separate defect, out of scope here.
      if (await el.count() === 0) continue;
      let seen = false;
      for (const section of sections) {
        await page.locator(`#preset-modal .preset-nav-item[data-section="${section}"]`).click();
        if (await el.isVisible()) { seen = true; break; }
      }
      if (!seen) hidden.push(id);
    }
    expect(hidden, 'controls read on save but never rendered').toEqual([]);
  });
});
