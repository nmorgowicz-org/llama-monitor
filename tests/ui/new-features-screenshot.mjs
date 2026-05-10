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
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage', '--disable-cache', '--disable-service-workers'],
  });

  const page = await browser.newPage();
  await page.setCacheEnabled(false);
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

    // ====================================
    // 7. CONTEXT NOTES BUTTON
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

    // ====================================
    // 12. EXPLICIT MODE TOGGLE CYCLING
    // ====================================
    console.log('[NEW FEATURES] Testing explicit mode toggle cycling...');

    // Step 12a: Click toggle → unlocked (level 1, 🔓 badge)
    console.log('[NEW FEATURES] Step 12a: Enabling explicit mode (unlocked)...');
    await page.click('#chat-explicit-toggle-footer');
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/12a-explicit-unlocked.png`, fullPage: false });
    console.log('[NEW FEATURES] Done: 12a-explicit-unlocked.png');

    // Step 12b: Click again → unrestricted (level 2, 🔥 badge)
    console.log('[NEW FEATURES] Step 12b: Enabling unrestricted mode...');
    await page.click('#chat-explicit-toggle-footer');
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/12b-explicit-unrestricted.png`, fullPage: false });
    console.log('[NEW FEATURES] Done: 12b-explicit-unrestricted.png');

    // Step 12c: Click again → locked (level 0, badge gone)
    console.log('[NEW FEATURES] Step 12c: Disabling explicit mode...');
    await page.click('#chat-explicit-toggle-footer');
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/12c-explicit-locked.png`, fullPage: false });
    console.log('[NEW FEATURES] Done: 12c-explicit-locked.png');

    // ====================================
    // 13. SUGGESTIONS TAG CLOUD
    // ====================================
    console.log('[NEW FEATURES] Testing suggestions tag cloud...');

    // Step 13a: Open suggestions dropdown → capture tag cloud
    console.log('[NEW FEATURES] Step 13a: Opening suggestions dropdown for tag cloud...');
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await sleep(1000);
    await page.screenshot({ path: `${OUTPUT_DIR}/13a-suggestions-tag-cloud.png`, fullPage: false });
    console.log('[NEW FEATURES] Done: 13a-suggestions-tag-cloud.png');

    // Step 13b: Type "horror" in search → capture filtered view
    console.log('[NEW FEATURES] Step 13b: Filtering suggestions with "horror"...');
    await page.focus('#suggestion-search-input');
    await page.type('#suggestion-search-input', 'horror');
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/13b-suggestions-search-filter.png`, fullPage: false });
    console.log('[NEW FEATURES] Done: 13b-suggestions-search-filter.png');

    // Close suggestions dropdown
    await page.evaluate(async () => {
      const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
      toggleSuggestionsDropdown();
    });
    await sleep(500);

    console.log('\n[NEW FEATURES] All screenshots saved to docs/screenshots/');

  } catch (err) {
    console.error('[NEW FEATURES] Error:', err.message);
    console.error(err.stack);
  } finally {
    // Clean up test tabs — keep only the first tab, persist to disk
    console.log('[NEW FEATURES] Cleaning up test tabs...');
    await page.evaluate(async () => {
      const { chat } = await import('/js/core/app-state.js');
      const { newChatTab, flushChatPersist } = await import('/js/features/chat-state.js');
      const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');

      if (chat.tabs.length > 1) {
        chat.tabs = [chat.tabs[0]];
        chat.activeTabId = chat.tabs[0].id;
        chat.tabsDirty = true;
        renderChatTabs();
        renderChatMessages();
        flushChatPersist();
      }
    });
    await sleep(1000);

    await browser.close();
    await cleanupServer(server);
    console.log('[NEW FEATURES] Server cleaned up.');
  }
})();
