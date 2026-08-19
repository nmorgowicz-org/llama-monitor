import { test, expect } from '@playwright/test';

// hybrid_cache_entries, prefill_batch_size and completion_batch_size were plumbed all the way
// from RapidMlxConfig to argv but had no control on any surface, so the only value a user could
// ever launch with was the default. Same shape of defect as the throughput fields, same tests:
// stored values must load, edited values must reach the request, and explicit Auto must omit the key.
//
// hybrid_cache_entries carries one extra rule -- it only reaches the runtime while the retained
// cache is on, so the save path drops it when the toggle is off rather than storing a number
// that would never be emitted.
test.describe('Rapid-MLX preset editor cache-entry and batch-size fields', () => {
  const SEED = {
    name: 'cache probe',
    backend: 'rapid_mlx',
    rapid_mlx: {
      port: 8080,
      model_source: '/tmp/Qwen3-8B-4bit',
      prefix_cache_enabled: true,
      retained_cache_mib: 8192,
      hybrid_cache_entries: 32,
      prefill_batch_size: 4,
      completion_batch_size: 8,
    },
  };

  async function openSeeded(page, seed = SEED) {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async (seed) => {
      const { sessionState } = await import('/js/core/app-state.js');
      const mod = await import('/js/features/presets.js');
      sessionState.presets = [{ id: 'probe', ...seed }];
      const sel = document.getElementById('preset-select');
      sel.innerHTML = '<option value="probe">cache probe</option>';
      sel.value = 'probe';
      mod.openPresetModal('edit');
    }, seed);
    await page.locator('#preset-modal .preset-nav-item[data-section="context"]').click();
  }

  async function savePut(page) {
    let body = null;
    await page.route('**/api/presets/**', async (route, request) => {
      if (request.method() === 'PUT') body = request.postDataJSON();
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"id":"probe"}' });
    });
    // Editing an existing preset is a two-step save: the first call renders a change summary
    // and flips the button to "Confirm Save", the second issues the PUT.
    for (const _ of [0, 1]) {
      await page.evaluate(async () => {
        const mod = await import('/js/features/presets.js');
        await mod.savePreset(new Event('submit'));
      });
    }
    return body;
  }

  test('@in-memory-test stored values load into the controls', async ({ page }) => {
    await openSeeded(page);
    const entries = page.locator('#modal-rapid-hybrid-cache-entries');
    await expect(entries).toHaveValue('32');
    await expect(entries.locator('option[value="4"]')).toHaveText('4 — one active history');
    await expect(entries.locator('option[value="8"]')).toHaveText('8 — main + one child');
    await expect(entries.locator('option[value="16"]')).toHaveText('16 — agent workflows (recommended)');
    await expect(page.locator('#modal-rapid-prefill-batch-size')).toHaveValue('4');
    await expect(page.locator('#modal-rapid-completion-batch-size')).toHaveValue('8');
  });

  test('@in-memory-test edited values reach the save request', async ({ page }) => {
    await openSeeded(page);
    await page.selectOption('#modal-rapid-hybrid-cache-entries', '16');
    await page.selectOption('#modal-rapid-prefill-batch-size', '8');

    const body = await savePut(page);
    expect(body, 'save request was never issued').not.toBeNull();
    expect(body.rapid_mlx.hybrid_cache_entries).toBe(16);
    expect(body.rapid_mlx.prefill_batch_size).toBe(8);
    // Untouched control keeps the seeded value rather than reverting to a default.
    expect(body.rapid_mlx.completion_batch_size).toBe(8);
  });

  test('@in-memory-test retained prefix entries are dropped when the cache is off', async ({ page }) => {
    await openSeeded(page);
    await page.selectOption('#modal-rapid-hybrid-cache-entries', '64');
    await page.selectOption('#modal-rapid-cache-memory-mib', '0');

    const body = await savePut(page);
    expect(body, 'save request was never issued').not.toBeNull();
    expect(body.rapid_mlx.prefix_cache_enabled).toBe(false);
    // null, not absent: `out` is spread over the stored rapid_mlx, so leaving the key out
    // would keep the seeded 32 and the argv builder -- which emits the flag on the field
    // alone, without consulting the toggle -- would still pass it to a cache-less server.
    expect(body.rapid_mlx.hybrid_cache_entries).toBeNull();
  });

  test('@in-memory-test new Rapid presets use the measured agent-workflow default', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      mod.openPresetModal('new', null, {
        name: 'bare', backend: 'rapid_mlx',
        rapid_mlx: { port: 8080, model_source: '/tmp/Qwen3-8B-4bit' },
      });
    });
    await page.locator('#preset-modal .preset-nav-item[data-section="context"]').click();

    await expect(page.locator('#modal-rapid-hybrid-cache-entries')).toHaveValue('16');

    let body = null;
    await page.route('**/api/presets', async (route, request) => {
      if (request.method() === 'POST') body = request.postDataJSON();
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"id":"probe"}' });
    });
    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      await mod.savePreset(new Event('submit'));
    });

    expect(body, 'save request was never issued').not.toBeNull();
    expect(body.rapid_mlx.hybrid_cache_entries).toBe(16);
    expect(body.rapid_mlx.prefill_batch_size).toBeNull();
    expect(body.rapid_mlx.completion_batch_size).toBeNull();
  });

  test('@in-memory-test explicit Auto remains a backward-compatible opt-out', async ({ page }) => {
    await openSeeded(page, {
      ...SEED,
      rapid_mlx: { ...SEED.rapid_mlx, hybrid_cache_entries: null },
    });
    await expect(page.locator('#modal-rapid-hybrid-cache-entries')).toHaveValue('0');

    const body = await savePut(page);
    expect(body, 'save request was never issued').not.toBeNull();
    expect(body.rapid_mlx.hybrid_cache_entries).toBeNull();
  });
});
