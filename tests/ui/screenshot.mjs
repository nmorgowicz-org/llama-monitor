import puppeteer from 'puppeteer';
import fs from 'fs';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://localhost:7778';
const REMOTE_SERVER = 'http://192.168.2.16:8001';
const OUTPUT_DIR = '../../docs/screenshots';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(OUTPUT_DIR, { recursive: true });

(async () => {
  console.log('Launching browser...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  try {
    console.log('Taking welcome screen screenshot...');
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });
    await sleep(3000);
    await page.screenshot({ path: `${OUTPUT_DIR}/01-welcome.png`, fullPage: true });
    console.log('Done: 01-welcome.png');

    console.log('Attaching to remote server...');
    await page.evaluate((url) => {
      const input = document.getElementById('setup-endpoint-url');
      if (input) {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    }, REMOTE_SERVER);
    await sleep(500);
    await page.evaluate(() => document.querySelector('button[onclick="doAttachFromSetup()"]')?.click());
    await sleep(8000);

    console.log('Taking inference metrics screenshot...');
    await page.click('button[onclick="switchTab(\'server\')"]');
    await sleep(5000);
    await page.screenshot({ path: `${OUTPUT_DIR}/02-inference-metrics.png`, fullPage: true });
    console.log('Done: 02-inference-metrics.png');

    console.log('Taking chat screenshot...');
    await page.click('button[onclick="switchTab(\'chat\')"]');
    await sleep(2000);

    await page.evaluate(() => {
      const input = document.getElementById('chat-input');
      if (input) {
        input.value = 'Hello, can you tell me a short joke?';
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });
    await sleep(500);
    await page.evaluate(() => document.getElementById('btn-send')?.click());

    // Wait for the LLM to finish responding (typing indicator hidden, send button re-enabled)
    console.log('Waiting for LLM response to complete...');
    await page.waitForFunction(() => {
      const typing = document.getElementById('chat-typing');
      const sendBtn = document.getElementById('btn-send');
      return typing && typing.style.display === 'none' && sendBtn && !sendBtn.disabled;
    }, { timeout: 90000 });
    await sleep(500); // small buffer for final DOM settle

    await page.screenshot({ path: `${OUTPUT_DIR}/03-chat.png`, fullPage: true });
    console.log('Done: 03-chat.png');

    console.log('Taking GPU/system metrics screenshot...');
    await page.click('button[onclick="switchTab(\'server\')"]');
    await sleep(3000);
    await page.evaluate(() => {
      const gpuSection = document.getElementById('gpu-section');
      if (gpuSection) gpuSection.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/04-gpu-metrics.png`, fullPage: true });
    console.log('Done: 04-gpu-metrics.png');

    console.log('Taking logs screenshot...');
    await page.click('button[onclick="switchTab(\'logs\')"]');
    await sleep(2000);
    await page.screenshot({ path: `${OUTPUT_DIR}/05-logs.png`, fullPage: true });
    console.log('Done: 05-logs.png');

    console.log('\nAll screenshots saved to docs/screenshots/');
  } catch (err) {
    console.error('Error:', err.message);
    console.error(err.stack);
  } finally {
    await browser.close();
  }
})();
