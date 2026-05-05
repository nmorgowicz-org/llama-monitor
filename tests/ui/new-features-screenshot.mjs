import puppeteer from 'puppeteer';
import fs from 'fs';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://localhost:7778';
const OUTPUT_DIR = '../../docs/screenshots';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

(async () => {
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

    // Attach to remote server
    console.log('[NEW FEATURES] Attaching to remote server at http://192.168.2.16:8001...');
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

    // Switch to chat tab
    console.log('[NEW FEATURES] Switching to chat tab...');
    await page.click('button[onclick="switchTab(\'chat\')"]');
    await sleep(2000);

    // Create multiple tabs for pin demo
    console.log('[NEW FEATURES] Creating multiple tabs...');
    await page.evaluate(() => {
      const addBtn = document.querySelector('.chat-tab-add');
      if (addBtn) { addBtn.click(); addBtn.click(); addBtn.click(); }
    });
    await sleep(1000);

    // Pin the first two tabs
    console.log('[NEW FEATURES] Pinning first two tabs...');
    await page.evaluate(() => {
      const pins = document.querySelectorAll('.chat-tab-pin-icon');
      if (pins.length >= 2) {
        pins[0].click(); // Pin first tab
        pins[1].click(); // Pin second tab
      }
    });
    await sleep(500);

    // Rename tabs for clarity
    await page.evaluate(() => {
      const tabs = window.chatTabs || [];
      if (tabs[0]) tabs[0].name = '💾 Rust Help';
      if (tabs[1]) tabs[1].name = '💾 Code Review';
      if (tabs[2]) tabs[2].name = 'Creative Writing';
      if (tabs[3]) tabs[3].name = 'Technical Chat';
      // Trigger re-render via tab switching or DOM update
      const activeTab = tabs.find(t => t.id === window.activeTabId);
      if (activeTab) activeTab.updated_at = Date.now();
    });
    await sleep(500);

    await page.screenshot({ path: `${OUTPUT_DIR}/06-tab-pinning.png`, fullPage: true });
    console.log('[NEW FEATURES] Done: 06-tab-pinning.png');

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

    // We already captured the key features - tab pinning and persona strip
    // Additional feature screenshots can be captured manually during QA

    console.log('\n[NEW FEATURES] All screenshots saved to docs/screenshots/');

  } catch (err) {
    console.error('[NEW FEATURES] Error:', err.message);
    console.error(err.stack);
  } finally {
    await browser.close();
  }
})();
