import puppeteer from 'puppeteer';
import fs from 'fs';
import { spawn } from 'child_process';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://127.0.0.1:7778';
const OUTPUT_DIR = '../../docs/screenshots';
const SERVER_LOG = '/tmp/llama-monitor-ui-review.log';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

// Start server
const startServer = () => {
  return new Promise((resolve, reject) => {
    const logFile = fs.openSync(SERVER_LOG, 'w');
    const server = spawn('./target/release/llama-monitor', ['--headless', '--port', '7778'], {
      cwd: '../../',
      stdio: ['ignore', logFile, logFile],
      detached: false
    });

    server.unref();
    
    // Give server time to start
    setTimeout(() => {
      console.log('[UI REVIEW] Server started on port 7778');
      resolve(server);
    }, 8000);
  });
};

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

(async () => {
  try {
    console.log('[FULL UI REVIEW] Starting server...');
    const server = await startServer();

    console.log('[FULL UI REVIEW] Launching browser...');
    const browser = await puppeteer.launch({
      headless: 'new',
      args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
    });

    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900 });

    try {
      await page.goto(BASE_URL, { waitUntil: 'networkidle0', timeout: 30000 });
      await sleep(3000);

      // ====================================
      // 1. WELCOME/SETUP SCREEN
      // ====================================
      console.log('[FULL UI REVIEW] Capturing setup screen...');
      await page.screenshot({ path: `${OUTPUT_DIR}/full-01-setup-screen.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-01-setup-screen.png');

      // ====================================
      // 2. TOP STATUS BAR (Setup mode)
      // ====================================
      console.log('[FULL UI REVIEW] Capturing top status bar...');
      await page.evaluate(() => {
        const status = document.getElementById('endpoint-url-display');
        if (status) status.scrollIntoView({ behavior: 'instant', block: 'start' });
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-02-top-status-bar.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-02-top-status-bar.png');

      // ====================================
      // 3. LEFT NAV BAR
      // ====================================
      console.log('[FULL UI REVIEW] Capturing left nav bar...');
      await page.evaluate(() => {
        const sidebar = document.querySelector('.sidebar-nav');
        if (sidebar) sidebar.scrollIntoView({ behavior: 'instant', block: 'start' });
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-03-left-nav-bar.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-03-left-nav-bar.png');

      // ====================================
      // 4. ATTACH TO REMOTE SERVER
      // ====================================
      console.log('[FULL UI REVIEW] Attaching to remote server...');
      await page.evaluate((url) => {
        const input = document.getElementById('setup-endpoint-url');
        if (input) {
          input.value = url;
          input.dispatchEvent(new Event('input', { bubbles: true }));
        }
      }, 'http://192.168.2.16:8001');
      await sleep(500);
      await page.evaluate(() => document.querySelector('button[onclick="doAttachFromSetup()"]')?.click());
      await sleep(8000);

      // ====================================
      // 5. TOP STATUS BAR (After attach)
      // ====================================
      console.log('[FULL UI REVIEW] Capturing top status bar (after attach)...');
      await page.evaluate(() => {
        const status = document.getElementById('endpoint-url-display');
        if (status) status.scrollIntoView({ behavior: 'instant', block: 'start' });
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-04-top-status-after-attach.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-04-top-status-after-attach.png');

      // ====================================
      // 6. DASHBOARD (Server Tab) - Main View
      // ====================================
      console.log('[FULL UI REVIEW] Switching to Server tab...');
      await page.click('button[onclick="switchTab(\'server\')"]');
      await sleep(3000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-05-server-tab-dashboard.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-05-server-tab-dashboard.png');

      // ====================================
      // 7. GPU METRICS SECTION
      // ====================================
      console.log('[FULL UI REVIEW] Capturing GPU metrics section...');
      await page.evaluate(() => {
        const gpuSection = document.getElementById('gpu-section');
        if (gpuSection) gpuSection.scrollIntoView({ behavior: 'instant', block: 'start' });
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-06-gpu-metrics.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-06-gpu-metrics.png');

      // ====================================
      // 8. INFERENCE DASHBOARD
      // ====================================
      console.log('[FULL UI REVIEW] Capturing inference dashboard...');
      await page.evaluate(() => {
        const inferenceGrid = document.querySelector('.inference-grid');
        if (inferenceGrid) inferenceGrid.scrollIntoView({ behavior: 'instant', block: 'start' });
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-07-inference-dashboard.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-07-inference-dashboard.png');

      // ====================================
      // 9. CHAT TAB - Default State
      // ====================================
      console.log('[FULL UI REVIEW] Switching to Chat tab...');
      await page.click('button[onclick="switchTab(\'chat\')"]');
      await sleep(2000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-08-chat-tab-default.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-08-chat-tab-default.png');

      // ====================================
      // 10. CHAT - With Messages
      // ====================================
      console.log('[FULL UI REVIEW] Simulating chat messages...');
      await page.evaluate(() => {
        const input = document.getElementById('chat-input');
        if (input) {
          input.value = 'What is the best way to optimize GPU memory usage in llama.cpp?';
          input.dispatchEvent(new Event('input', { bubbles: true }));
        }
      });
      await sleep(500);
      await page.evaluate(() => {
        const sendBtn = document.querySelector('.chat-input-send');
        if (sendBtn) sendBtn.click();
      });
      await sleep(5000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-09-chat-with-messages.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-09-chat-with-messages.png');

      // ====================================
      // 11. TAB PINNING FEATURE
      // ====================================
      console.log('[FULL UI REVIEW] Creating and pinning tabs...');
      await page.evaluate(() => {
        const addBtn = document.querySelector('.chat-tab-add');
        if (addBtn) { addBtn.click(); addBtn.click(); addBtn.click(); }
      });
      await sleep(1000);
      await page.evaluate(() => {
        const pins = document.querySelectorAll('.chat-tab-pin-icon');
        if (pins.length >= 2) {
          pins[0].click();
          pins[1].click();
        }
      });
      await sleep(500);
      await page.evaluate(() => {
        if (window.chatTabs && window.chatTabs[0]) window.chatTabs[0].name = '💾 Rust Help';
        if (window.chatTabs && window.chatTabs[1]) window.chatTabs[1].name = '💾 Code Review';
      });
      await sleep(500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-10-tab-pinning.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-10-tab-pinning.png');

      // ====================================
      // 12. PERSONA STRIP
      // ====================================
      console.log('[FULL UI REVIEW] Capturing persona strip...');
      await page.evaluate(() => {
        const personas = document.querySelectorAll('.persona-chip');
        personas.forEach((chip, i) => {
          if (i < 3) chip.click();
        });
      });
      await sleep(2000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-11-persona-strip.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-11-persona-strip.png');

      // ====================================
      // 13. MESSAGE ACTIONS (Edit/Regenerate)
      // ====================================
      console.log('[FULL UI REVIEW] Capturing message actions...');
      await page.evaluate(() => {
        const actions = document.querySelectorAll('.message-actions-btn');
        if (actions[0]) actions[0].click(); // Edit first message
      });
      await sleep(1000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-12-message-actions.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-12-message-actions.png');

      // ====================================
      // 14. CHAT EXPORT MODAL (separate capture)
      // ====================================
      console.log('[FULL UI REVIEW] Opening export modal...');
      await page.evaluate(() => {
        // Close any open modal first
        const closeBtn = document.querySelector('.modal-close');
        if (closeBtn) closeBtn.click();
      });
      await sleep(500);
      await page.evaluate(() => {
        const exportBtn = document.querySelector('.chat-export-btn');
        if (exportBtn) exportBtn.click();
      });
      await sleep(1500);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-13-export-modal.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-13-export-modal.png');

      // ====================================
      // 14. SETTINGS MODAL
      // ====================================
      console.log('[FULL UI REVIEW] Opening Settings modal...');
      await page.evaluate(() => {
        const settingsBtn = document.querySelector('#sidebar-btn-settings');
        if (settingsBtn) settingsBtn.click();
      });
      await sleep(3000);
      await page.screenshot({ path: `${OUTPUT_DIR}/full-14-settings-modal.png`, fullPage: true });
      console.log('[FULL UI REVIEW] ✓ full-14-settings-modal.png');

          console.log('\n[FULL UI REVIEW] ✅ All screenshots captured successfully!');
      console.log(`[FULL UI REVIEW] Output directory: ${OUTPUT_DIR}`);
      const files = fs.readdirSync(OUTPUT_DIR).filter(f => f.startsWith('full-')).sort();
      files.forEach(f => console.log(`  - ${f}`));

    } catch (error) {
      console.error('[FULL UI REVIEW] Error during capture:', error.message);
    } finally {
      await browser.close();
      console.log('[FULL UI REVIEW] Browser closed');
    }
  } catch (error) {
    console.error('[FULL UI REVIEW] Fatal error:', error.message);
  }
})();
