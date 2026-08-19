import { test, expect } from '@playwright/test';

// These five RapidMlxConfig fields were accepted by the backend and documented in the plan
// docs, but had no control anywhere in the UI, so no user could ever set them. The point of
// these tests is that the controls reach buildSpawnPayload -- a rendered <select> that never
// arrives in the payload is the same defect in a new coat.
test.describe('Rapid-MLX Phase 7 throughput fields', () => {
  async function openRapidHardware(page) {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.evaluate(async () => {
      const { openSpawnWizard, wizardState } = await import('/js/features/spawn-wizard.js');
      openSpawnWizard();
      wizardState.engine.selected = 'rapid_mlx';
      wizardState.model.source = 'local';
      wizardState.model.path = '/tmp/Qwen3-8B-4bit';
    });
  }

  test('@in-memory-test each control reaches the launch payload', async ({ page }) => {
    await openRapidHardware(page);

    const payload = await page.evaluate(async () => {
      const { buildSpawnPayload } = await import('/js/features/spawn-wizard.js');
      const set = (id, value) => {
        const el = document.getElementById(id);
        el.value = value;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      };
      set('spawn-rapid-gpu-memory-utilization', '0.85');
      set('spawn-rapid-max-num-seqs', '8');
      set('spawn-rapid-max-concurrent-requests', '32');
      set('spawn-rapid-pflash-policy', 'auto');
      return buildSpawnPayload().rapid_mlx;
    });

    expect(payload.gpu_memory_utilization).toBe(0.85);
    expect(payload.max_num_seqs).toBe(8);
    expect(payload.max_concurrent_requests).toBe(32);
    expect(payload.pflash_policy).toBe('auto');
  });

  test('@in-memory-test "Auto" omits the key rather than sending a default', async ({ page }) => {
    await openRapidHardware(page);

    const payload = await page.evaluate(async () => {
      const { buildSpawnPayload } = await import('/js/features/spawn-wizard.js');
      return buildSpawnPayload().rapid_mlx;
    });

    // An absent flag and an explicit runtime default are different states; sending a value
    // for a field the user left on Auto would pin behaviour they never asked to pin.
    expect(payload).not.toHaveProperty('gpu_memory_utilization');
    expect(payload).not.toHaveProperty('max_num_seqs');
    expect(payload).not.toHaveProperty('max_concurrent_requests');
  });

  // Only one exclusion rule remains. The second paired speculative_policy with max_num_seqs;
  // it was built on --speculative, which no rapid-mlx release has, and went with the field.
  test('@in-memory-test the mutual-exclusion rule is surfaced', async ({ page }) => {
    await openRapidHardware(page);

    const set = async (id, value) => {
      await page.evaluate(({ id, value }) => {
        const el = document.getElementById(id);
        el.value = value;
        el.dispatchEvent(new Event('change', { bubbles: true }));
      }, { id, value });
    };

    // Rule 1: pflash_policy=on bypasses TurboQuant.
    await set('spawn-rapid-pflash-policy', 'on');
    await set('spawn-turboquant-mode', 'k8v4');
    await expect(page.locator('#spawn-rapid-exclusion-warning')).toContainText('PFlash bypasses TurboQuant');

    // Clearing one side clears the warning.
    await set('spawn-turboquant-mode', 'none');
    await expect(page.locator('#spawn-rapid-exclusion-warning')).toHaveCount(0);
  });
});
