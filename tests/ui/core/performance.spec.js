// ── Performance Baseline ──────────────────────────────────────────────────────

import { test, expect } from '@playwright/test';

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

    // Assertions to track regression (baseline updated: 38 JS, added Certificates TLS support)
    expect(jsRequests.length).toBeLessThanOrEqual(38);
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
