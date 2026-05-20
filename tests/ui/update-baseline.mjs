/**
 * update-baseline.mjs — re-measures the JS module count by doing a headless
 * cold load against the running test server and writes the new count to
 * core/js-module-baseline.json.
 *
 * Usage:  cd tests/ui && npm run update-baseline
 *
 * The test server must already be running on http://127.0.0.1:7778, OR
 * LLAMA_MONITOR_UI_URL can point to another instance.
 */

import { chromium } from '@playwright/test';
import { writeFileSync, readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { join, dirname } from 'path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const BASELINE_FILE = join(__dirname, 'core/js-module-baseline.json');
const BASE_URL = process.env.LLAMA_MONITOR_UI_URL || 'http://127.0.0.1:7778';

export default async function updateBaseline() {
  const browser = await chromium.launch();
  const context = await browser.newContext();
  const page = await context.newPage();

  const jsFiles = [];
  page.on('request', req => {
    const url = req.url();
    if (url.endsWith('.js')) {
      jsFiles.push(url.replace(BASE_URL, ''));
    }
  });

  try {
    await page.goto(BASE_URL, { waitUntil: 'load' });
    await page.waitForSelector('html.modules-ready', { timeout: 15000 });
  } finally {
    await browser.close();
  }

  const existing = JSON.parse(readFileSync(BASELINE_FILE, 'utf8'));
  const newCount = jsFiles.length;

  if (newCount === existing.count) {
    console.log(`Baseline unchanged: ${newCount} JS modules`);
    return;
  }

  console.log(`Baseline updated: ${existing.count} → ${newCount} JS modules`);
  jsFiles.forEach(f => console.log(`  ${f}`));

  writeFileSync(BASELINE_FILE, JSON.stringify(
    { count: newCount, note: existing.note },
    null, 2,
  ) + '\n');
  console.log(`Written to ${BASELINE_FILE}`);
}

// Run directly
updateBaseline().catch(err => { console.error(err); process.exit(1); });
