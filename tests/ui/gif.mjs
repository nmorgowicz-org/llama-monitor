import puppeteer from 'puppeteer';
import fs from 'fs';
import { execSync } from 'child_process';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://localhost:7778';
const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';
const FRAME_DIR = './frames';
const FPS = 10;
const DURATION_SEC = 5;
const TOTAL_FRAMES = FPS * DURATION_SEC;

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

fs.mkdirSync(FRAME_DIR, { recursive: true });

async function captureFrames(page, prefix, count) {
  for (let i = 0; i < count; i++) {
    const path = `${FRAME_DIR}/${prefix}_${String(i).padStart(3, '0')}.png`;
    await page.screenshot({ path });
    console.log(`  Frame ${i + 1}/${count}`);
    await sleep(1000 / FPS);
  }
}

async function framesToGif(prefix, output) {
  const pattern = `${FRAME_DIR}/${prefix}_*.png`;
  console.log(`Creating ${output}...`);
  execSync(`ffmpeg -y -framerate ${FPS} -pattern_type glob -i '${pattern}' -vf "split[s0][s1];[s0]palettegen=stats_mode=diff[m];[s1][m]paletteuse=dither=bayer:bayer_scale=5" ${output}`);
  console.log(`Done: ${output} (${(fs.statSync(output).size / 1024).toFixed(0)} KB)`);
}

(async () => {
  console.log('Launching browser...');
  const browser = await puppeteer.launch({
    headless: 'new',
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
  });

  const page = await browser.newPage();
  await page.setViewport({ width: 1440, height: 900 });

  try {
    // Navigate and attach
    await page.goto(BASE_URL, { waitUntil: 'networkidle0' });
    await sleep(2000);

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

    // 1. Inference metrics GIF
    console.log('\nCapturing inference metrics...');
    await page.click('button[onclick="switchTab(\'server\')"]');
    await sleep(3000);
    await captureFrames(page, 'inference', TOTAL_FRAMES);
    await framesToGif('inference', '../../docs/screenshots/inference-metrics.gif');

    // 2. GPU/System metrics GIF (scroll to GPU section)
    console.log('\nCapturing GPU/system metrics...');
    await page.evaluate(() => {
      const gpuSection = document.getElementById('gpu-section');
      if (gpuSection) gpuSection.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(2000);
    await captureFrames(page, 'gpu', TOTAL_FRAMES);
    await framesToGif('gpu', '../../docs/screenshots/gpu-metrics.gif');

    // Cleanup frames
    execSync(`rm -rf ${FRAME_DIR}`);
    console.log('\nDone! GIFs saved to docs/screenshots/');
  } catch (err) {
    console.error('Error:', err.message);
  } finally {
    await browser.close();
  }
})();
