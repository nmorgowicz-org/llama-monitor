import puppeteer from 'puppeteer';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { spawn } from 'child_process';
import net from 'net';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const OUTPUT_DIR = join(__dirname, '../../docs/screenshots/artifacts');
const SCREENSHOT_PORT = parseInt(process.env.SCREENSHOT_PORT || '8892', 10);

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

async function findAvailablePort(startPort = SCREENSHOT_PORT) {
    for (let port = startPort; port < startPort + 200; port++) {
        const available = await new Promise(resolve => {
            const server = net.createServer();
            server.unref();
            server.on('error', () => resolve(false));
            server.listen(port, '127.0.0.1', () => {
                server.close(() => resolve(true));
            });
        });
        if (available) return port;
    }
    throw new Error(`No available port found starting at ${startPort}`);
}

async function waitForHttp(url, timeout = 30000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
        try {
            const response = await fetch(url, { method: 'GET' });
            if (response.ok) return;
        } catch (_) {
            // keep polling
        }
        await sleep(250);
    }
    throw new Error(`Server did not become ready at ${url} within ${timeout}ms`);
}

async function spawnLlamaMonitor(port) {
    const binaryPath = join(__dirname, '../../target/release/llama-monitor');
    const proc = spawn(binaryPath, ['--port', String(port), '--headless'], {
        stdio: ['ignore', 'pipe', 'pipe'],
    });

    proc.stdout.on('data', data => {
        const output = data.toString().trim();
        if (output) console.log(`[llama-monitor] ${output}`);
    });

    proc.stderr.on('data', data => {
        const output = data.toString().trim();
        if (output) console.log(`[llama-monitor] ${output}`);
    });

    proc.on('error', err => {
        console.error(`Failed to spawn llama-monitor: ${err.message}`);
    });

    const url = `http://127.0.0.1:${port}`;
    await waitForHttp(url);
    return { proc, url };
}

async function cleanupServer(server) {
    if (!server?.proc) return;
    server.proc.kill('SIGTERM');
    await sleep(750);
    if (server.proc.exitCode === null) {
        server.proc.kill('SIGKILL');
    }
}

(async () => {
  console.log('[NEW FEATURES] Finding available port...');
  const port = await findAvailablePort();
  console.log(`[NEW FEATURES] Spawning llama-monitor on port ${port}...`);
  const server = await spawnLlamaMonitor(port);
  const BASE_URL = server.url;

  console.log('[NEW FEATURES] Launching browser...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  try {
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });
    await sleep(3000);

    // Attach to remote server for chat functionality
    console.log('[NEW FEATURES] Attaching to remote server at http://192.168.2.16:8001...');
    await page.evaluate((url) => {
      const input = document.getElementById('setup-endpoint-url');
      if (input) {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }, 'http://192.168.2.16:8001');
    await sleep(500);
    await page.evaluate(() => document.getElementById('btn-attach')?.click());
    await sleep(8000);

   // Enable guided generation features
    console.log('[NEW FEATURES] Enabling guided generation features...');
    await page.evaluate(() => {
      const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
      settings.enabled_context_notes = true;
      settings.enabled_suggestions = true;
      settings.enabled_quick_guide = true;
      localStorage.setItem('llama_monitor_settings', JSON.stringify(settings));
    });
    await sleep(500);

    // Switch to chat tab
    console.log('[NEW FEATURES] Switching to chat tab...');
    await page.click('button[data-tab="chat"]');
    await sleep(2000);

    // Create multiple tabs
    console.log('[NEW FEATURES] Creating multiple tabs...');
    await page.evaluate(() => {
      const addBtn = document.querySelector('.chat-tab-add');
      if (addBtn) { addBtn.click(); addBtn.click(); addBtn.click(); }
    });
    await sleep(1000);

    // Capture tabs
    await page.screenshot({ path: `${OUTPUT_DIR}/06-chat-tabs.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 06-chat-tabs.png');

    // Capture persona strip
    console.log('[NEW FEATURES] Capturing persona strip...');
    await page.evaluate(() => {
      const personaStrip = document.querySelector('.chat-persona-strip');
      if (personaStrip) {
        personaStrip.scrollIntoView({ behavior: 'instant', block: 'start' });
      }
    });
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/07-persona-strip.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 07-persona-strip.png');

    // ====================================
    // 8. CONTEXT NOTES BUTTON
    // ====================================
    console.log('[NEW FEATURES] Testing Context Notes button...');
    await page.evaluate(async () => {
      const { toggleContextSidebar } = await import('./js/features/chat-notes.js');
      toggleContextSidebar();
    });
    await sleep(1000);
    await page.screenshot({ path: `${OUTPUT_DIR}/08-context-notes-expanded.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 08-context-notes-expanded.png');

    // Close context notes
    await page.evaluate(async () => {
      const { toggleContextSidebar } = await import('./js/features/chat-notes.js');
      toggleContextSidebar();
    });
    await sleep(500);

    // ====================================
    // 9. SUGGESTIONS BUTTON
    // ====================================
    console.log('[NEW FEATURES] Testing Suggestions button...');
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await sleep(1000);
    await page.screenshot({ path: `${OUTPUT_DIR}/09-suggestions-dropdown.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 09-suggestions-dropdown.png');

    // Close suggestions
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await sleep(500);

    // ====================================
    // 10. QUICK GUIDE BUTTON
    // ====================================
    console.log('[NEW FEATURES] Testing Quick Guide button...');
    await page.evaluate(async () => {
      const { toggleQuickGuide } = await import('./js/features/chat-quick-guide.js');
      toggleQuickGuide();
    });
    await sleep(1000);
    await page.screenshot({ path: `${OUTPUT_DIR}/10-quick-guide-dropdown.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 10-quick-guide-dropdown.png');

    // Close quick guide
    await page.evaluate(async () => {
      const { toggleQuickGuide } = await import('./js/features/chat-quick-guide.js');
      toggleQuickGuide();
    });
    await sleep(500);

    // ====================================
    // 11. ALL BUTTONS VISIBLE
    // ====================================
    console.log('[NEW FEATURES] Capturing all buttons visible...');
    await page.evaluate(() => {
      const inputRow = document.getElementById('chat-input-row');
      if (inputRow) inputRow.scrollIntoView({ behavior: 'instant', block: 'end' });
    });
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/11-chat-input-buttons.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 11-chat-input-buttons.png');

    console.log('\n[NEW FEATURES] All screenshots saved to docs/screenshots/');

  } catch (err) {
    console.error('[NEW FEATURES] Error:', err.message);
    console.error(err.stack);
  } finally {
    await browser.close();
    await cleanupServer(server);
    console.log('[NEW FEATURES] Server cleaned up.');
  }
})();
