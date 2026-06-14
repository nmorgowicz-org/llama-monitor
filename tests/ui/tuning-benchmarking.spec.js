import { test, expect } from '@playwright/test';
import { dismissAuthShell } from '../helpers.js';

test.describe('Tuning and Benchmarking', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    await dismissAuthShell(page);
  });

  test('Benchmark flow renders results correctly', async ({ page }) => {
    // Navigate to Server tab
    await page.getByRole('button', { name: /server/i }).click();
    await page.waitForSelector('#view-server');

    // Open Tune panel via the "Tune" button in the header
    const tuneBtn = page.locator('button:has-text("Tune")');
    await expect(tuneBtn).toBeVisible();
    await tuneBtn.click();

    // Verify the "Run Benchmark" button is present
    const runBenchmarkBtn = page.getByRole('button', { name: /run benchmark/i });
    await expect(runBenchmarkBtn).toBeVisible();

    // Mock benchmark response to make it deterministic
    await page.route('**/api/benchmark/run', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          grade: 'Great',
          metrics: { throughput: 120, latency: 45 },
          suggestions: [
            { label: 'Increase context', description: 'Boost context size for better coherence.', param: 'context_size', value: 4096 },
            { label: 'Optimize KV', description: 'Use 4-bit KV quantization.', param: 'kv_cache_type', value: 'fp16' },
          ],
        }),
      });
    });

    // Run benchmark
    await runBenchmarkBtn.click();

    // Check that the results are rendered correctly
    await expect(page.locator('.tune-suggestion-card')).toHaveCount(2);
    await expect(page.locator('.tune-suggestion-card').first()).toContainText('Increase context');
    await expect(page.locator('.tune-suggestion-card').first()).toContainText('Boost context size');
    
    // Verify suggestions have an "Apply" button
    const applyBtn = page.locator('.tune-suggestion-apply');
    await expect(applyBtn).toBeVisible();
  });

  test('MTP Sweep selects model and shows results', async ({ page }) => {
    await page.getByRole('button', { name: /server/i }).click();
    await page.evaluate(() => {
      const tuneBtn = document.querySelector('button:has-text("Tune")');
      if (tuneBtn) tuneBtn.click();
    });

    // Select a model for the sweep
    const sweepRange = page.locator('#mtp-sweep-range');
    await expect(sweepRange).toBeVisible();

    // Mock MTP sweep response
    await page.route('**/api/mtp/sweep', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          results: [
            { param: 'temperature', value: 0.8, metric: 'perplexity' },
            { param: 'top_p', value: 0.9, metric: 'perplexity' },
          ],
        }),
      });
    });

    // Run the sweep
    await page.click('#mtp-sweep-run-btn');
    
    // Verify results table is rendered
    await expect(page.locator('#mtp-sweep-results')).toBeVisible();
  });

  test('Suggestions appear and are interactive', async ({ page }) => {
    await page.getByRole('button', { name: /server/i }).click();
    await page.evaluate(() => {
      const tuneBtn = document.querySelector('button:has-text("Tune")');
      if (tuneBtn) tuneBtn.click();
    });

    // Trigger suggestions by running benchmark (mocked)
    await page.route('**/api/benchmark/run', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          grade: 'Good',
          metrics: { throughput: 100 },
          suggestions: [{ label: 'Test Suggestion', description: 'Test description', param: 'test', value: 1 }],
        }),
      });
    });
    await page.click('button:has-text("Run Benchmark")');

    // Verify suggestion cards
    const card = page.locator('.tune-suggestion-card');
    await expect(card).toBeVisible();
    await expect(card.locator('.tune-suggestion-apply')).toBeVisible();

    // Interact with Apply button
    await card.locator('.tune-suggestion-apply').click();
    // Verify it's gone or marked as applied
    await expect(card).not.toBeVisible();
  });
});
