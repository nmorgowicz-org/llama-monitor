import puppeteer from 'puppeteer';
import fs from 'fs';

const BASE_URL = process.env.LLAMA_MONITOR_URL || 'http://localhost:7778';
const REMOTE_SERVER = 'http://192.168.2.16:8001';
const OUTPUT_DIR = '../../docs/screenshots';
const CHAT_ONLY = process.argv.includes('--chat-only');

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
    if (CHAT_ONLY) {
      await captureChatOnly(page, BASE_URL, REMOTE_SERVER);
    } else {
      await captureAll(page, BASE_URL, REMOTE_SERVER);
    }
  } catch (err) {
    console.error('Error:', err.message);
    console.error(err.stack);
  } finally {
    await browser.close();
  }
})();

async function captureAll(page, baseUrl, remoteServer) {
  console.log('Taking welcome screen screenshot...');
  await page.goto(baseUrl, { waitUntil: 'networkidle0' });
  await sleep(3000);
  await page.screenshot({ path: `${OUTPUT_DIR}/01-welcome.png`, fullPage: true });
  console.log('Done: 01-welcome.png');

  await attachToServer(page, remoteServer);

  console.log('Taking inference metrics screenshot...');
  await page.click('button[onclick="switchTab(\'server\')"]');
  await sleep(5000);
  await page.screenshot({ path: `${OUTPUT_DIR}/02-inference-metrics.png`, fullPage: true });
  console.log('Done: 02-inference-metrics.png');

  await captureChat(page, '03-chat.png');

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
}

async function captureChatOnly(page, baseUrl, remoteServer) {
  console.log('[CHAT ONLY] Navigating and attaching to server...');
  await page.goto(baseUrl, { waitUntil: 'networkidle0' });
  await sleep(3000);

  await attachToServer(page, remoteServer);

  console.log('[CHAT ONLY] Switching to chat tab...');
  await page.click('button[onclick="switchTab(\'chat\')"]');
  await sleep(2000);

  // Create a second tab to demonstrate multi-tab
  console.log('[CHAT ONLY] Creating second tab...');
  await page.evaluate(() => {
    const addBtn = document.querySelector('.chat-tab-add');
    if (addBtn) addBtn.click();
  });
  await sleep(500);

  // Inject sample conversation into first tab (no LLM needed)
  console.log('[CHAT ONLY] Injecting sample messages...');
  await page.evaluate(() => {
    const tab = activeChatTab();
    if (!tab) return;
    // Switch to first tab if there are multiple
    if (window.chatTabs && window.chatTabs.length > 1) {
      switchChatTab(window.chatTabs[0].id);
    }
    const t = activeChatTab();
    if (!t) return;
    t.messages = [
      { role: 'user', content: 'What are the key differences between Rust and Go for systems programming?', timestamp_ms: Date.now() - 60000 },
      { role: 'assistant', content: 'Great question. Here are the main differences:\n\n**Memory Safety**\nRust uses ownership and borrowing with compile-time guarantees. Go relies on a garbage collector.\n\n**Concurrency**\n```rust\n// Rust uses fear-free concurrency with ownership\nlet handle = thread::spawn(|| {\n    println!("Hello from thread!");\n});\n```\n\nGo uses goroutines and channels with a simpler mental model but less compile-time safety.\n\n**Performance**\nRust generally has zero-cost abstractions and no GC pauses. Go trades some performance for developer productivity.\n\nWould you like me to dive deeper into any of these areas?', timestamp_ms: Date.now() - 30000 },
    ];
    t.ai_name = 'Qwen3.6';
    t.user_name = 'Nick';
    renderChatMessages();
    renderChatTabs();
  });
  await sleep(800);

  // Open system prompt panel to show it
  console.log('[CHAT ONLY] Opening system prompt panel...');
  await page.evaluate(() => {
    const btn = document.getElementById('btn-system-prompt');
    if (btn) btn.click();
  });
  await sleep(500);

  // Enable explicit mode to show the toggle active
  console.log('[CHAT ONLY] Enabling explicit mode toggle...');
  await page.evaluate(() => {
    const tab = activeChatTab();
    if (tab && !tab.explicit_mode) toggleExplicitMode();
  });
  await sleep(300);

  // Open model params panel
  console.log('[CHAT ONLY] Opening model params panel...');
  await page.evaluate(() => {
    const btn = document.getElementById('btn-model-params');
    if (btn) btn.click();
  });
  await sleep(500);

  await page.screenshot({ path: `${OUTPUT_DIR}/chat-overhaul.png`, fullPage: true });
  console.log('\nChat-only screenshot saved to docs/screenshots/chat-overhaul.png');
}

async function attachToServer(page, remoteServer) {
  console.log('Attaching to remote server...');
  await page.evaluate((url) => {
    const input = document.getElementById('setup-endpoint-url');
    if (input) {
      input.value = url;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  }, remoteServer);
  await sleep(500);
  await page.evaluate(() => document.querySelector('button[onclick="doAttachFromSetup()"]')?.click());
  await sleep(8000);
}

async function captureChat(page, filename) {
  // Inject sample messages directly (no LLM dependency — faster and reliable)
  console.log('Injecting sample messages for screenshot...');
  await page.evaluate(() => {
    const tab = activeChatTab();
    if (!tab) return;
    tab.messages = [
      { role: 'user', content: 'Explain how llama.cpp offloads layers to GPU.', timestamp_ms: Date.now() - 60000 },
      { role: 'assistant', content: 'llama.cpp uses the `-ngl` (number GPU layers) flag to split the model between CPU and GPU.\n\n```bash\nllama-server -m model.gguf -ngl 32\n```\n\nThis offloads 32 transformer layers to the GPU while keeping the rest on CPU. The split happens at the layer boundary — you can\'t split individual layers.\n\n**What stays on CPU:**\n- Token embedding lookup\n- Output softmax + sampling\n- Any layers beyond `-ngl`\n\nThe GPU handles the heavy matrix multiplications in the offloaded layers, which is where most compute happens.', timestamp_ms: Date.now() - 30000 },
    ];
    tab.ai_name = 'Assistant';
    renderChatMessages();
  });
  await sleep(800);

  await page.screenshot({ path: `${OUTPUT_DIR}/${filename}`, fullPage: true });
  console.log(`Done: ${filename}`);
}
