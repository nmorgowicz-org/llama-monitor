/**
 * Sparkline validation screenshot capture
 *
 * Spawns a headless llama-monitor instance, attaches to a remote server,
 * and captures screenshots of sparkline visualizations for design review.
 *
 * Usage:
 *   cd tests/ui
 *   node sparkline-screenshot.mjs
 *
 * Environment:
 *   REMOTE_SERVER  — URL of remote llama.cpp server (default: http://192.168.2.16:8001)
 *
 * Output:
 *   docs/screenshots/artifacts/sparkline-validate-*.png
 *   (artifacts/ is gitignored; only curated screenshots go in docs/screenshots/)
 *
 * Prerequisites:
 *   - Release binary built (cargo build --release)
 *   - Remote server running and accessible
 *   - puppeteer installed (npm install in tests/ui/)
 */
import puppeteer from 'puppeteer';
import { spawn } from 'child_process';
import net from 'net';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import fs from 'fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const OUTPUT_DIR = join(__dirname, '../../docs/screenshots/artifacts');
const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

async function findPort(start = 8900) {
  for (let p = start; p < start + 200; p++) {
    const ok = await new Promise(r => {
      const s = net.createServer();
      s.unref();
      s.on('error', () => r(false));
      s.listen(p, '127.0.0.1', () => s.close(() => r(true)));
    });
    if (ok) return p;
  }
  throw new Error('no port');
}

async function waitForHttp(url, ms = 30000) {
  const t = Date.now();
  while (Date.now() - t < ms) {
    try { if ((await fetch(url)).ok) return; } catch {}
    await sleep(250);
  }
  throw new Error(`timeout: ${url}`);
}

async function spawnServer(port) {
  const bin = join(__dirname, '../../target/release/llama-monitor');
  const proc = spawn(bin, ['--port', String(port), '--headless'], { stdio: ['ignore', 'pipe', 'pipe'] });
  proc.stdout.on('data', d => console.log(`[server] ${d}`));
  proc.stderr.on('data', d => console.log(`[server err] ${d}`));
  await waitForHttp(`http://127.0.0.1:${port}`);
  return { proc, url: `http://127.0.0.1:${port}` };
}

async function captureSparklineClips(page, selector) {
  const rects = await page.$$eval(selector, els => els.map((el, index) => {
    const rect = el.getBoundingClientRect();
    return {
      index,
      x: rect.x + window.scrollX,
      y: rect.y + window.scrollY,
      width: rect.width,
      height: rect.height
    };
  }).filter(rect => rect.width > 0 && rect.height > 0));

  console.log(`Found ${rects.length} sparkline SVGs`);
  for (const rect of rects) {
    await page.screenshot({
      path: `${OUTPUT_DIR}/sparkline-validate-svg-${rect.index}.png`,
      clip: {
        x: Math.max(0, rect.x),
        y: Math.max(0, rect.y),
        width: Math.max(1, rect.width),
        height: Math.max(1, rect.height)
      }
    });
  }
  console.log(`Captured ${rects.length} individual sparkline SVGs`);
}

(async () => {
  const port = await findPort();
  console.log(`Port: ${port}`);
  const server = await spawnServer(port);
  const browser = await puppeteer.launch({ headless: 'new', args: ['--no-sandbox', '--disable-setuid-sandbox'] });
  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: 1 });

  try {
    // Go to app and attach to remote
    await page.goto(server.url, { waitUntil: 'networkidle0' });
    await page.waitForSelector('#setup-endpoint-url', { visible: true });

    // Attach to remote server
    await page.$eval('#setup-endpoint-url', (el, url) => {
      el.value = url;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, REMOTE_SERVER);
    await sleep(200);
    await page.click('#setup-attach-btn');

    // Wait for monitor view
    await page.waitForFunction(() => {
      const s = document.getElementById('view-setup');
      const m = document.getElementById('view-monitor');
      return s && m && getComputedStyle(s).display === 'none' && getComputedStyle(m).display !== 'none';
    }, { timeout: 30000 });

    // Wait for metrics to populate
    console.log('Waiting for metrics to populate...');
    await sleep(4000);

    // Capture full dashboard
    await page.screenshot({ path: `${OUTPUT_DIR}/sparkline-validate-full.png`, fullPage: true });
    console.log('Captured: sparkline-validate-full.png');

    // Scroll to GPU section and capture
    await page.evaluate(() => window.scrollTo(0, 400));
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/sparkline-validate-gpu-section.png`, fullPage: true });
    console.log('Captured: sparkline-validate-gpu-section.png');

    // Scroll to System section and capture
    await page.evaluate(() => window.scrollTo(0, 800));
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/sparkline-validate-system-section.png`, fullPage: true });
    console.log('Captured: sparkline-validate-system-section.png');

    // Capture all sparkline SVGs for close inspection
    const sparklineSelector = 'svg.metric-sparkline, svg.hw-sparkline, svg.hw-metric-sparkline, svg.hw-clock-footer-spark';
    await captureSparklineClips(page, sparklineSelector);

    console.log('Done — check docs/screenshots/ for sparkline-validate-*.png');
  } catch (err) {
    console.error(err);
    process.exitCode = 1;
  } finally {
    await browser.close();
    server.proc.kill('SIGTERM');
    await sleep(500);
    server.proc.kill('SIGKILL');
  }
})();
