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
  test('@in-memory-test MLX information architecture and fit strip stay coherent', async ({ page }) => {
    const estimateBodies = [];
    await page.route('**/api/vram-estimate', (route, request) => {
      estimateBodies.push(request.postDataJSON());
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          total_bytes: 16 * 1024 ** 3,
          weights_bytes: 12 * 1024 ** 3,
          active_kv_bytes: 2 * 1024 ** 3,
          retained_kv_bytes: 1024 ** 3,
          overhead_bytes: 1024 ** 3,
          available_bytes: 24 * 1024 ** 3,
          recommendation: 'comfortable',
        }),
      });
    });
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async () => {
      const { sessionState } = await import('/js/core/app-state.js');
      const mod = await import('/js/features/presets.js');
      sessionState.presets = [{
        id: 'mlx-ia',
        name: 'MLX IA',
        backend: 'rapid_mlx',
        rapid_mlx: {
          model_source: { kind: 'mlx_directory', path: '/tmp/Qwen3-8B-4bit' },
          port: 8001,
        },
      }];
      const select = document.getElementById('preset-select');
      select.innerHTML = '<option value="mlx-ia">MLX IA</option>';
      select.value = 'mlx-ia';
      mod.openPresetModal('edit');
    });

    const visibleLabels = await page.locator('#preset-modal .preset-nav-item:visible .pni-label')
      .allTextContents();
    expect(visibleLabels).toEqual(['Model & Fit', 'Generation', 'Cache & Performance', 'Server & Safety']);
    await expect(page.locator('#preset-vram-strip')).toBeVisible();

    await page.locator('#preset-modal .preset-nav-item[data-section="generation"]').click();
    for (const id of ['modal-temperature', 'modal-top-p', 'modal-top-k', 'modal-min-p',
      'modal-repeat-penalty', 'modal-presence-penalty']) {
      await expect(page.locator(`#${id}`)).toBeVisible();
    }
    await expect(page.locator('.mlx-shared-sampling-row')).toHaveCount(2);
    await expect(page.locator('#modal-enable-thinking')).toBeHidden();
    await expect(page.locator('#pe-row-rapid-reasoning')).toBeVisible();
    await expect(page.locator('[data-mlx-group="protocol"]')).toBeVisible();

    await page.locator('#preset-modal .preset-nav-item[data-section="context"]').click();
    await expect(page.locator('#pe-row-rapid-advanced')).toBeVisible();
    await expect(page.locator('#pe-row-rapid-hybrid-cache-entries')).toBeVisible();
    await page.selectOption('#modal-rapid-kv-cache-dtype', 'int4');
    await page.selectOption('#modal-rapid-cache-memory-mib', '16384');
    await page.selectOption('#modal-rapid-prefill-step-size', '1024');
    await expect.poll(() => estimateBodies.length).toBeGreaterThan(1);
    const latestEstimate = estimateBodies.at(-1);
    expect(latestEstimate.kv_cache_dtype).toBe('int4');
    expect(latestEstimate.retained_cache_mib).toBe(16384);
    expect(latestEstimate.prefill_step_size).toBe(1024);

    await page.locator('#preset-modal .preset-nav-item[data-section="advanced"]').click();
    await page.locator('[data-mlx-group="companions"]').evaluate(detail => { detail.open = true; });
    await expect(page.locator('#pe-row-rapid-speculative')).toBeVisible();

    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      mod.openPresetModal('new', null, { name: 'llama', backend: 'llama_cpp' });
    });
    await page.locator('#preset-modal .preset-nav-item[data-section="generation"]').click();
    await expect(page.locator('.mlx-shared-sampling-row')).toHaveCount(0);
    await expect(page.locator('.mlx-generation-llama-only')).toHaveCount(0);
    await expect(page.locator('#modal-enable-thinking')).toBeVisible();
  });

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

    // Trust consent is conditionally visible (only after speculative enabled + companion
    // model entered + preflight says trust required), so it's always hidden for this test.
    const conditionalIds = new Set(['modal-rapid-speculative-trust-consent']);

    const hidden = [];
    for (const id of ids) {
      // A referenced id with no element is a separate defect, out of scope here.
      if (conditionalIds.has(id)) continue;
      const el = page.locator(`#${id}`);
      if (await el.count() === 0) continue;
      let seen = false;
      for (const section of sections) {
        await page.locator(`#preset-modal .preset-nav-item[data-section="${section}"]`).click();
        await page.locator('#preset-modal details.mlx-native-group').evaluateAll(
          details => details.forEach(detail => { detail.open = true; }),
        );
        if (await el.isVisible()) { seen = true; break; }
      }
      if (!seen) hidden.push(id);
    }
    expect(hidden, 'controls read on save but never rendered').toEqual([]);

    await page.locator('#preset-modal .preset-nav-item[data-section="advanced"]').click();
    await page.locator('[data-mlx-group="companions"]').evaluate(detail => { detail.open = true; });
    await expect(page.locator('#pe-row-rapid-speculative')).toBeVisible();
    await page.locator('#modal-rapid-speculative-enabled').check();
    await expect(page.locator('#modal-rapid-speculative-source')).toBeVisible();
  });
});
