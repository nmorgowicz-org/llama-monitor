import { test, expect } from '@playwright/test';

// The spawn wizard gained five Rapid-MLX throughput controls; the preset editor did not.
// A preset that cannot express a field does not merely omit it -- the editor rebuilds the
// rapid_mlx object on every save, so it writes its own defaults over whatever the wizard
// set. These tests cover both directions: existing values must load into the controls, and
// edited values must reach the request body.
test.describe('Rapid-MLX preset editor throughput fields', () => {
  const SEED = {
    name: 'throughput probe',
    backend: 'rapid_mlx',
    rapid_mlx: {
      port: 8080,
      model_source: '/tmp/Qwen3-8B-4bit',
      gpu_memory_utilization: 0.85,
      max_num_seqs: 8,
      max_concurrent_requests: 32,
      pflash_policy: 'auto',
      speculative_policy: 'on',
    },
  };

  // Edit mode, not seeded-new: only the edit path loads stored values into the controls,
  // and editing an existing preset is the path where a dropped field overwrites real data.
  async function openSeeded(page) {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async (seed) => {
      const { sessionState } = await import('/js/core/app-state.js');
      const mod = await import('/js/features/presets.js');
      sessionState.presets = [{ id: 'probe', ...seed }];
      const sel = document.getElementById('preset-select');
      sel.innerHTML = '<option value="probe">throughput probe</option>';
      sel.value = 'probe';
      mod.openPresetModal('edit');
    }, SEED);
    await page.locator('#preset-modal .preset-nav-item[data-section="advanced"]').click();
  }

  test('@in-memory-test stored values load into the controls', async ({ page }) => {
    await openSeeded(page);
    await expect(page.locator('#modal-rapid-gpu-memory-utilization')).toHaveValue('0.85');
    await expect(page.locator('#modal-rapid-max-num-seqs')).toHaveValue('8');
    await expect(page.locator('#modal-rapid-max-concurrent-requests')).toHaveValue('32');
    await expect(page.locator('#modal-rapid-pflash-policy')).toHaveValue('auto');
    await expect(page.locator('#modal-rapid-speculative-policy')).toHaveValue('on');
  });

  test('@in-memory-test edited values reach the save request', async ({ page }) => {
    await openSeeded(page);

    let body = null;
    await page.route('**/api/presets/**', async (route, request) => {
      if (request.method() === 'PUT') body = request.postDataJSON();
      await route.fulfill({ status: 200, contentType: 'application/json', body: '{"id":"probe"}' });
    });

    await page.selectOption('#modal-rapid-gpu-memory-utilization', '0.95');
    await page.selectOption('#modal-rapid-max-num-seqs', '4');
    await page.selectOption('#modal-rapid-pflash-policy', 'on');
    // Editing an existing preset is a two-step save: the first call renders a change
    // summary and flips the button to "Confirm Save", the second issues the PUT.
    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      await mod.savePreset(new Event('submit'));
    });
    await expect(page.locator('#preset-change-summary')).toBeVisible();
    await expect(page.locator('#preset-change-summary-list')).toContainText('0.95');
    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      await mod.savePreset(new Event('submit'));
    });

    expect(body, 'save request was never issued').not.toBeNull();
    expect(body.rapid_mlx.gpu_memory_utilization).toBe(0.95);
    expect(body.rapid_mlx.max_num_seqs).toBe(4);
    expect(body.rapid_mlx.pflash_policy).toBe('on');
    // Untouched controls keep the seeded values rather than reverting to a default.
    expect(body.rapid_mlx.max_concurrent_requests).toBe(32);
    expect(body.rapid_mlx.speculative_policy).toBe('on');
  });

  test('@in-memory-test Auto omits the key rather than pinning a default', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async () => {
      const mod = await import('/js/features/presets.js');
      mod.openPresetModal('new', null, {
        name: 'bare', backend: 'rapid_mlx',
        rapid_mlx: { port: 8080, model_source: '/tmp/Qwen3-8B-4bit' },
      });
    });
    await page.locator('#preset-modal .preset-nav-item[data-section="advanced"]').click();

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
    expect(body.rapid_mlx).not.toHaveProperty('gpu_memory_utilization');
    expect(body.rapid_mlx).not.toHaveProperty('max_num_seqs');
    expect(body.rapid_mlx).not.toHaveProperty('max_concurrent_requests');
    expect(body.rapid_mlx).not.toHaveProperty('speculative_policy');
  });
});
