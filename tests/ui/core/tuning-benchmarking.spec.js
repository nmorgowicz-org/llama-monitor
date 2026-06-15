import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

async function switchToMonitor(page) {
  await dismissAuthShell(page);
  await page.evaluate(async () => {
    const { switchView } = await import('/js/features/setup-view.js');
    switchView('monitor');
  });
}

test.describe('Tuning and Benchmarking', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await switchToMonitor(page);
    // Benchmark pill is normally shown only when a server is connected.
    // Call the module's showTunePanel() so the pill and its parent group are
    // both made visible, matching exactly what the app does on server connect.
    await page.evaluate(async () => {
      const { showTunePanel } = await import('/js/features/tune-panel.js');
      showTunePanel();
    });
  });

  test('Benchmark flow renders results correctly', async ({ page }) => {
    const pill = page.locator('#benchmark-pill');
    await expect(pill).toBeVisible();
    await pill.click();

    // Run button text is "Run" (not "Run Benchmark")
    const runBtn = page.locator('#tune-run-btn');
    await expect(runBtn).toBeVisible();

    // Mock /api/benchmark with the shape the backend actually returns.
    // (not /api/benchmark/run — that path does not exist)
    await page.route('**/api/benchmark', async (route) => {
      if (route.request().method() !== 'POST') { await route.continue(); return; }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          gen_tokens_per_second: 15,
          prompt_tokens_per_second: 80,
          time_to_first_token_ms: 250,
          suggestions: [
            { label: 'Increase context', description: 'Boost context size for better coherence.', param: 'context_size', value: 4096 },
            { label: 'Optimize KV', description: 'Use 4-bit KV quantization.', param: 'kv_cache_type', value: 'fp16' },
          ],
        }),
      });
    });

    await runBtn.click();

    await expect(page.locator('.tune-suggestion-card')).toHaveCount(2);
    await expect(page.locator('.tune-suggestion-card').first()).toContainText('Increase context');
    await expect(page.locator('.tune-suggestion-card').first()).toContainText('Boost context size');
    await expect(page.locator('.tune-suggestion-apply').first()).toBeVisible();
  });

  test('MTP Sweep shows results when enabled', async ({ page }) => {
    // MTP sweep card is hidden until an MTP-capable config is active (setTuneConfig).
    // Force it visible for isolated testing.
    await page.evaluate(() => {
      const card = document.getElementById('mtp-sweep-card');
      if (card) card.style.display = '';
    });

    await page.locator('#benchmark-pill').click();

    const sweepRange = page.locator('#mtp-sweep-range');
    await expect(sweepRange).toBeVisible();

    // Mock /api/bench/mtp-sweep with the shape the backend actually returns.
    // (not /api/mtp/sweep — that path does not exist)
    await page.route('**/api/bench/mtp-sweep', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          probes: [
            { n_max: 1, gen_tps: 12.5, ttft_ms: 200 },
            { n_max: 2, gen_tps: 15.3, ttft_ms: 220 },
          ],
          recommended_n_max: 2,
        }),
      });
    });

    await page.click('#mtp-sweep-run-btn');

    await expect(page.locator('#mtp-sweep-results')).toBeVisible();
  });

  test('Suggestions appear; Apply with no active server shows error toast', async ({ page }) => {
    await page.locator('#benchmark-pill').click();

    await page.route('**/api/benchmark', async (route) => {
      if (route.request().method() !== 'POST') { await route.continue(); return; }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          gen_tokens_per_second: 10,
          suggestions: [{ label: 'Test Suggestion', description: 'Test description', param: 'test', value: 1 }],
        }),
      });
    });

    await page.locator('#tune-run-btn').click();

    const card = page.locator('.tune-suggestion-card');
    await expect(card).toBeVisible();
    await expect(card.locator('.tune-suggestion-apply')).toBeVisible();

    // Without an active server config (_tuneConfig is null until a server is
    // launched via spawn/start), Apply shows an error toast rather than restarting.
    await card.locator('.tune-suggestion-apply').click();
    await expect(page.locator('.toast.toast-error')).toBeVisible();
  });
});
