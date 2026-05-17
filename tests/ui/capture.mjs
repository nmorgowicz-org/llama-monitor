/**
 * Consolidated screenshot and GIF capture harness for llama-monitor.
 *
 * This is the single entrypoint for repo-owned visual capture automation.
 * Prefer adding new scenarios here instead of creating new one-off scripts.
 *
 * Quick examples:
 *   node tests/ui/capture.mjs --list-scenarios
 *   node tests/ui/capture.mjs --scenario welcome
 *   SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario chat
 *   SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario guided-gen
 *   SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
 *
 * Adding a new screenshot flow:
 *   1. Add a `scenario<Name>()` function near the other scenario functions.
 *   2. Reuse shared helpers for boot, attach, tab cleanup, prompts, and captures.
 *   3. Register the scenario in `SCENARIOS`.
 *   4. Add an example to `printUsage()` and the docs in `tests/ui/README.md`.
 *
 * Troubleshooting:
 *   - If remote agent data is missing, confirm `REMOTE_SERVER` is reachable and
 *     that the seeded temp config copied the needed local settings/token state.
 *   - If captures do not reflect source edits, rebuild the release binary with
 *     `cargo build --release`; the harness launches `target/release/llama-monitor`.
 *   - If a port is occupied, set `SCREENSHOT_PORT` to a higher base port; the
 *     harness scans forward from that port.
 *   - If chat screenshots leave test tabs behind, use the shared screenshot tab
 *     helpers rather than mutating tab state ad hoc.
 *   - If a popover appears missing, log its geometry/state from the scenario and
 *     verify whether the issue is interaction, clipping, or stale release assets.
 */
import puppeteer from 'puppeteer';
import fs from 'fs';
import os from 'os';
import net from 'net';
import { execFileSync, spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const ROOT_DIR = join(__dirname, '../..');
const ARTIFACTS_DIR = join(ROOT_DIR, 'docs/screenshots/artifacts');
const SCREENSHOTS_DIR = join(ROOT_DIR, 'docs/screenshots');
const FRAME_DIR = join(__dirname, 'frames');
const REAL_APP_CONFIG_DIR = join(process.env.HOME || os.homedir(), '.config', 'llama-monitor');
const TEMP_HOME = fs.mkdtempSync(join(os.tmpdir(), 'llama-monitor-capture-'));
const TEMP_CONFIG_HOME = join(TEMP_HOME, '.config');
const TEMP_APP_CONFIG_DIR = join(TEMP_CONFIG_HOME, 'llama-monitor');
const SCREENSHOT_TAB_PREFIX = '[screenshot]';

const DEFAULT_VIEWPORT = { width: 1440, height: 900, deviceScaleFactor: 1 };
const DEFAULT_PORT = parseInt(process.env.SCREENSHOT_PORT || '8892', 10);
const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';
const BINARY_PATH = join(ROOT_DIR, 'target/release/llama-monitor');

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

fs.mkdirSync(ARTIFACTS_DIR, { recursive: true });
fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
fs.mkdirSync(TEMP_APP_CONFIG_DIR, { recursive: true });

function parseArgs(argv) {
    const options = {
        scenario: null,
        gpuOnly: false,
        inferenceOnly: false,
        listScenarios: false,
        noAttach: false,
        closeUp: false,
        viewport: { ...DEFAULT_VIEWPORT },
    };

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === '--scenario' && argv[i + 1]) {
            options.scenario = argv[i + 1];
            i += 1;
        } else if (arg === '--chat-only') {
            options.chatOnly = true;
        } else if (arg === '--gpu-only') {
            options.gpuOnly = true;
        } else if (arg === '--inference-only') {
            options.inferenceOnly = true;
        } else if (arg === '--list-scenarios') {
            options.listScenarios = true;
        } else if (arg === '--no-attach') {
            options.noAttach = true;
        } else if (arg === '--close-up') {
            options.closeUp = true;
        } else if (arg === '--help' || arg === '-h') {
            options.help = true;
        }
    }

    if (options.gpuOnly && options.inferenceOnly) {
        throw new Error('Use only one of --gpu-only or --inference-only');
    }

    return options;
}

function printUsage() {
    console.log(`Usage:
  node tests/ui/capture.mjs --scenario <name> [options]

Scenarios:
  Core
    welcome          Welcome screen (no attach required)
    chat             Chat, telemetry, logs

  Chat Features
    guided-gen       Suggestions, quick guide, director, surprise, explicit mode
    sidebar          Sidebar, FTS search, context menu, name filter

  Configuration
    settings         Settings modal, preferences, persona, models, shortcuts
    tls              TLS modes and ACME (Certificates tab, each TLS mode, custom certs, ACME config)
    filebrowser      File browser modal (Browse buttons in Config modal, modal open)
    panels           Chat config panels (behavior, model, style, debug)
    dashboard        Server tab, GPU section

  Validation
    sparkline        Sparkline validation screenshots
    gifs             Inference/GPU animated GIF capture
    smoke            Startup smoke test

Options:
  --gpu-only         For gifs scenario, capture only GPU/system animation
  --inference-only   For gifs scenario, capture only inference animation
  --no-attach        Skip remote attach for scenarios that do not require it
  --close-up         Also capture element-level close-ups (debugging only)
  --list-scenarios   Print available scenarios

Examples:
  node tests/ui/capture.mjs --scenario welcome
  SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario chat
  SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario guided-gen
  SCREENSHOT_PORT=8893 node tests/ui/capture.mjs --scenario sidebar
  SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
  SCREENSHOT_PORT=8894 node tests/ui/capture.mjs --scenario settings --close-up
`);
}

function seedConfig() {
    const filesToCopy = ['ui-settings.json', 'presets.json', 'gpu-env.json'];
    for (const filename of filesToCopy) {
        const source = join(REAL_APP_CONFIG_DIR, filename);
        const destination = join(TEMP_APP_CONFIG_DIR, filename);
        if (fs.existsSync(source)) {
            fs.copyFileSync(source, destination);
        }
    }
}

function cleanupTempHome() {
    fs.rmSync(TEMP_HOME, { recursive: true, force: true });
}

async function findAvailablePort(startPort = DEFAULT_PORT) {
    for (let port = startPort; port < startPort + 200; port += 1) {
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
        } catch {
            // Keep polling.
        }
        await sleep(250);
    }
    throw new Error(`Server did not become ready at ${url} within ${timeout}ms`);
}

async function spawnLlamaMonitor(port) {
    const proc = spawn(BINARY_PATH, ['--port', String(port), '--headless'], {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: {
            ...process.env,
            HOME: TEMP_HOME,
            XDG_CONFIG_HOME: TEMP_CONFIG_HOME,
        },
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

async function launchBrowser(viewport = DEFAULT_VIEWPORT) {
    const browser = await puppeteer.launch({
        headless: 'new',
        args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage', '--disable-cache', '--disable-service-workers'],
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);
    await page.setViewport(viewport);
    return { browser, page };
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
    await page.evaluate((name) => {
        document.querySelector(`button[data-tab="${name}"]`)?.click();
    }, tabName);
    await page.waitForFunction((name) => {
        const tab = document.querySelector(`button[data-tab="${name}"]`);
        const pageEl = document.getElementById(`page-${name}`);
        if (!tab || !pageEl) return false;
        const activeTab = tab.classList.contains('active') || tab.classList.contains('selected');
        const activePage = pageEl.classList.contains('active');
        return activeTab && activePage;
    }, { timeout: 10000 }, tabName);
}

async function attachToServer(page, remoteServer = REMOTE_SERVER) {
    console.log(`[CAPTURE] Attaching to remote server at ${remoteServer}...`);

    // Intercept the /api/attach response to log the real error
    const attachPromise = new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
            reject(new Error('Attach API request timed out (no /api/attach response within 30s)'));
        }, 30000);
        page.once('response', async (response) => {
            if (!response.url().includes('/api/attach')) return;
            clearTimeout(timeout);
            try {
                const body = await response.text();
                console.log(`[CAPTURE] /api/attach response ${response.status()}: ${body.trim()}`);
                resolve();
            } catch (e) {
                console.log(`[CAPTURE] /api/attach response ${response.status()} (read error: ${e.message})`);
                resolve();
            }
        });
    });

    await page.waitForSelector('#setup-endpoint-url', { visible: true });
    await page.$eval('#setup-endpoint-url', (input, url) => {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, remoteServer);
    await sleep(200);

    const attachBtn = await page.$('#setup-attach-btn');
    if (!attachBtn) {
        throw new Error('Attach button #setup-attach-btn not found');
    }
    console.log('[CAPTURE] Clicking attach button...');
    await attachBtn.click();

    // Wait for both the API response and the monitor view
    await Promise.all([
        attachPromise,
        waitForMonitor(page),
    ]);

    // Optional: confirm endpoint displayed (use #endpoint-url, not #endpoint-url-display)
    await page.waitForFunction(
        endpoint => document.getElementById('endpoint-url')?.textContent?.includes(endpoint),
        { timeout: 5000 }
    ).catch(() => {
        console.log('[CAPTURE] Endpoint URL not confirmed in display (non-fatal)');
    });

    await sleep(1500);
    console.log('[CAPTURE] Attach successful.');
}

async function gotoApp(page, baseUrl) {
    await page.goto(baseUrl, { waitUntil: 'networkidle0' });
    await sleep(1500);
}

async function cleanupScreenshotTabs(page, { keepOne = false } = {}) {
    await page.evaluate(async ({ prefix, keepOne }) => {
        const { chat } = await import('/js/core/app-state.js');
        const { newChatTab, persistChatTabs } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');

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
        renderChatSessionsSidebar();
        await persistChatTabs();
    }, { prefix: SCREENSHOT_TAB_PREFIX, keepOne });
}

async function clearExistingChats(page) {
    await page.evaluate(() => {
        document.getElementById('btn-clear')?.click();
    });
    await sleep(200);
}

async function createFreshChat(page) {
    await switchTab(page, 'chat');
    await sleep(500);

    await cleanupScreenshotTabs(page);
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        addChatTab();
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(300);

    await page.evaluate(async prefix => {
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

async function activateScreenshotChat(page) {
    await page.evaluate(prefix => {
        const items = Array.from(document.querySelectorAll('.csp-item'));
        const target = items.find(el => {
            const name = el.querySelector('.csp-item-name')?.textContent || '';
            return name.includes(prefix);
        });
        target?.click();
    }, SCREENSHOT_TAB_PREFIX);
    await sleep(500);
}

async function sendChatPrompt(page, prompt) {
    // Check if chat input is visible
    const chatInput = await page.$('#chat-input');
    if (!chatInput) {
        console.log('[CAPTURE] Chat input not found!');
        throw new Error('Chat input not found');
    }
    const inputVisible = await chatInput.evaluate(el => getComputedStyle(el).display !== 'none');
    console.log('[CAPTURE] Chat input visible:', inputVisible);
    if (!inputVisible) {
        console.log('[CAPTURE] Chat input is not visible, trying to scroll into view...');
        await chatInput.evaluate(el => el.scrollIntoView({ behavior: 'instant', block: 'center' }));
        await sleep(500);
    }
    await page.$eval('#chat-input', (input, text) => {
        input.value = text;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, prompt);
    // Use JavaScript to click the send button (puppeteer click may fail without proper event handling)
    await page.evaluate(() => {
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn) sendBtn.click();
    });
    await page.waitForFunction(() => {
        return document.querySelectorAll('#chat-messages .chat-message-user').length > 0;
    }, { timeout: 10000 });
}

async function logChatState(page, label) {
    const state = await page.evaluate(() => {
        const { chat } = window;
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
        const lastAssistant = assistantMessages[assistantMessages.length - 1];
        const lastBody = lastAssistant?.querySelector('.chat-msg-body');
        const sendBtn = document.getElementById('btn-send');
        return {
            chatBusy: chat?.busy,
            streamingElement: !!streaming,
            assistantCount: assistantMessages.length,
            lastMessageLength: lastBody?.textContent?.length ?? 0,
            sendBtnClass: sendBtn?.className ?? null,
        };
    });
    console.log(`[CAPTURE] ${label}:`, JSON.stringify(state));
    return state;
}

async function waitForChatResponse(page, timeoutMs = 300000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatResponse: waiting for chat response...');
    
    await logChatState(page, 'waitForChatResponse:BEFORE');
    
    await page.waitForFunction(() => {
        // Check chat.busy state directly - this is the authoritative source
        const { chat } = window;
        if (chat?.busy) return false;
        
        // Also check UI state as fallback
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        return assistantMessages.length > 0;
    }, { timeout: timeoutMs });
    
    // Increased buffer to ensure AI has fully completed
    await sleep(10000);
    
    await logChatState(page, 'waitForChatResponse:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatResponse: completed in ${elapsed}ms`);
}

async function waitForChatIdle(page, timeoutMs = 120000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatIdle: waiting for chat to become idle...');
    
    await logChatState(page, 'waitForChatIdle:BEFORE');
    
    // Wait for chat to become idle (no streaming, send button not in stop mode)
    await page.waitForFunction(() => {
        // Check chat.busy state directly - this is the authoritative source
        const { chat } = window;
        if (chat?.busy) return false;
        
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
        return true;
    }, { timeout: timeoutMs });
    
    // Verify idle state is stable (wait 5s to ensure no new streaming starts)
    for (let i = 0; i < 5; i++) {
        await sleep(1000);
        const isIdle = await page.evaluate(() => {
            const { chat } = window;
            if (chat?.busy) return false;
            const streaming = document.querySelector('#chat-messages .chat-message-streaming');
            if (streaming) return false;
            const sendBtn = document.getElementById('btn-send');
            if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
            return true;
        });
        if (!isIdle) {
            console.log('[CAPTURE] waitForChatIdle: chat became busy again, waiting...');
            i = -1; // Reset counter and start over
        }
    }
    
    await logChatState(page, 'waitForChatIdle:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatIdle: completed in ${elapsed}ms`);
}

async function waitForChatComplete(page, timeoutMs = 300000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatComplete: waiting for chat to complete...');
    
    await logChatState(page, 'waitForChatComplete:BEFORE');
    
    // Wait for streaming to stop and assistant message to appear
    await waitForChatResponse(page, timeoutMs);

    // Retry until no [stopped] text in the last assistant message (max 3 retries)
    for (let i = 0; i < 3; i++) {
        const hasStopped = await page.evaluate(() => {
            const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
            if (assistantMessages.length === 0) return false;
            const lastMessage = assistantMessages[assistantMessages.length - 1];
            const body = lastMessage.querySelector('.chat-msg-body');
            return body && body.textContent.includes('[stopped]');
        });

        if (!hasStopped) break;

        console.log(`[CAPTURE] Detected [stopped] response, waiting longer (attempt ${i + 1}/3)...`);
        await sleep(10000);
    }

    // Final check
    const stillStopped = await page.evaluate(() => {
        const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
        if (assistantMessages.length === 0) return false;
        const lastMessage = assistantMessages[assistantMessages.length - 1];
        const body = lastMessage.querySelector('.chat-msg-body');
        return body && body.textContent.includes('[stopped]');
    });

    if (stillStopped) {
        console.log('[CAPTURE] WARNING: [stopped] response persists after retries, may need manual review');
    }
    
    await logChatState(page, 'waitForChatComplete:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatComplete: completed in ${elapsed}ms`);
}

async function waitForChatSettledOrError(page, timeoutMs = 300000) {
    await page.waitForFunction(() => {
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const error = document.querySelector('#chat-messages .chat-error');
        if (error) return true;
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        return assistantMessages.length > 1;
    }, { timeout: timeoutMs });
}

async function waitForSuggestionsSettled(page, timeoutMs = 300000) {
    await page.waitForFunction(() => {
        const dropdown = document.getElementById('suggestions-dropdown');
        const list = document.getElementById('suggestions-list');
        if (!dropdown || !list) return false;
        const isLoading = list.querySelector('.suggestions-loading');
        if (isLoading) return false;
        const hasItems = list.querySelectorAll('.suggestion-item').length > 0;
        const hasEmpty = !!list.querySelector('.suggestions-empty-state');
        const collapsed = dropdown.classList.contains('setup-collapsed');
        return hasItems || hasEmpty || !collapsed;
    }, { timeout: timeoutMs });
}

async function describeSuggestionsPanel(page) {
    return page.evaluate(() => {
        const dropdown = document.getElementById('suggestions-dropdown');
        const list = document.getElementById('suggestions-list');
        const toggle = document.getElementById('suggestions-view-toggle');
        const generate = document.getElementById('suggestions-generate-btn');
        const tagCloud = dropdown?.querySelector('.suggestions-tag-cloud');
        return {
            expanded: dropdown?.classList.contains('dropdown-expanded') ?? false,
            setupCollapsed: dropdown?.classList.contains('setup-collapsed') ?? false,
            toggleLabel: toggle?.textContent?.trim() ?? null,
            generateVisible: !!generate && getComputedStyle(generate).display !== 'none',
            tagCloudVisible: !!tagCloud && getComputedStyle(tagCloud).display !== 'none',
            suggestionCount: list?.querySelectorAll('.suggestion-item').length ?? 0,
            emptyStateText: list?.querySelector('.suggestions-empty-state p')?.textContent?.trim() ?? null,
            loading: !!list?.querySelector('.suggestions-loading'),
        };
    });
}

function attachSuggestionsResponseLogger(page) {
    const handler = async response => {
        if (!response.url().includes('/api/chat/suggestions')) return;
        try {
            const payload = await response.text();
            const condensed = payload.replace(/\s+/g, ' ').slice(0, 3000);
            console.log(`[CAPTURE] Suggestions API ${response.status()}: ${condensed}`);
        } catch (error) {
            console.log(`[CAPTURE] Suggestions API response logging failed: ${error.message}`);
        }
    };
    page.on('response', handler);
    return () => page.off('response', handler);
}

async function captureShot(page, filename, options = {}) {
    await page.screenshot({ path: join(ARTIFACTS_DIR, filename), ...options });
    console.log(`[CAPTURE] Saved ${filename}`);
}

async function captureCloseUp(page, selector, filename, options = {}) {
    if (!options.closeUp) return;
    const padding = options.padding ?? 24;
    const handle = await page.$(selector);
    if (!handle) {
        console.log(`[CAPTURE] Close-up skipped (not found): ${selector}`);
        return;
    }
    await handle.evaluate(el => {
        el.scrollIntoView({ behavior: 'instant', block: 'center', inline: 'nearest' });
    });
    await sleep(300);
    const box = await handle.boundingBox();
    if (!box) return;
    const viewport = page.viewport();
    const clip = {
        x: Math.max(0, box.x - padding),
        y: Math.max(0, box.y - padding),
        width: Math.min((viewport?.width ?? box.width) - Math.max(0, box.x - padding), box.width + padding * 2),
        height: Math.min((viewport?.height ?? box.height) - Math.max(0, box.y - padding), box.height + padding * 2),
    };
    const cuName = filename.replace('.png', '-cu.png');
    await page.screenshot({ path: join(ARTIFACTS_DIR, cuName), clip });
    console.log(`[CAPTURE] Close-up saved ${cuName}`);
}

async function captureElementScreenshot(page, selector, filename, options = {}) {
    // Always capture; --close-up is only for captureCloseUp helper.

    const padding = options.padding ?? 20;
    const handle = await page.$(selector);
    if (!handle) {
        throw new Error(`Missing selector for screenshot capture: ${selector}`);
    }

    await handle.evaluate(el => {
        el.scrollIntoView({ behavior: 'instant', block: 'center', inline: 'nearest' });
    });
    await sleep(options.settleMs ?? 500);

    const box = await handle.boundingBox();
    if (!box) {
        throw new Error(`Selector has no visible bounds: ${selector}`);
    }

    const viewport = page.viewport();
    const clip = {
        x: Math.max(0, box.x - padding),
        y: Math.max(0, box.y - padding),
        width: Math.min((viewport?.width ?? box.width) - Math.max(0, box.x - padding), box.width + padding * 2),
        height: Math.min((viewport?.height ?? box.height) - Math.max(0, box.y - padding), box.height + padding * 2),
    };

    await page.screenshot({ path: join(ARTIFACTS_DIR, filename), clip });
    console.log(`[CAPTURE] Saved ${filename}`);
}

async function captureSparklineClips(page, selector) {
    const rects = await page.$$eval(selector, els => els.map((el, index) => {
        const rect = el.getBoundingClientRect();
        return {
            index,
            x: rect.x + window.scrollX,
            y: rect.y + window.scrollY,
            width: rect.width,
            height: rect.height,
        };
    }).filter(rect => rect.width > 0 && rect.height > 0));

    for (const rect of rects) {
        await page.screenshot({
            path: join(ARTIFACTS_DIR, `sparkline-validate-svg-${rect.index}.png`),
            clip: {
                x: Math.max(0, rect.x),
                y: Math.max(0, rect.y),
                width: Math.max(1, rect.width),
                height: Math.max(1, rect.height),
            },
        });
    }
    console.log(`[CAPTURE] Saved ${rects.length} sparkline SVG clips`);
}

async function startLiveGeneration(remoteServer = REMOTE_SERVER) {
    return fetch(`${remoteServer}/v1/chat/completions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            model: 'default',
            stream: false,
            temperature: 0.7,
            max_tokens: 800,
            messages: [{
                role: 'user',
                content: 'Write a dense explanation of transformer inference performance, token throughput, KV cache behavior, and GPU offload tradeoffs.',
            }],
        }),
    }).then(async response => {
        if (!response.ok) {
            throw new Error(`Generation request failed: ${response.status} ${response.statusText}`);
        }
        await response.text();
    });
}

async function captureFrames(page, prefix, totalFrames, fps) {
    fs.mkdirSync(FRAME_DIR, { recursive: true });
    for (let i = 0; i < totalFrames; i += 1) {
        const path = join(FRAME_DIR, `${prefix}_${String(i).padStart(3, '0')}.png`);
        await page.screenshot({ path });
        await sleep(1000 / fps);
    }
}

function framesToGif(prefix, output, fps) {
    execFileSync('ffmpeg', [
        '-y',
        '-framerate', String(fps),
        '-i', join(FRAME_DIR, `${prefix}_%03d.png`),
        '-vf', 'split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5',
        output,
    ], { stdio: 'inherit' });
}

function cleanupFrames() {
    fs.rmSync(FRAME_DIR, { recursive: true, force: true });
}

async function describePopover(page, toggleSelector, panelSelector) {
    return page.evaluate(({ toggleSelector, panelSelector }) => {
        const toggle = document.querySelector(toggleSelector);
        const panel = document.querySelector(panelSelector);
        if (!toggle || !panel) {
            return { missing: true, toggleFound: !!toggle, panelFound: !!panel };
        }

        const panelStyle = getComputedStyle(panel);
        const toggleRect = toggle.getBoundingClientRect();
        const panelRect = panel.getBoundingClientRect();
        return {
            missing: false,
            toggleAriaExpanded: toggle.getAttribute('aria-expanded'),
            panelClass: panel.className,
            panelOpacity: panelStyle.opacity,
            panelDisplay: panelStyle.display,
            panelPointerEvents: panelStyle.pointerEvents,
            panelHeight: Math.round(panelRect.height),
            panelWidth: Math.round(panelRect.width),
            opensUpward: panelRect.bottom <= toggleRect.top,
        };
    }, { toggleSelector, panelSelector });
}

async function describeQuickGuideFlow(page) {
    return page.evaluate(() => {
        const errors = Array.from(document.querySelectorAll('#chat-messages .chat-error')).map(el => el.textContent?.trim()).filter(Boolean);
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        const lastAssistant = assistantMessages.at(-1)?.querySelector('.chat-msg-body')?.textContent?.trim() ?? null;
        const container = document.getElementById('quick-guide-container');
        const activeMode = document.querySelector('.quick-guide-mode-btn.active')?.dataset.guideMode ?? null;
        const armedChip = document.getElementById('quick-guide-status-chip')?.textContent?.trim() ?? null;
        return {
            assistantCount: assistantMessages.length,
            lastAssistantPreview: lastAssistant?.slice(0, 240) ?? null,
            errorCount: errors.length,
            latestError: errors.at(-1) ?? null,
            quickGuideExpanded: container?.classList.contains('quick-guide-expanded') ?? false,
            activeMode,
            armedChip,
        };
    });
}

async function enableGuidedGeneration(page) {
    await page.evaluate(() => {
        const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
        settings.enabled_context_notes = true;
        settings.enabled_suggestions = true;
        settings.enabled_quick_guide = true;
        localStorage.setItem('llama_monitor_settings', JSON.stringify(settings));
    });
    await sleep(500);
}

// ── Welcome Screen ──────────────────────────────────────────────────────────────

async function scenarioWelcome(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await captureShot(page, 'welcome-welcome.png', { fullPage: true });
}

// ── Core Chat ───────────────────────────────────────────────────────────────────

async function scenarioChat(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a fresh chat with a short, safe conversation
    await cleanupScreenshotTabs(page);
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        addChatTab();
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(300);

    await sendChatPrompt(page, 'Explain how a database index speeds up queries in 3 bullet points.');
    await waitForChatComplete(page);
    await sleep(2000);

    await captureShot(page, 'chat-chat.png', { fullPage: true });

    const telemetryToggle = await page.$('#chat-telemetry-btn');
    if (telemetryToggle) {
        await telemetryToggle.click();
        await sleep(500);
        await captureShot(page, 'chat-chat-telemetry.png', { fullPage: true });
        const telemetryPin = await page.$('#chat-telemetry-pin-btn');
        if (telemetryPin) {
            await telemetryPin.click();
            await sleep(500);
            await captureShot(page, 'chat-chat-telemetry-pinned.png', { fullPage: true });
        }
    }
    await cleanupScreenshotTabs(page);

    await switchTab(page, 'logs');
    await page.waitForSelector('#logs-empty-state.visible', { timeout: 10000 });
    await captureShot(page, 'chat-logs.png', { fullPage: true });
}

// ── Guided Generation ───────────────────────────────────────────────────────────
// Suggestions, quick guide, director, surprise, explicit mode, context notes.

async function scenarioGuidedGen(ctx, options) {
    const { page, baseUrl } = ctx;
    const detachSuggestionsLogger = attachSuggestionsResponseLogger(page);
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    await enableGuidedGeneration(page);

    await createFreshChat(page);
    await sleep(500);

    // Create multiple chats with distinct, safe content for 06-chat-tabs.png
    console.log('[CAPTURE] Creating test chat tabs with content...');
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        for (let i = 0; i < 3; i++) {
            addChatTab();
        }
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(500);

    // Seed short messages into each chat so tabs look realistic
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tabs = chat.tabs;

        if (tabs[0]) {
            tabs[0].name = 'CI Pipeline';
            tabs[0].messages = [
                { role: 'user', content: 'Outline a simple CI pipeline for a Rust backend.' },
                { role: 'assistant', content: 'Use GitHub Actions with these steps:\n- Run cargo fmt and cargo clippy.\n- Run cargo test.\n- Build a release binary.\n- Optionally upload artifacts.' },
            ];
        }
        if (tabs[1]) {
            tabs[1].name = 'Debugging';
            tabs[1].messages = [
                { role: 'user', content: 'List steps to debug a slow HTTP endpoint.' },
                { role: 'assistant', content: '- Profile request duration.\n- Check database queries and indexes.\n- Inspect external service calls.\n- Review logs for retries or timeouts.' },
            ];
        }
        if (tabs[2]) {
            tabs[2].name = 'GPU Monitoring';
            tabs[2].messages = [
                { role: 'user', content: 'How can I monitor GPU temperature and utilization from the CLI?' },
                { role: 'assistant', content: 'Use tools like nvidia-smi, nvtop, or custom scripts that read from /sys/class/thermal and GPU management APIs.' },
            ];
        }

        for (const t of tabs) t.updated_at = Date.now();
        await switchChatTab(tabs[0].id);
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(800);

    await captureShot(page, 'panels-chat-tabs.png', { fullPage: true });
    await activateScreenshotChat(page);

    // For 08-context-notes-expanded.png:
    // - Use a neo-noir style conversation.
    // - Inject context notes so the sidebar shows real data.
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tab = chat.tabs.find(t => t.name?.startsWith('[screenshot]')) || chat.tabs[0];
        if (!tab) return;
        tab.name = '[screenshot] Noir Scene';
        tab.messages = [
            { role: 'user', content: 'Write a short opening scene in a neo-noir detective story.' },
            { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
        ];
        tab.context_notes = [
            { section: 'Character', content: 'Detective in a rain-soaked city, dry humor, haunted by a past case.' },
            { section: 'Setting', content: 'Neo-noir metropolis, neon signs, constant rain, corrupt underworld.' },
            { section: 'Tone', content: 'Tense, cinematic, short punchy lines. No melodrama.' },
        ];
        tab.updated_at = Date.now();
        await switchChatTab(tab.id);
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(600);

    // Open context sidebar with real notes visible
    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(1200);
    await captureShot(page, 'guided-gen-context-notes-expanded.png', { fullPage: true });
    // Close context sidebar
    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('/js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(500);

    // Suggestions dropdown (09-suggestions-dropdown.png)
    // Fresh chat to reduce context buildup
    await createFreshChat(page);
    await sleep(500);
    // Use a short prompt to set context
    await sendChatPrompt(page, 'Brainstorm 3 product names for a CLI tool that monitors GPUs.');
    await waitForChatComplete(page);
    await sleep(2000);

    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(1000);
    console.log('[CAPTURE] Suggestions pre-generate state:', JSON.stringify(
        {
            ...(await describePopover(page, '#suggestions-toggle', '#suggestions-dropdown')),
            ...(await describeSuggestionsPanel(page)),
        }
    ));
    await captureShot(page, 'guided-gen-suggestions-dropdown.png', { fullPage: true });
        await captureCloseUp(page, '#suggestions-dropdown', 'guided-gen-suggestions-dropdown.png', options);
    await page.click('#suggestions-generate-btn');
    await waitForSuggestionsSettled(page);
    await sleep(1200);
    console.log('[CAPTURE] Suggestions generated state:', JSON.stringify(
        await describeSuggestionsPanel(page)
    ));
    await captureShot(page, 'guided-gen-suggestions-results.png', { fullPage: true });
        await captureCloseUp(page, '#suggestions-dropdown', 'guided-gen-suggestions-results.png', options);
    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(500);

    // Quick guide dropdown with real conversation
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(1000);
    console.log('[CAPTURE] Quick guide state:', JSON.stringify(
        await describePopover(page, '#quick-guide-toggle', '#quick-guide-container')
    ));
    await captureShot(page, 'guided-gen-quick-guide-dropdown.png', { fullPage: true });
        await captureCloseUp(page, '#quick-guide-container', 'guided-gen-quick-guide-dropdown.png', options);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(500);

    // Quick guide response: apply a quick guide instruction and capture resulting reply
    // Fresh chat with seeded context for quick guide demo
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        // Use the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'I need help optimizing database queries for a web application.' },
                { role: 'assistant', content: 'I can help with database query optimization. What database are you using, and what kind of queries are slow?' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.type('#quick-guide-input', 'Keep the next reply concise and technical, 3 bullets max.');
    await sleep(300);
    await page.keyboard.press('Enter');
    // Wait for quick guide response to complete before sending next message
    await waitForChatIdle(page);
    // Now send a user message that will use the guide
    await sendChatPrompt(page, 'Explain how connection pooling improves performance.');
    await waitForChatComplete(page);
    await sleep(1500);
    await captureShot(page, 'guided-gen-quick-guide-response.png', { fullPage: true });

  // Director mode: switch to director mode and generate ideas
    // Fresh chat with seeded noir scene for director demo
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab, renderChatTabs, renderChatMessages } = await import('/js/features/chat-state.js');
        const { renderChatTabs: renderTabs2, renderChatMessages: renderMsgs2 } = await import('/js/features/chat-render.js');
        // Find the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.name = '[director] Noir Scene';
            tab.messages = [
                { role: 'user', content: 'Write a short opening scene in a neo-noir detective story.' },
                { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
            ];
            await switchChatTab(tab.id);
            renderTabs2();
            renderMsgs2();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('.quick-guide-mode-btn'))
            .find(b => b.dataset.guideMode === 'director');
        if (btn) btn.click();
    });
    await sleep(400);
    const directorInput = await page.$('#quick-guide-director-input');
    if (directorInput) {
        await directorInput.type('Raise tension and introduce a suspect who may be lying.', { delay: 20 });
        const generateBtn = await page.$('#quick-guide-director-generate-btn');
        if (generateBtn) {
            await generateBtn.click();
            await sleep(2000);
            // Wait for director results
            await page.waitForSelector('.quick-guide-director-item', { timeout: 120000 }).catch(() => {});
            await sleep(800);
            await captureShot(page, 'guided-gen-director-options.png', { fullPage: true });

            // 10d-guide-ai-director-results.png: apply one idea and capture resulting reply
            const applyBtn = await page.$('.quick-guide-director-apply-btn');
            if (applyBtn) {
                await applyBtn.click();
                // Wait for director apply response to complete before sending next message
                await waitForChatIdle(page);
                await sendChatPrompt(page, 'Continue the scene with higher tension.');
await waitForChatComplete(page);
                await sleep(1500);
                await captureShot(page, 'guided-gen-director-applied.png', { fullPage: true });
            }
        }
    }

    // Surprise mode: switch to surprise mode and arm a surprise
    // Fresh chat with content for chat-related screenshot
    await createFreshChat(page);
    await sleep(500);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        // Use the active tab (last one after createFreshChat)
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'Write a scene where a detective discovers a hidden clue.' },
                { role: 'assistant', content: 'The safe was empty, but behind the false back she found a single photograph—her partner, standing next to the victim, both of them smiling.' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('/js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(400);
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('.quick-guide-mode-btn'))
            .find(b => b.dataset.guideMode === 'surprise');
        if (btn) btn.click();
    });
    await sleep(400);
    const surpriseInput = await page.$('#quick-guide-surprise-input');
    if (surpriseInput) {
        await surpriseInput.type('A contact leaks a key piece of evidence that changes everything.', { delay: 20 });
        const armBtn = await page.$('#quick-guide-surprise-arm-btn');
        if (armBtn) {
            await armBtn.click();
            await sleep(800);
            await captureShot(page, 'guided-gen-surprise-armed.png', { fullPage: false });
        }
    }

    // Close quick guide if open
    await page.evaluate(async () => {
        const { closeQuickGuide } = await import('/js/features/chat-quick-guide.js');
        closeQuickGuide();
    });
    await sleep(300);

    // 11-chat-input-buttons.png with conversation present
    await captureShot(page, 'panels-chat-input-buttons.png', { fullPage: true });

    // Explicit mode toggles (12a/12b/12c) with real content
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-unlocked.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-unrestricted.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-explicit-locked.png', { fullPage: false });

    // Re-open suggestions and ensure setup area is expanded for tag cloud shot
    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('/js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(600);
    // Expand setup if collapsed
    await page.evaluate(() => {
        const toggle = document.getElementById('suggestions-view-toggle');
        if (toggle && toggle.textContent?.trim() === 'Show Setup') {
            toggle.click();
        }
    });
    await sleep(800);
    await captureShot(page, 'guided-gen-suggestions-tag-cloud.png', { fullPage: false });

    // Type into search input and wait for filter to apply
    await page.click('#suggestion-search-input');
    await page.evaluate(() => {
        const input = document.getElementById('suggestion-search-input');
        if (input) input.value = '';
    });
    await page.type('#suggestion-search-input', 'horror', { delay: 50 });
    await sleep(800);
    await captureShot(page, 'guided-gen-suggestions-search-filter.png', { fullPage: false });

    // Open manage categories modal to validate rendering
    await page.evaluate(() => document.getElementById('suggestions-manage-btn')?.click());
    await sleep(800);
    await captureShot(page, 'guided-gen-manage-categories.png', { fullPage: false });
    await captureElementScreenshot(page, '#categories-builtin-list', 'guided-gen-categories-builtin-list.png', { padding: 12 });
    await page.keyboard.press('Escape');
    await sleep(300);

    await cleanupScreenshotTabs(page);
    detachSuggestionsLogger();
}

// ── Settings & Modals ───────────────────────────────────────────────────────────
// Settings modal, preferences, persona, models, keyboard shortcuts.

async function scenarioSettings(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Settings modal via button click
    try {
        await page.evaluate(() => { window.openSettingsModal?.(); });
        await page.waitForSelector('#settings-modal.open', { timeout: 5000 });
        await sleep(800);
        await captureShot(page, 'settings-settings-modal.png', { fullPage: true });
        await captureCloseUp(page, '#settings-modal', 'settings-settings-modal.png', options);

        const perfTab = await page.$('#settings-modal .settings-tab[data-tab="performance"]');
        if (perfTab) {
            await perfTab.click();
            await sleep(500);
            await captureShot(page, 'settings-settings-performance.png', { fullPage: true });
            await captureCloseUp(page, '#settings-modal', 'settings-settings-performance.png', options);
        }

        const advTab = await page.$('#settings-modal .settings-tab[data-tab="advanced"]');
        if (advTab) {
            await advTab.click();
            await sleep(500);
            await captureShot(page, 'settings-settings-advanced.png', { fullPage: true });
            await captureCloseUp(page, '#settings-modal', 'settings-settings-advanced.png', options);
        }

        await page.keyboard.press('Escape');
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Settings modal failed, skipping...');
    }

    // User preferences
    const userBtn = await page.$('#nav-user-btn');
    if (userBtn) {
        try {
            await userBtn.click();
            await sleep(300);
            const prefsBtn = await page.$('#user-menu-preferences');
            if (prefsBtn) {
                await prefsBtn.click();
                await page.waitForSelector('#user-preferences-modal.open', { timeout: 5000 });
                await sleep(500);
                await captureShot(page, 'settings-user-preferences.png', { fullPage: true });
                await captureCloseUp(page, '#user-preferences-modal', 'settings-user-preferences.png', options);
                await page.keyboard.press('Escape');
                await sleep(300);
            }
        } catch (e) {
            console.log('[CAPTURE] User preferences modal failed, skipping...');
        }
    }

    // Persona modal
    const personaBtn = await page.$('#chat-persona-btn');
    if (personaBtn) {
        try {
            await personaBtn.click();
            let personaMenuOpened = true;
            try {
                await page.waitForSelector('#chat-persona-menu:not(.hidden)', { timeout: 1200 });
            } catch {
                personaMenuOpened = false;
            }
            if (personaMenuOpened) {
                await page.waitForSelector('#chat-persona-menu .chat-persona-menu-item', { timeout: 5000 });
                await page.click('#chat-persona-menu .chat-persona-menu-item');
                await sleep(250);
                await personaBtn.click();
                await page.waitForSelector('#chat-persona-menu:not(.hidden)', { timeout: 5000 });
                const manageTemplatesBtn = await page.$('#chat-persona-edit-prompt');
                if (manageTemplatesBtn) {
                    await manageTemplatesBtn.click();
                }
            } else {
                await page.evaluate(() => {
                    document.getElementById('chat-persona-edit-prompt')?.click();
                });
            }
            await page.waitForSelector('#template-manager-modal.active', { timeout: 5000 });
            await page.waitForSelector('#template-manager-modal .template-list-item', { timeout: 5000 });
            await sleep(500);
            await page.evaluate(() => {
                document.querySelector('#template-manager-modal .template-list-item')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
                const details = document.querySelector('#persona-explicit-policies details');
                if (details) details.open = true;
            });
            await sleep(600);
            await captureShot(page, 'guided-gen-persona-modal.png', { fullPage: true });
            await captureCloseUp(page, '#template-manager-modal', 'guided-gen-persona-modal.png', options);
            await page.click('#template-manager-close');
            await page.waitForSelector('#template-manager-modal.active', { hidden: true, timeout: 5000 });
            await sleep(300);
        } catch (e) {
            console.log('[CAPTURE] Persona modal failed, skipping...');
        }
    }

    // Models modal
    try {
        await page.evaluate(() => { window.openModelsModal?.(); });
        await page.waitForSelector('#models-modal.open', { timeout: 5000 });
        await sleep(700);
        await captureShot(page, 'panels-models-modal.png', { fullPage: true });
        await captureCloseUp(page, '#models-modal', 'panels-models-modal.png', options);
        await page.click('#models-modal-close');
        await page.waitForSelector('#models-modal.open', { hidden: true, timeout: 5000 });
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Models modal failed, skipping...');
    }

    // Keyboard shortcuts via Ctrl+/
    try {
        await page.keyboard.down('Control');
        await page.keyboard.press('/');
        await page.keyboard.up('Control');
        await page.waitForSelector('#keyboard-shortcuts-modal.open', { timeout: 5000 });
        await sleep(500);
        await captureShot(page, 'panels-keyboard-shortcuts.png', { fullPage: true });
        await captureCloseUp(page, '#keyboard-shortcuts-modal', 'panels-keyboard-shortcuts.png', options);
        await page.click('#keyboard-shortcuts-modal .shortcuts-close');
        await page.waitForSelector('#keyboard-shortcuts-modal.open', { hidden: true, timeout: 5000 });
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Keyboard shortcuts modal failed, skipping...');
    }
}

// ── Chat Panels ─────────────────────────────────────────────────────────────────
// Behavior, model params, style panels, debug prompt.

async function scenarioPanels(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a chat with a short, safe conversation so panels have real content behind them
    await createFreshChat(page);
    await sendChatPrompt(page, 'Compare REST and gRPC in 4 short bullets.');
    await waitForChatComplete(page);
    await sleep(1500);

    const styleBtn = await page.$('#btn-chat-style');
    if (styleBtn) {
        await styleBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-chat-style.png', { fullPage: true });
        await captureCloseUp(page, '#chat-style-sidebar', 'panels-chat-style.png', options);
        await styleBtn.click();
        await sleep(300);
    }

    const behaviorBtn = await page.$('#btn-behavior');
    if (behaviorBtn) {
        await behaviorBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-behavior-settings.png', { fullPage: true });
        await captureCloseUp(page, '#behavior-sidebar', 'panels-behavior-settings.png', options);
        await behaviorBtn.click();
        await sleep(300);
    }

    const responseBtn = await page.$('#btn-model-params');
    if (responseBtn) {
        await responseBtn.click();
        await sleep(500);
        await captureShot(page, 'panels-model-settings.png', { fullPage: true });
        await captureCloseUp(page, '#model-params-sidebar', 'panels-model-settings.png', options);
        await responseBtn.click();
        await sleep(300);
    }

    // Send a real message so the prompt debug modal has actual content
    await sendChatPrompt(page, 'Explain the difference between TCP and UDP in 3 bullet points.');
    await waitForChatComplete(page);
    await sleep(1500);

    const debugDropdownBtn = await page.$('#btn-debug-dropdown');
    if (debugDropdownBtn) {
        try {
            await debugDropdownBtn.click();
            await page.waitForFunction(() => {
                const menu = document.getElementById('debug-dropdown-menu');
                return !!menu && menu.classList.contains('open');
            }, { timeout: 5000 });

            const debugBtn = await page.$('#btn-debug-prompt');
            if (!debugBtn) {
                throw new Error('Prompt Debug menu item not found after opening tools menu');
            }

            await debugBtn.click();
            await page.waitForSelector('#debug-prompt-modal.active', { timeout: 5000 });
            await page.waitForFunction(() => {
                const content = document.getElementById('debug-content');
                return !!content && !content.classList.contains('hidden');
            }, { timeout: 5000 });
            await sleep(500);
            await captureShot(page, 'panels-prompt-debug.png', { fullPage: true });
            await captureCloseUp(page, '#debug-prompt-modal', 'panels-prompt-debug.png', options);
            await page.keyboard.press('Escape');
            await sleep(300);
        } catch (e) {
            console.log(`[CAPTURE] Debug prompt modal failed, skipping... ${e.message}`);
        }
    }
}

// ── Dashboard ───────────────────────────────────────────────────────────────────
// Server tab, GPU section.

async function scenarioDashboard(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'server');
    await sleep(1000);
    await captureShot(page, 'settings-server-tab.png', { fullPage: true });
    await page.evaluate(() => {
        const gpu = document.getElementById('gpu-section') || document.getElementById('system-section');
        gpu?.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(500);
    await captureShot(page, 'dashboard-gpu-section.png', { fullPage: true });
}

// Validation pass for sparkline layouts and clipped section captures.
async function scenarioSparkline(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    console.log('[CAPTURE] Waiting for metrics to populate...');
    await sleep(4000);
    await captureShot(page, 'sparkline-sparkline-validate-full.png', { fullPage: true });
    await captureElementScreenshot(page, '#gpu-section', 'sparkline-sparkline-validate-gpu-section.png', { padding: 24 });
    await captureElementScreenshot(page, '#system-section', 'sparkline-sparkline-validate-system-section.png', { padding: 24 });
    await captureSparklineClips(page, 'svg.metric-sparkline, svg.hw-sparkline, svg.hw-metric-sparkline, svg.hw-clock-footer-spark');
}

// Animated capture flow for inference and hardware metric GIFs.
async function scenarioGifs(ctx, options) {
    const { page, baseUrl } = ctx;
    const fps = 10;
    const durationSec = 6;
    const totalFrames = fps * durationSec;

    await gotoApp(page, baseUrl);
    await attachToServer(page);

    if (!options.gpuOnly) {
        console.log('[CAPTURE] Capturing inference metrics GIF...');
        await switchTab(page, 'server');
        const generationPromise = startLiveGeneration();
        await sleep(1500);
        await captureFrames(page, 'inference', totalFrames, fps);
        await generationPromise;
        framesToGif('inference', join(ARTIFACTS_DIR, 'inference-metrics.gif'), fps);
        cleanupFrames();
    }

    if (!options.inferenceOnly) {
        console.log('[CAPTURE] Capturing GPU/system metrics GIF...');
        await switchTab(page, 'server');
        await page.evaluate(() => {
            const target = document.getElementById('gpu-section') || document.getElementById('system-section');
            target?.scrollIntoView({ behavior: 'instant', block: 'start' });
        });
        await sleep(1200);
        await captureFrames(page, 'gpu', totalFrames, fps);
        framesToGif('gpu', join(ARTIFACTS_DIR, 'gpu-metrics.gif'), fps);
        cleanupFrames();
    }
}

// Sidebar features capture: expanded panel, collapsed strip, FTS search, context menu
// ── Sidebar ─────────────────────────────────────────────────────────────────────
// Chat sidebar, FTS search, context menu, name filter.

async function scenarioSidebar(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        await attachToServer(page);
    }

    // Create multiple chats with different names for grouping
    // Directly show the chat page and session panel (nav state may not be initialized)
    await page.evaluate(async () => {
        // Show chat page
        const pages = document.querySelectorAll('.page');
        pages.forEach(p => p.classList.remove('active'));
        const chatPage = document.getElementById('page-chat');
        if (chatPage) chatPage.classList.add('active');
        // Show session panel
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.add('visible');
        // Render sidebar items (normally done by switchTab)
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await page.waitForSelector('#chat-sessions-panel', { visible: true });
    await sleep(500);

    // Create multiple chats with distinct content for sidebar and FTS search
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        for (let i = 0; i < 5; i++) {
            addChatTab();
        }
    });
    await sleep(500);

    // Rename chats and seed them with content that will be searchable
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        const tabs = chat.tabs;

        if (tabs[0]) {
            tabs[0].name = 'Noir Scene';
            tabs[0].messages = [
                { role: 'user', content: 'Write a noir scene in progress.' },
                { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
            ];
            tabs[0].pinned = true;
        }
        if (tabs[1]) {
            tabs[1].name = 'Debug Session';
            tabs[1].messages = [
                { role: 'user', content: 'Help me debug a slow HTTP endpoint.' },
                { role: 'assistant', content: 'Start by profiling the request duration, then inspect database queries, external service calls, and any retries or timeouts in the logs.' },
            ];
        }
        if (tabs[2]) {
            tabs[2].name = 'CI Pipeline';
            tabs[2].messages = [
                { role: 'user', content: 'Outline a simple CI pipeline for a Rust backend.' },
                { role: 'assistant', content: 'Use GitHub Actions: run cargo fmt, cargo clippy, cargo test, then build a release binary and upload artifacts.' },
            ];
        }
        if (tabs[3]) {
            tabs[3].name = 'Recipe Ideas';
            tabs[3].messages = [
                { role: 'user', content: 'Suggest 3 quick dinner recipes using chicken and rice.' },
                { role: 'assistant', content: '- Chicken and rice skillet with vegetables.\n- One-pot lemon herb chicken rice.\n- Stir-fried chicken with soy-ginger rice.' },
            ];
        }
        if (tabs[4]) {
            tabs[4].name = 'GPU Monitoring';
            tabs[4].messages = [
                { role: 'user', content: 'How can I monitor GPU temperature and utilization from the CLI?' },
                { role: 'assistant', content: 'Use tools like nvidia-smi, nvtop, or custom scripts reading from /sys/class/thermal and GPU management APIs.' },
            ];
        }

        for (const t of tabs) t.updated_at = Date.now();
        renderChatTabs();
        renderChatMessages();
        renderChatSessionsSidebar();
    });
    await sleep(800);

    // Persist all tabs to database so FTS search can find them
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        for (const tab of chat.tabs) {
            const tabPayload = {
                id: tab.id,
                name: tab.name || '',
                system_prompt: tab.system_prompt || '',
                ai_name: tab.ai_name || null,
                user_name: tab.user_name || null,
                explicit_level: tab.explicit_level || 0,
                active_template_id: tab.active_template_id || null,
                auto_compact: tab.auto_compact !== false,
                auto_compact_summarize: tab.auto_compact_summarize !== false,
                compact_mode: tab.compact_mode || 'summarize',
                compact_threshold: tab.compact_threshold || 0.8,
                model_params: tab.model_params || {},
                context_notes: tab.context_notes || [],
                sidebar_width: tab.sidebar_width || 320,
                tab_order: tab.tab_order || 0,
                pinned: tab.pinned || false,
                last_ctx_pct: tab.last_ctx_pct || null,
                total_input_tokens: tab.total_input_tokens || 0,
                total_output_tokens: tab.total_output_tokens || 0,
                created_at: tab.created_at || Date.now(),
                updated_at: Date.now(),
                messages: [],
            };
            await fetch('/api/chat/tabs', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(tabPayload),
            });
            if (tab.messages && tab.messages.length > 0) {
                const msgPayload = {
                    messages: tab.messages.map((m, idx) => ({
                        tab_id: tab.id,
                        role: m.role || 'user',
                        content: m.content || '',
                        timestamp_ms: m.timestamp_ms || Date.now(),
                        seq: idx,
                    })),
                };
                await fetch(`/api/chat/tabs/${tab.id}/messages`, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify(msgPayload),
                });
            }
        }
    });
    await sleep(1500);

    // Ensure sidebar is rendered with all chats
    await page.evaluate(async () => {
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await sleep(300);

    // Capture expanded sidebar with multiple chats
await captureShot(page, 'sidebar-sidebar-expanded.png', { fullPage: true });

    // Capture sidebar element detail

    await captureElementScreenshot(page, '#chat-sessions-panel', 'sidebar-sidebar-panel-detail.png', { padding: 16 });

    // Collapse the sidebar (directly set class for reliability)
    await page.evaluate(() => {
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.add('collapsed');
        localStorage.setItem('csp-collapsed', 'true');
    });
    await sleep(500);

    // Capture collapsed strip
    await captureShot(page, 'sidebar-sidebar-collapsed.png', { fullPage: true });

    // Capture collapsed strip detail
    await captureElementScreenshot(page, '#csp-collapsed-strip', 'sidebar-sidebar-collapsed-detail.png', { padding: 12 });

    // Expand again
    await page.evaluate(() => {
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.remove('collapsed');
        localStorage.setItem('csp-collapsed', 'false');
    });
    await sleep(300);

    // Re-render sidebar after expanding (items may not be visible when collapsed)
    await page.evaluate(async () => {
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        renderChatSessionsSidebar();
    });
    await sleep(1000);

    // Test FTS search: open search mode, query across multiple chats, capture results
    const searchBtn = await page.$('.csp-search-btn');
    if (searchBtn) {
        console.log('[CAPTURE] FTS search button found, opening search mode...');
        await page.evaluate(async () => {
            const { openSearch } = await import('/js/features/chat-search.js');
            openSearch();
        });
        await sleep(600);

        const searchInput = await page.$('#csp-search-input');
        if (searchInput) {
            console.log('[CAPTURE] Search input found, typing query that matches multiple chats...');
            // "rain" matches Noir Scene; "debug" matches Debug Session; "rain" is enough for a clear demo.
            await page.type('#csp-search-input', 'rain');
            await sleep(1200);

            // Ensure results area is visible
            const resultsVisible = await page.evaluate(() => {
                const results = document.querySelector('.csp-search-results');
                return results ? (results.style.display !== 'none') : false;
            });
            console.log('[CAPTURE] Search results visible:', resultsVisible);

            // Capture full-page with search mode and results
            await captureShot(page, 'sidebar-fts-search-active.png', { fullPage: true });

            // Capture close-up of search results
            const searchResults = await page.$('.csp-search-results');
            if (searchResults) {
                await captureElementScreenshot(page, '.csp-search-results', 'sidebar-fts-search-results.png', { padding: 12 });
                await captureCloseUp(page, '.csp-search-results', 'sidebar-fts-search-results.png', options);
            }

            // Close search
            await page.keyboard.press('Escape');
            await sleep(300);
        } else {
            console.log('[CAPTURE] Search input not found after opening search mode');
        }
    } else {
        console.log('[CAPTURE] FTS search button not found; skipping FTS search captures');
    }

    // Test context menu: hover over a chat item and click the "..." button
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatSessionsSidebar } = await import('/js/features/chat-sessions-sidebar.js');
        // Switch to first tab to make it active
        const tab = chat.tabs[0];
        if (tab) {
            await switchChatTab(tab.id);
            renderChatSessionsSidebar();
        }
    });
    await sleep(500);

    // Hover over first item to reveal action buttons
    const hoverDebug = await page.evaluate(() => {
        const item = document.querySelector('.csp-item');
        const panel = document.getElementById('chat-sessions-panel');
        return {
            itemExists: !!item,
            itemVisible: item ? getComputedStyle(item).display !== 'none' : false,
            panelVisible: panel ? panel.classList.contains('visible') : false,
            panelCollapsed: panel ? panel.classList.contains('collapsed') : false,
        };
    });
    console.log('[CAPTURE] Hover debug:', JSON.stringify(hoverDebug));
    // Actually, the hover is handled by CSS :hover pseudo-class, not a class
    // Need to use puppeteer's hover instead
    const firstItem = await page.$('.csp-item');
    if (firstItem) {
        // Try scrolling the sidebar to ensure the item is in the viewport
        await firstItem.evaluate(el => el.scrollIntoView({ behavior: 'instant', block: 'center' }));
        await sleep(200);
        // Try hovering using mouse.move
        const box = await firstItem.boundingBox();
        if (box) {
            await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
            await sleep(500);
        }
    }

    // Click the "..." more button to open context menu (using JS to bypass CSS hover)
    await page.evaluate(() => {
        const item = document.querySelector('.csp-item');
        if (item) {
            const moreBtn = item.querySelector('button[data-action="more"]');
            if (moreBtn) {
                // Show the actions container (normally shown on hover)
                const actions = item.querySelector('.csp-item-actions');
                if (actions) actions.style.display = 'flex';
                // Click the more button
                moreBtn.click();
            }
        }
    });
    await sleep(500);
    await captureShot(page, 'chat-context-menu.png', { fullPage: true });
    // Capture context menu detail
    const menu = await page.$('.csp-context-menu');
    if (menu) {
        await captureElementScreenshot(page, '.csp-context-menu', 'chat-context-menu-detail.png', { padding: 12 });
    }
    // Close menu
    await page.keyboard.press('Escape');
    await sleep(300);

    // Test search filter (name filter, not FTS)
    await page.type('#csp-search', 'Noir');
    await sleep(500);
    await captureShot(page, 'sidebar-sidebar-name-filter.png', { fullPage: true });

    // Clear filter
    await page.evaluate(() => {
        document.getElementById('csp-search').value = '';
        document.getElementById('csp-search').dispatchEvent(new Event('input'));
    });
    await sleep(300);

    await cleanupScreenshotTabs(page);
}

async function scenarioTls(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Open Settings modal
    try {
        await page.evaluate(() => { window.openSettingsModal?.(); });
        await page.waitForSelector('#settings-modal.open', { timeout: 5000 });
        await sleep(800);

        // Switch to Security tab
        const securityTab = await page.$('#settings-modal .settings-tab[data-tab="security"]');
        if (!securityTab) {
            console.log('[CAPTURE] Security tab not found; skipping TLS scenario');
            await page.keyboard.press('Escape');
            return;
        }

        await securityTab.click();
        await sleep(900);

        // Helper: log visibility of key Security elements
        const logCertsState = async () => {
            const state = await page.evaluate(() => {
                const pane = document.getElementById('settings-security');
                const tlsStatus = document.getElementById('tls-status-text');
                const pills = document.querySelectorAll('.cert-mode-pill');
                const acmeFqdn = document.getElementById('acme-fqdn');
                const acmeSection = document.getElementById('acme-credentials-section');
                const customCertPath = document.getElementById('tls-custom-cert-path');
                const customKeyPath = document.getElementById('tls-custom-key-path');
                const btnApplyCustom = document.getElementById('btn-apply-custom-cert');
                return {
                    paneExists: !!pane,
                    tlsStatusExists: !!tlsStatus,
                    pillsCount: pills.length,
                    acmeFqdnExists: !!acmeFqdn,
                    acmeSectionExists: !!acmeSection,
                    customCertPathExists: !!customCertPath,
                    customKeyPathExists: !!customKeyPath,
                    btnApplyCustomExists: !!btnApplyCustom,
                    paneScrollHeight: pane?.scrollHeight ?? null,
                };
            });
            console.log('[CAPTURE] TLS Certificates state:', JSON.stringify(state));
            return state;
        };

        // Helper: dismiss all toasts to keep screenshots clean
        const dismissToasts = async () => {
            await page.evaluate(() => {
                const toasts = document.querySelectorAll('[data-toast], .toast, .settings-toast, [role="status"]');
                toasts.forEach(t => {
                    const closeBtn = t.querySelector('[aria-label="Close"], button, .toast-close');
                    if (closeBtn) closeBtn.click();
                    else t.remove();
                });
            });
            await sleep(300);
        };

        // Helper: scroll a target element to the top of the pane so it appears as high as possible
        const scrollToTopOfPane = async (selector) => {
            await page.evaluate((sel) => {
                const pane = document.getElementById('settings-security');
                const target = document.querySelector(sel);
                if (!pane || !target) return;
                const paneRect = pane.getBoundingClientRect();
                const targetRect = target.getBoundingClientRect();
                const scrollDelta = targetRect.top - paneRect.top;
                pane.scrollTop += scrollDelta;
            }, selector);
            await sleep(500);
        };

        // Helper: select a certificate mode via pill
        const selectCertMode = async (mode) => {
            await page.evaluate((m) => {
                const pill = document.querySelector(`.cert-mode-pill[data-mode="${m}"]`);
                if (pill) pill.click();
            }, mode);
            await sleep(600);
        };

        await logCertsState();

        // 1) Security & Certificates tab overview (top area, default mode)
        await page.evaluate(() => {
            const pane = document.getElementById('settings-security');
            if (pane) pane.scrollTo({ top: 0, behavior: 'instant' });
        });
        await sleep(300);
        await dismissToasts();
        await captureShot(page, 'tls-certificates-tab.png', { fullPage: true });
        await captureCloseUp(page, '#settings-modal', 'tls-certificates-tab.png', options);

        // 2) No TLS mode: select "No HTTPS" pill, scroll to Certificates card, capture
        await selectCertMode('none');
        await scrollToTopOfPane('#cert-mode-none');
        await dismissToasts();
        await logCertsState();
        await captureShot(page, 'tls-mode-no-tls.png', { fullPage: true });

        // 3) Self-signed mode: select "Self-Signed" pill, scroll to Certificates card, capture
        await selectCertMode('self-signed');
        await scrollToTopOfPane('#cert-mode-self-signed');
        await dismissToasts();
        await logCertsState();
        await captureShot(page, 'tls-mode-self-signed.png', { fullPage: true });

        // 4) Custom certificate mode: select "Bring Your Own Key" pill, fill paths, apply, capture
        await selectCertMode('custom');
        const customCertPath = await page.$('#tls-custom-cert-path');
        const customKeyPath = await page.$('#tls-custom-key-path');
        const btnApplyCustom = await page.$('#btn-apply-custom-cert');
        if (customCertPath && customKeyPath && btnApplyCustom) {
            await customCertPath.type('/path/to/cert.pem', { delay: 10 });
            await customKeyPath.type('/path/to/key.pem', { delay: 10 });
            await sleep(400);
            await btnApplyCustom.click();
            await sleep(600);
            await scrollToTopOfPane('#tls-custom-cert-path');
            await dismissToasts();
            await logCertsState();
            await captureShot(page, 'tls-mode-custom.png', { fullPage: true });
        } else {
            console.log('[CAPTURE] Custom cert fields not fully present; skipping Custom cert shot');
        }

        // 5) ACME mode: select "Let's Encrypt (ACME)" pill, scroll so ACME is high, capture
        await selectCertMode('acme');
        await page.evaluate(() => {
            const acmeFqdn = document.getElementById('acme-fqdn');
            if (acmeFqdn) {
                const pane = document.getElementById('settings-security');
                acmeFqdn.scrollIntoView({ behavior: 'instant', block: 'start' });
                if (pane) {
                    pane.scrollTop += 6;
                }
            }
        });
        await sleep(600);
        await dismissToasts();

        const acmeVisible = await page.evaluate(() => {
            const acmeFqdn = document.getElementById('acme-fqdn');
            if (!acmeFqdn) return false;
            const rect = acmeFqdn.getBoundingClientRect();
            return rect.top >= 0 && rect.bottom <= window.innerHeight;
        });
        console.log('[CAPTURE] ACME section visible:', acmeVisible);

        if (acmeVisible) {
            // Full view with ACME card high and title visible
            await captureShot(page, 'tls-mode-acme-full.png', { fullPage: true });

            // Show "Other" provider input, keep ACME high, capture
            await page.evaluate(() => {
                const select = document.getElementById('acme-dns-provider');
                const otherOption = Array.from(select?.options || [])
                    .find(o => o.value === '__other__');
                if (otherOption) {
                    select.value = '__other__';
                    select.dispatchEvent(new Event('change'));
                }
            });
            await sleep(600);
            await page.evaluate(() => {
                const acmeFqdn = document.getElementById('acme-fqdn');
                if (acmeFqdn) {
                    const pane = document.getElementById('settings-security');
                    acmeFqdn.scrollIntoView({ behavior: 'instant', block: 'start' });
                    if (pane) {
                        pane.scrollTop += 6;
                    }
                }
            });
            await sleep(400);
            await dismissToasts();

            const customWrapVisible = await page.evaluate(() => {
                const el = document.getElementById('acme-provider-custom-wrap');
                if (!el) return false;
                const style = window.getComputedStyle(el);
                return style.display !== 'none';
            });
            if (customWrapVisible) {
                await captureShot(page, 'tls-acme-other-provider.png', { fullPage: true });
            }
        } else {
            console.log('[CAPTURE] ACME section not visible after scroll; capturing anyway');
            await captureShot(page, 'tls-mode-acme-full.png', { fullPage: true });
        }

        // 6) Database Administration section
        await page.evaluate(() => {
            const dbPanel = document.getElementById('db-admin-panel');
            if (dbPanel) {
                dbPanel.scrollIntoView({ behavior: 'instant', block: 'start' });
                const pane = document.getElementById('settings-security');
                if (pane) {
                    pane.scrollTop += 6;
                }
            }
        });
        await sleep(600);
        await dismissToasts();

        const dbVisible = await page.evaluate(() => {
            const dbPanel = document.getElementById('db-admin-panel');
            if (!dbPanel) return false;
            const rect = dbPanel.getBoundingClientRect();
            return rect.top >= 0 && rect.bottom <= window.innerHeight;
        });
        console.log('[CAPTURE] DB admin section visible:', dbVisible);

        if (dbVisible) {
            await captureShot(page, 'tls-db-admin-section.png', { fullPage: true });
        } else {
            console.log('[CAPTURE] DB admin section not visible; capturing anyway');
            await captureShot(page, 'tls-db-admin-section.png', { fullPage: true });
        }

        await page.keyboard.press('Escape');
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] TLS/Certificates scenario failed:', e.message);
    }
}

async function scenarioFilebrowser(ctx, options) {
    const { page, baseUrl } = ctx;

    try {
        await gotoApp(page, baseUrl);

        // Open Settings modal
        const settingsBtn = await page.$('#sidebar-btn-settings');
        if (!settingsBtn) {
            console.log('[CAPTURE] Settings button not found');
            return;
        }
        await settingsBtn.click();
        await sleep(600);

        // Ensure Settings modal is visible
        const settingsModal = await page.$('#settings-modal');
        if (!settingsModal) {
            console.log('[CAPTURE] Settings modal not found');
            return;
        }

        // Switch to Advanced tab
        const advancedTab = await page.$('#settings-tab-advanced');
        if (!advancedTab) {
            console.log('[CAPTURE] Advanced tab not found');
            return;
        }
        await advancedTab.click();
        await sleep(600);

        // Open Config modal from Advanced tab
        const openConfigBtn = await page.$('#settings-open-config-btn');
        if (!openConfigBtn) {
            console.log('[CAPTURE] Open Config button not found');
            return;
        }
        await openConfigBtn.click();
        await sleep(800);

        // Ensure Config modal is visible
        const configModal = await page.$('#config-modal');
        if (!configModal) {
            console.log('[CAPTURE] Config modal not found');
            return;
        }

        // Scroll to llama-server executable section
        await page.evaluate(() => {
            const section = document.querySelector('#config-modal .modal-section');
            if (section) {
                section.scrollIntoView({ behavior: 'instant', block: 'start' });
            }
        });
        await sleep(400);

        // Capture the llama-server executable section with Browse button visible
        await captureShot(page, 'filebrowser-config-browse-btn.png', { fullPage: true });

        // Click the Browse button for llama-server executable
        const browseBtn = await page.$('#config-browse-server-path');
        if (!browseBtn) {
            console.log('[CAPTURE] Browse button for server path not found');
            return;
        }
        await browseBtn.click();
        await sleep(1000);

        // Ensure file browser modal is visible
        const fileBrowserModal = await page.$('#file-browser-modal');
        if (!fileBrowserModal) {
            console.log('[CAPTURE] File browser modal not found');
            return;
        }

        // Capture the file browser modal open
        await captureShot(page, 'filebrowser-modal-open.png', { fullPage: true });

        // Close file browser modal
        await page.keyboard.press('Escape');
        await sleep(300);

        // Close Config modal
        await page.keyboard.press('Escape');
        await sleep(300);

        // Close Settings modal
        await page.keyboard.press('Escape');
        await sleep(300);
    } catch (e) {
        console.log('[CAPTURE] Filebrowser scenario failed:', e.message);
    }
}

async function scenarioSmoke({ page, baseUrl }, options) {
    const criticalPatterns = [
        'import',
        'Cannot set properties of (null|undefined)',
        'is not defined',
        'TypeError',
        'SyntaxError',
        'Failed to fetch module',
        'Failed to load module script',
    ];

    const logs = {
        error: [],
        warn: [],
    };

    page.on('console', msg => {
        const level = msg.type();
        const text = msg.text();
        if (level === 'error') logs.error.push(text);
        if (level === 'warn') logs.warn.push(text);
    });

    page.on('pageerror', err => {
        logs.error.push(err.message || String(err));
    });

    await gotoApp(page, baseUrl);

    if (!options.noAttach) {
        await attachToServer(page);
    } else {
        await waitForMonitor(page);
    }

    await sleep(2000);

    const hasCritical = logs.error.some(line =>
        criticalPatterns.some(p => line.includes(p))
    );

    console.log('[SMOKE] Console warnings:', logs.warn.length);
    console.log('[SMOKE] Console errors:', logs.error.length);

    if (logs.error.length > 0) {
        console.log('[SMOKE] Error details:');
        logs.error.forEach(e => console.log('  -', e));
    }

    if (hasCritical) {
        throw new Error(
            'SMOKE FAIL: critical console errors detected on startup. ' +
            'Check for import/export issues, missing symbols, or runtime failures.'
        );
    }

    console.log('[SMOKE] PASS: no critical console errors on startup.');
}

const SCENARIOS = {
    // Core
    welcome: scenarioWelcome,
    chat: scenarioChat,
    // Chat features
    'guided-gen': scenarioGuidedGen,
    sidebar: scenarioSidebar,
    // Configuration
    settings: scenarioSettings,
    tls: scenarioTls,
    filebrowser: scenarioFilebrowser,
    panels: scenarioPanels,
    dashboard: scenarioDashboard,
    // Validation
    sparkline: scenarioSparkline,
    gifs: scenarioGifs,
    smoke: scenarioSmoke,
};

export async function runCli({ scenario: forcedScenario = null, argv = process.argv.slice(2) } = {}) {
    const options = parseArgs(argv);
    if (options.help) {
        printUsage();
        return;
    }
    if (options.listScenarios) {
        Object.keys(SCENARIOS).forEach(name => console.log(name));
        return;
    }

    // Default to the welcome flow so an unqualified invocation still succeeds
    // without requiring a remote attach.
    const scenarioName = forcedScenario || options.scenario || 'welcome';
    const scenario = SCENARIOS[scenarioName];
    if (!scenario) {
        throw new Error(`Unknown scenario "${scenarioName}". Use --list-scenarios.`);
    }

    seedConfig();
    const port = await findAvailablePort();
    console.log(`[CAPTURE] Spawning llama-monitor on port ${port} for scenario "${scenarioName}"...`);

    let server = null;
    let browser = null;

    try {
        server = await spawnLlamaMonitor(port);
        const launched = await launchBrowser(options.viewport);
        browser = launched.browser;
        const page = launched.page;
        await scenario({ page, baseUrl: server.url, browser }, options);
        console.log(`[CAPTURE] Scenario "${scenarioName}" complete.`);
    } catch (err) {
        console.error(err.stack || err.message);
        process.exitCode = 1;
    } finally {
        cleanupFrames();
        if (browser) await browser.close();
        await cleanupServer(server);
        cleanupTempHome();
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    await runCli();
}
