// ── Performance Baseline ──────────────────────────────────────────────────────

import { test, expect } from '@playwright/test';
import { readFileSync, writeFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { join, dirname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const BASELINE_FILE = join(__dirname, 'js-module-baseline.json');

function readBaseline() {
  return JSON.parse(readFileSync(BASELINE_FILE, 'utf8'));
}

test.describe('performance baseline', () => {
  test('cold load network and timing', async ({ page }) => {
    const requests = [];
    page.on('request', req => {
      const url = req.url();
      if (url.endsWith('.js') || url.endsWith('.css')) {
        requests.push({ url: url.replace('http://127.0.0.1:7778', ''), method: req.method() });
      }
    });

    const startTime = Date.now();
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');
    const modulesReadyTime = Date.now() - startTime;

    const jsRequests = requests.filter(r => r.url.endsWith('.js'));
    const cssRequests = requests.filter(r => r.url.endsWith('.css'));

    console.log('=== Performance Baseline ===');
    console.log(`Time to modules-ready: ${modulesReadyTime}ms`);
    console.log(`JS requests: ${jsRequests.length}`);
    console.log(`CSS requests: ${cssRequests.length}`);
    console.log(`Total asset requests: ${requests.length}`);
    console.log('JS files loaded:');
    jsRequests.forEach(r => console.log(`  ${r.url}`));

    const baseline = readBaseline();
    expect(
      jsRequests.length,
      `JS module count regressed: ${jsRequests.length} > ${baseline.count}. ` +
      `Run \`cd tests/ui && npm run update-baseline\` after verifying the new modules are intentional.`,
    ).toBeLessThanOrEqual(baseline.count);
    expect(modulesReadyTime).toBeLessThan(5000); // should be under 5s locally
  });

  test('dashboard update path timing', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('html.modules-ready');

    const updateMs = await page.evaluate(() => {
      const start = performance.now();
      if (typeof window.updateDashboard === 'function') {
        window.updateDashboard({});
      }
      return performance.now() - start;
    });

    console.log(`Dashboard update time: ${Math.round(updateMs)}ms`);
    expect(updateMs).toBeLessThan(500); // should be under 500ms
  });
});
