import puppeteer from 'puppeteer';
import fs from 'fs';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://127.0.0.1:7778';
const OUTPUT_DIR = '../../docs/screenshots';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

(async () => {
  console.log('[FULL UI REVIEW] Launching browser...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  try {
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });
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
    await page.screenshot({ path: `${OUTPUT_DIR}/full-06-gpu-metrics-section.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-06-gpu-metrics-section.png');

    // ====================================
    // 8. CHAT TAB (Empty State)
    // ====================================
    console.log('[FULL UI REVIEW] Switching to Chat tab (empty state)...');
    await page.click('button[onclick="switchTab(\'chat\')"]');
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-07-chat-tab-empty.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-07-chat-tab-empty.png');

    // ====================================
    // 9. CHAT TAB (With Persona Strip & Pin)
    // ====================================
    console.log('[FULL UI REVIEW] Creating tabs and pinning...');
    await page.evaluate(() => {
      const addBtn = document.querySelector('.chat-tab-add');
      if (addBtn) { addBtn.click(); addBtn.click(); }
    });
    await sleep(1000);
    await page.evaluate(() => {
      const pins = document.querySelectorAll('.chat-tab-pin-icon');
      if (pins.length >= 2) { pins[0].click(); }
    });
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-08-chat-tab-with-features.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-08-chat-tab-with-features.png');

    // ====================================
    // 10. SETTINGS MODAL
    // ====================================
    console.log('[FULL UI REVIEW] Opening Settings modal...');
    await page.click('button[aria-label="Settings"]');
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-09-settings-modal.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-09-settings-modal.png');

    // ====================================
    // 11. MODEL SELECTION MODAL
    // ====================================
    console.log('[FULL UI REVIEW] Opening Models modal...');
    await page.evaluate(() => {
      if (document.querySelector('#settings-modal .modal-close')) {
        document.querySelector('#settings-modal .modal-close').click();
      }
    });
    await sleep(500);
    await page.click('button[aria-label="Models"]');
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-10-models-modal.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-10-models-modal.png');

    // ====================================
    // 12. LOGS TAB
    // ====================================
    console.log('[FULL UI REVIEW] Switching to Logs tab...');
    await page.evaluate(() => {
      if (document.querySelector('#models-modal .modal-close')) {
        document.querySelector('#models-modal .modal-close').click();
      }
    });
    await sleep(500);
    await page.click('button[onclick="switchTab(\'logs\')"]');
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-11-logs-tab.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-11-logs-tab.png');

    // ====================================
    // 13. CHAT INPUT & MESSAGE AREA
    // ====================================
    console.log('[FULL UI REVIEW] Capturing chat input area...');
    await page.click('button[onclick="switchTab(\'chat\')"]');
    await sleep(1000);
    await page.evaluate(() => {
      const input = document.getElementById('chat-input');
      if (input) {
        input.focus();
        input.scrollIntoView({ behavior: 'instant', block: 'end' });
      }
    });
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-12-chat-input-area.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-12-chat-input-area.png');

    // ====================================
    // 14. MODEL PARAMS PANEL (Expanded)
    // ====================================
    console.log('[FULL UI REVIEW] Opening model params panel...');
    await page.click('#btn-model-params');
    await sleep(500);
    await page.click('#param-temperature');
    await page.evaluate(() => {
      const slider = document.getElementById('param-temperature');
      if (slider) {
        slider.value = '0.7';
        slider.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });
    await sleep(300);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-13-model-params-panel.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-13-model-params-panel.png');

    // ====================================
    // 15. FINAL OVERVIEW
    // ====================================
    console.log('[FULL UI REVIEW] Capturing final overview...');
    await page.evaluate(() => {
      const sysBtn = document.getElementById('btn-system-prompt');
      if (sysBtn) sysBtn.click();
    });
    await sleep(500);
    await page.screenshot({ path: `${OUTPUT_DIR}/full-15-final-overview.png`, fullPage: true });
    console.log('[FULL UI REVIEW] ✓ full-15-final-overview.png');

    console.log('\n[FULL UI REVIEW] All screenshots saved to docs/screenshots/');

  } catch (err) {
    console.error('[FULL UI REVIEW] Error:', err.message);
    console.error(err.stack);
  } finally {
    await browser.close();
  }
})();
