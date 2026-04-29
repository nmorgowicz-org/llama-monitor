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

  await captureChat(page, '03-chat-inline.png');

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

  // Create two extra tabs to demonstrate multi-tab bar
  console.log('[CHAT ONLY] Creating extra tabs...');
  await page.evaluate(() => {
    const addBtn = document.querySelector('.chat-tab-add');
    if (addBtn) { addBtn.click(); addBtn.click(); }
  });
  await sleep(500);

  // Switch back to first tab and inject a rich sample conversation
  console.log('[CHAT ONLY] Injecting sample conversation...');
  await page.evaluate(() => {
    if (window.chatTabs && window.chatTabs.length > 1) {
      switchChatTab(window.chatTabs[0].id);
    }
    const t = activeChatTab();
    if (!t) return;

    // Rename tabs for the screenshot
    if (window.chatTabs) {
      if (window.chatTabs[0]) window.chatTabs[0].name = 'Rust vs Go';
      if (window.chatTabs[1]) window.chatTabs[1].name = 'Refactor help';
      if (window.chatTabs[2]) window.chatTabs[2].name = 'New Chat';
    }

    t.ai_name = 'Qwen3.6';
    t.user_name = 'Nick';
    t.messages = [
      {
        role: 'user',
        content: 'What are the key differences between Rust and Go for systems programming?',
        timestamp_ms: Date.now() - 120000,
      },
      {
        role: 'assistant',
        content: `Great question. Here are the main differences:\n\n**Memory Safety**\nRust enforces ownership and borrowing at compile time — no GC, no dangling pointers. Go uses a garbage collector, which simplifies code but adds latency.\n\n**Concurrency**\n\`\`\`rust\nuse std::thread;\n\nfn main() {\n    let handle = thread::spawn(|| {\n        println!("Hello from a thread!");\n    });\n    handle.join().unwrap();\n}\n\`\`\`\n\nGo uses goroutines and channels — simpler mental model, but no compile-time data-race prevention.\n\n**Performance**\nRust has zero-cost abstractions and no GC pauses. Go trades some performance for developer speed.\n\nWant me to benchmark a specific use case?`,
        timestamp_ms: Date.now() - 90000,
        input_tokens: 42,
        output_tokens: 187,
      },
      {
        role: 'user',
        content: 'Show me a Rust example of safe concurrent shared state.',
        timestamp_ms: Date.now() - 60000,
      },
      {
        role: 'assistant',
        content: `Sure — the standard pattern uses \`Arc<Mutex<T>>\`:\n\n\`\`\`rust\nuse std::sync::{Arc, Mutex};\nuse std::thread;\n\nfn main() {\n    let counter = Arc::new(Mutex::new(0u32));\n\n    let handles: Vec<_> = (0..4).map(|_| {\n        let c = Arc::clone(&counter);\n        thread::spawn(move || {\n            let mut val = c.lock().unwrap();\n            *val += 1;\n        })\n    }).collect();\n\n    for h in handles { h.join().unwrap(); }\n    println!("Final: {}", *counter.lock().unwrap()); // always 4\n}\n\`\`\`\n\n- \`Arc\` — atomic reference count (cheap clone, heap-allocated)\n- \`Mutex\` — exclusive lock; \`.lock()\` blocks until acquired\n- The compiler **prevents** sharing without \`Arc\`; no runtime surprise\n\nFor read-heavy workloads, swap \`Mutex\` for \`RwLock\` to allow concurrent readers.`,
        timestamp_ms: Date.now() - 30000,
        input_tokens: 230,
        output_tokens: 312,
      },
    ];

    // Set visible_message_limit low enough to trigger the Load More button
    t.visible_message_limit = 3;

    renderChatMessages();
    renderChatTabs();
  });
  await sleep(1200); // let hljs run on the code blocks

  // Open model params panel to show the controls
  console.log('[CHAT ONLY] Opening model params panel...');
  await page.evaluate(() => {
    const btn = document.getElementById('btn-model-params');
    if (btn) btn.click();
  });
  await sleep(500);

  // Set a non-default temperature to light up the dirty indicator
  console.log('[CHAT ONLY] Setting non-default temperature...');
  await page.evaluate(() => {
    const slider = document.getElementById('param-temperature');
    if (slider) {
      slider.value = '0.4';
      slider.dispatchEvent(new Event('input', { bubbles: true }));
    }
  });
  await sleep(300);

  await page.screenshot({ path: `${OUTPUT_DIR}/03-chat.png`, fullPage: true });
  console.log('Done: 03-chat.png');

  // Second shot: chat-only with system prompt panel open (for docs variety)
  await page.evaluate(() => {
    const paramsBtn = document.getElementById('btn-model-params');
    if (paramsBtn) paramsBtn.click(); // close params
  });
  await sleep(400);
  await page.evaluate(() => {
    const sysBtn = document.getElementById('btn-system-prompt');
    if (sysBtn) sysBtn.click();
  });
  await sleep(500);

  await page.screenshot({ path: `${OUTPUT_DIR}/chat-system-prompt.png`, fullPage: true });
  console.log('Done: chat-system-prompt.png');
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
  // Inject a rich sample conversation with code blocks (no LLM dependency)
  console.log('Injecting sample messages for screenshot...');
  await page.evaluate(() => {
    const tab = activeChatTab();
    if (!tab) return;
    tab.ai_name = 'Assistant';
    tab.messages = [
      {
        role: 'user',
        content: 'Explain how llama.cpp offloads layers to GPU.',
        timestamp_ms: Date.now() - 90000,
      },
      {
        role: 'assistant',
        content: `llama.cpp uses the \`-ngl\` (number GPU layers) flag to split the model between CPU and GPU.\n\n\`\`\`bash\nllama-server -m model.gguf -ngl 32\n\`\`\`\n\nThis offloads 32 transformer layers to the GPU while keeping the rest on CPU.\n\n**What stays on CPU:**\n- Token embedding lookup\n- Output softmax + sampling\n- Any layers beyond \`-ngl\`\n\n**What goes to GPU:**\n- Transformer self-attention\n- FFN matrix multiplications (the bulk of compute)\n\nThe GPU handles the heavy matrix multiplications in the offloaded layers, which is where most of the compute time is spent.`,
        timestamp_ms: Date.now() - 60000,
        input_tokens: 38,
        output_tokens: 156,
      },
      {
        role: 'user',
        content: 'How do I check which layers are on GPU vs CPU at runtime?',
        timestamp_ms: Date.now() - 30000,
      },
      {
        role: 'assistant',
        content: `The cleanest way is to check llama-server's startup log — it prints a per-layer offload summary:\n\n\`\`\`\nllm_load_tensors: offloading 32 repeating layers to GPU\nllm_load_tensors: offloading non-repeating layers to GPU\nllm_load_tensors: offloaded 33/33 layers to GPU\nllm_load_tensors: VRAM used: 14532 MiB\n\`\`\`\n\nYou can also query the Prometheus metrics endpoint at runtime:\n\n\`\`\`bash\ncurl -s http://localhost:8080/metrics | grep kv_cache\n\`\`\`\n\nIf you want programmatic access, the \`/v1/completions\` response includes \`timings\` with \`predicted_ms\` and \`prompt_ms\` which indirectly reflect GPU utilisation.`,
        timestamp_ms: Date.now() - 10000,
        input_tokens: 195,
        output_tokens: 203,
      },
    ];
    renderChatMessages();
  });
  await sleep(1200); // let hljs highlight the code blocks

  await page.screenshot({ path: `${OUTPUT_DIR}/${filename}`, fullPage: true });
  console.log(`Done: ${filename}`);
}
