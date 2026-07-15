import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

const fullSample = {
  health: 'Ok', ready: true, model: '<img src=x onerror=alert(1)>', uptime_seconds: 60,
  prompt_tokens_per_second: 0.0, generation_tokens_per_second: 0.0,
  running_requests: 0, waiting_requests: 0,
  active_memory_bytes: 1073741824, peak_memory_bytes: 2147483648, cache_memory_bytes: 0,
  global_cache_hit_rate: 0.5, global_cache_entries: 2,
  cache_metrics: { hit_rate: 0.5, entry_count: 2 },
  completed_requests_total: 3, prompt_tokens_total: 40, completion_tokens_total: 20,
};

async function render(page, sample, sequence, failed = false, session = 'rapid-a', sampledAt = Date.now() - 2000) {
  await page.evaluate(async ({ sampleValue, pollSequence, pollFailed, sessionId, sampledAtUnixMs }) => {
    const cards = await import('/js/features/rapid-mlx-cards.js');
    cards.renderRapidMlxCards(sampleValue, pollSequence, pollFailed, sessionId, sampledAtUnixMs);
  }, { sampleValue: sample, pollSequence: sequence, pollFailed: failed, sessionId: session, sampledAtUnixMs: sampledAt });
}

test.describe('Rapid-MLX dashboard card registry', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
    await page.evaluate(() => {
      document.querySelectorAll('.page').forEach(pageElement => {
        pageElement.classList.toggle('active', pageElement.id === 'page-server');
      });
    });
  });

  test('mounts only supported cards and preserves zero values safely', async ({ page }) => {
    await render(page, fullSample, 1);
    await expect(page.locator('[data-card-id]')).toHaveCount(6);
    await expect(page.locator('[data-card-id="throughput"]')).toContainText('0.0 t/s');
    await expect(page.locator('.widget-generation')).toHaveCount(0);
    await expect(page.locator('.widget-context')).toHaveCount(0);
    await expect(page.locator('#m-slot-grid')).toHaveCount(0);
    await expect(page.locator('[data-card-id="runtime"] img')).toHaveCount(0);
    await expect(page.locator('[data-card-id="runtime"]')).toContainText('<img src=x onerror=alert(1)>');
    await render(page, { ...fullSample, backend_details: { progress: { future: true } } }, 2);
    await expect(page.locator('[data-card-id="progress"]')).toHaveCount(0);
    await render(page, { health: 'Ok', ready: true, uptime_seconds: 5 }, 3, false, 'rapid-no-model');
    await expect(page.locator('[data-card-id="runtime"]')).toHaveCount(0);
  });

  test('removes an optional card on the third distinct missing poll', async ({ page }) => {
    await render(page, fullSample, 1);
    const partial = { ...fullSample };
    delete partial.prompt_tokens_per_second;
    delete partial.generation_tokens_per_second;
    await render(page, partial, 2);
    await expect(page.locator('[data-card-id="throughput"] .metric-live-chip')).toContainText(/stale · \d+s ago · 1\/3/);
    await render(page, partial, 2); // repeated websocket push, not a backend poll
    await expect(page.locator('[data-card-id="throughput"] .metric-live-chip')).toContainText(/stale · \d+s ago · 1\/3/);
    await render(page, partial, 3);
    await expect(page.locator('[data-card-id="throughput"] .metric-live-chip')).toContainText(/stale · \d+s ago · 2\/3/);
    await render(page, partial, 4);
    await expect(page.locator('[data-card-id="throughput"]')).toHaveCount(0);
  });

  test('marks failed runtime telemetry degraded and renders accessible progress', async ({ page }) => {
    const withProgress = { ...fullSample, backend_details: { progress: { current: 25, total: 100 } } };
    await render(page, withProgress, 1);
    const progress = page.locator('[data-card-id="progress"] [role="progressbar"]');
    await expect(progress).toHaveAttribute('aria-valuemin', '0');
    await expect(progress).toHaveAttribute('aria-valuemax', '100');
    await expect(progress).toHaveAttribute('aria-valuenow', '25');

    await render(page, withProgress, 2, true);
    const runtime = page.locator('[data-card-id="runtime"]');
    await expect(runtime).toContainText('Telemetry unavailable');
    await expect(runtime).toContainText('degraded');
    await expect(runtime.locator('.metric-live-chip')).toContainText(/stale · \d+s ago · 1\/3/);
    await render(page, withProgress, 3, true);
    await render(page, withProgress, 4, true);
    await expect(runtime).toContainText('Telemetry unavailable');
    await expect(runtime).not.toContainText('live');
  });

  test('resets between sessions and restores llama nodes on backend switch', async ({ page }) => {
    await render(page, fullSample, 1, false, 'rapid-a');
    await render(page, null, 2, false, 'rapid-b');
    const loading = page.locator('[data-telemetry-state="loading"]');
    await expect(loading).toHaveCount(1);
    await expect(loading).toHaveAttribute('role', 'status');
    await page.evaluate(async () => {
      const cards = await import('/js/features/rapid-mlx-cards.js');
      cards.restoreLlamaCards();
    });
    await expect(page.locator('.widget-generation')).toHaveCount(1);
    await expect(page.locator('.widget-context')).toHaveCount(1);
    await expect(page.locator('#rapid-mlx-card-grid')).toHaveCount(0);
  });
});
