import puppeteer from 'puppeteer';
import fs from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { spawn } from 'child_process';
import net from 'net';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const OUTPUT_DIR = join(__dirname, '../../docs/screenshots');

const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';
const CHAT_ONLY = process.argv.includes('--chat-only');
const SCREENSHOT_PORT = parseInt(process.env.SCREENSHOT_PORT || '8891', 10);
const SCREENSHOT_TAB_PREFIX = '[screenshot]';

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

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

async function waitForMonitor(page) {
    await page.waitForFunction(() => {
        const setup = document.getElementById('view-setup');
        const monitor = document.getElementById('view-monitor');
        if (!setup || !monitor) return false;
        return getComputedStyle(monitor).display !== 'none' && getComputedStyle(setup).display === 'none';
    }, { timeout: 30000 });
}

async function switchTab(page, tabName) {
    await page.click(`button[data-tab="${tabName}"]`);
    await page.waitForFunction((name) => {
        const tab = document.querySelector(`button[data-tab="${name}"]`);
        const pageEl = document.getElementById(`page-${name}`);
        if (!tab || !pageEl) return false;
        const activeTab = tab.classList.contains('active') || tab.classList.contains('selected');
        const activePage = pageEl.classList.contains('active');
        return activeTab && activePage;
    }, { timeout: 10000 }, tabName);
}

async function attachToServer(page, remoteServer) {
    console.log(`Attaching to ${remoteServer}...`);
    await page.waitForSelector('#setup-endpoint-url', { visible: true });
    await page.$eval('#setup-endpoint-url', (input, url) => {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, remoteServer);
    await sleep(200);
    await page.click('#setup-attach-btn');
    await waitForMonitor(page);
    await page.waitForFunction(
        endpoint => document.getElementById('endpoint-url-display')?.textContent?.includes(endpoint),
        { timeout: 15000 },
        remoteServer
    ).catch(() => null);
    await sleep(1500);
}

async function clearExistingChats(page) {
    await page.evaluate(() => {
        const clearBtn = document.getElementById('btn-clear');
        clearBtn?.click();
    });
    await sleep(200);
}

async function cleanupScreenshotTabs(page, { keepOne = false } = {}) {
    await page.evaluate(async ({ prefix, keepOne }) => {
        const { chat } = await import('/js/core/app-state.js');
        const { newChatTab, persistChatTabs } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');

        const screenshotTabs = chat.tabs.filter(tab => tab.name.startsWith(prefix));
        const keepId = keepOne ? screenshotTabs.at(-1)?.id : null;

        chat.tabs = chat.tabs.filter(tab => {
            if (!tab.name.startsWith(prefix)) return true;
            return keepOne && tab.id === keepId;
        });

        if (!chat.tabs.length) {
            const fallback = newChatTab('Chat 1');
            chat.tabs = [fallback];
            chat.activeTabId = fallback.id;
        } else if (!chat.tabs.some(tab => tab.id === chat.activeTabId)) {
            chat.activeTabId = chat.tabs[chat.tabs.length - 1].id;
        }

        renderChatTabs();
        renderChatMessages();
        await persistChatTabs();
    }, { prefix: SCREENSHOT_TAB_PREFIX, keepOne });
}

async function createFreshChat(page) {
    await switchTab(page, 'chat');
    await page.waitForSelector('#chat-input', { visible: true });

    await cleanupScreenshotTabs(page);
    await page.click('#chat-tab-add-btn');
    await page.waitForFunction(prefix => {
        const tabs = Array.from(document.querySelectorAll('#chat-tab-bar .chat-tab'));
        const active = tabs.find(tab => tab.classList.contains('active'));
        return !!active && active.textContent.includes(prefix);
    }, { timeout: 5000 }, SCREENSHOT_TAB_PREFIX).catch(() => null);

    await page.evaluate(async (prefix) => {
        const { chat } = await import('/js/core/app-state.js');
        const { persistChatTabs } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');

        const activeTab = chat.tabs.find(tab => tab.id === chat.activeTabId);
        if (!activeTab) return;
        activeTab.name = `${prefix} Chat`;
        activeTab.messages = [];
        activeTab.updated_at = Date.now();
        renderChatTabs();
        renderChatMessages();
        await persistChatTabs();
    }, SCREENSHOT_TAB_PREFIX);

    await clearExistingChats(page);
    await sleep(300);
}

async function waitForChatResponse(page, timeoutMs = 180000) {
    await page.waitForFunction(
        () => {
            const streaming = document.querySelector('#chat-messages .chat-message-streaming');
            if (streaming) return false;
            const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
            const thinkingBlocks = Array.from(document.querySelectorAll('#chat-messages .chat-thinking'));
            return assistantMessages.length > 0 || thinkingBlocks.length > 0;
        },
        { timeout: timeoutMs }
    );
}

async function sendChatPrompt(page, prompt) {
    await page.$eval('#chat-input', (input, text) => {
        input.value = text;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, prompt);
    await page.click('#btn-send');
    await page.waitForFunction(() => {
        return document.querySelectorAll('#chat-messages .chat-message-user').length > 0;
    }, { timeout: 10000 });
}

async function captureWelcome(page, baseUrl) {
    console.log('Capturing welcome screen...');
    await page.goto(baseUrl, { waitUntil: 'networkidle0' });
    await page.waitForSelector('#view-setup', { visible: true });
    await sleep(1500);
    await page.screenshot({ path: `${OUTPUT_DIR}/01-welcome.png`, fullPage: true });
}

async function captureLogs(page) {
    console.log('Capturing logs screen...');
    await switchTab(page, 'logs');
    await page.waitForSelector('#logs-empty-state.visible', { timeout: 10000 });
    await sleep(1200);
    await page.screenshot({ path: `${OUTPUT_DIR}/05-logs.png`, fullPage: true });
}

async function captureChat(page) {
    console.log('Capturing chat screen...');
    await createFreshChat(page);
    await sendChatPrompt(
        page,
        'Reply with a compact final answer only: explain what llama.cpp GPU offload via -ngl does and why prompt processing and generation often run at different token speeds.'
    );
    await waitForChatResponse(page);
    await sleep(1500);
    await page.screenshot({ path: `${OUTPUT_DIR}/03-chat.png`, fullPage: true });
    await cleanupScreenshotTabs(page);
}

async function captureAll(page, baseUrl, remoteServer) {
    await captureWelcome(page, baseUrl);
    await attachToServer(page, remoteServer);
    await captureChat(page);
    await captureLogs(page);
}

async function captureChatOnly(page, baseUrl, remoteServer) {
    await page.goto(baseUrl, { waitUntil: 'networkidle0' });
    await attachToServer(page, remoteServer);
    await captureChat(page);
}

(async () => {
    const port = await findAvailablePort();
    console.log(`Using port: ${port}`);

    let server = null;
    const browser = await puppeteer.launch({
        headless: 'new',
        args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage'],
    });

    try {
        server = await spawnLlamaMonitor(port);
        const page = await browser.newPage();
        await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: 1 });

        if (CHAT_ONLY) {
            await captureChatOnly(page, server.url, REMOTE_SERVER);
        } else {
            await captureAll(page, server.url, REMOTE_SERVER);
        }

        console.log(`Screenshots saved to ${OUTPUT_DIR}`);
    } catch (err) {
        console.error(err.stack || err.message);
        process.exitCode = 1;
    } finally {
        await browser.close();
        await cleanupServer(server);
    }
})();
