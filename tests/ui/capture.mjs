/**
 * Consolidated screenshot and GIF capture harness for llama-monitor.
 *
 * THIS SCRIPT MUST ONLY BE RUN SEQUENTIALLY.
 * - Do not run multiple scenarios in parallel (no background tasks, no && chains, no &).
 * - Each scenario launches its own llama-monitor instance, uses its own temp config,
 *   and may attach to the remote llama-server. Running in parallel causes port conflicts,
 *   attach-timeouts, and race conditions in the capture harness.
 *
 * This is the single entrypoint for repo-owned visual capture automation.
 * Prefer adding new scenarios here instead of creating new one-off scripts.
 *
 * Output directories:
 *   docs/screenshots/artifacts/  — ALL capture output lands here (auto-created).
 *   docs/screenshots/            — ONLY files referenced by README / docs live here.
 *                                  Do NOT copy from artifacts/ manually; promote
 *                                  individual files by hand after reviewing them.
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
// Set RUNNING_PORT to connect to an already-running llama-monitor (e.g. your production instance
// with a remote agent connected). When set, no binary is spawned and no temp config is seeded.
// Example: RUNNING_PORT=8080 node tests/ui/capture.mjs --scenario dashboard
const RUNNING_PORT = process.env.RUNNING_PORT ? parseInt(process.env.RUNNING_PORT, 10) : null;
const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';
const BINARY_PATH = join(ROOT_DIR, 'target/release/llama-monitor');
const CAPTURE_FORM_AUTH = process.env.SCREENSHOT_FORM_AUTH || 'admin:secret123';

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
    welcome          Welcome screen, auth shell, and spawn wizard button (no attach required)
    chat             Chat, telemetry, logs

  Chat Features
    guided-gen       Suggestions, quick guide, director, surprise, explicit mode
    sidebar          Sidebar, FTS search flyout, context menu, title filter
    chat-history-qa  History Q&A panel (ask questions about conversation)

  Models and Presets
    models-v2        Models modal: discovery summary, third-party scan, HF download panel
    preset-editor    Preset editor: model/context, GPU, and advanced tabs

  Configuration
    settings         Settings modal, preferences, persona, models, shortcuts
    tls              TLS modes and ACME (Certificates tab, each TLS mode, custom certs, ACME config)
    filebrowser      File browser modal (Browse buttons in Config modal, modal open)
    panels           Chat config panels (behavior, model, style, debug)
    dashboard        Server tab, GPU section

  Spawn Wizard
    spawn-wizard           Wizard step 1 profiles, step 2 HF discover/community picks/quant advisor, step 3 VRAM
    spawn-wizard-gif       Animated GIF walking through all spawn wizard steps (1→2→3→4→5→6)
    spawn-wizard-hf-download  HF download panel: idle options and simulated progress

  Performance & Updates
    tune-panel       Performance benchmark panel on server tab
    llama-updater    Llama-server binary update pill, version modal with release notes, and app release notes

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
  SCREENSHOT_PORT=8896 node tests/ui/capture.mjs --scenario spawn-wizard --no-attach
  SCREENSHOT_PORT=8897 node tests/ui/capture.mjs --scenario spawn-wizard-gif --no-attach
  SCREENSHOT_PORT=8900 node tests/ui/capture.mjs --scenario tune-panel
  SCREENSHOT_PORT=8901 node tests/ui/capture.mjs --scenario llama-updater
  SCREENSHOT_PORT=8902 node tests/ui/capture.mjs --scenario chat-history-qa
  RUNNING_PORT=8080 node tests/ui/capture.mjs --scenario dashboard
  RUNNING_PORT=8080 node tests/ui/capture.mjs --scenario gifs --gpu-only

Note: RUNNING_PORT connects to an already-running llama-monitor (e.g. your production instance
with a remote agent reporting GPU data). No binary is spawned; no temp config is seeded.
`);
}

function seedConfig() {
    // Copy encryption-key first so encrypted values in ui-settings.json (e.g. remote_agent_token)
    // can be decrypted — without it the ephemeral instance generates a new key and auth fails.
    // Copy ssh-known-hosts.json so the agent SSH host key check passes (prevents enrollment block).
    // Copy hf-token so HF API searches use auth (higher rate limits, better trending results).
    const filesToCopy = ['encryption-key', 'ssh-known-hosts.json', 'hf-token', 'ui-settings.json', 'presets.json', 'gpu-env.json', 'community-picks.json'];
    for (const filename of filesToCopy) {
        const source = join(REAL_APP_CONFIG_DIR, filename);
        const destination = join(TEMP_APP_CONFIG_DIR, filename);
        if (fs.existsSync(source)) {
            fs.copyFileSync(source, destination);
        }
    }

    // Copy the certs/ directory (including remote-cas/) so the remote agent https_client starts
    // with the correct trust anchors. Without this it's built once at startup without the CA cert
    // and the mTLS agent connection always fails in that session.
    const sourceCerts = join(REAL_APP_CONFIG_DIR, 'certs');
    const destCerts = join(TEMP_APP_CONFIG_DIR, 'certs');
    if (fs.existsSync(sourceCerts)) {
        fs.cpSync(sourceCerts, destCerts, { recursive: true });
    }

    // If no community-picks.json exists in real config, use the bundled example fixture.
    const cpDest = join(TEMP_APP_CONFIG_DIR, 'community-picks.json');
    if (!fs.existsSync(cpDest)) {
        const cpExample = join(ROOT_DIR, 'docs/reference/community-picks-example.json');
        if (fs.existsSync(cpExample)) {
            fs.copyFileSync(cpExample, cpDest);
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

async function spawnLlamaMonitor(port, extraArgs = []) {
    const proc = spawn(BINARY_PATH, ['--port', String(port), '--headless', ...extraArgs], {
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

async function captureAuthShell(port, viewport = DEFAULT_VIEWPORT) {
    console.log('[CAPTURE] Capturing form-auth welcome shell...');
    const authServer = await spawnLlamaMonitor(port, ['--form-auth', CAPTURE_FORM_AUTH]);
    let authBrowser = null;

    try {
        const launched = await launchBrowser(viewport);
        authBrowser = launched.browser;
        const authPage = launched.page;
        await gotoApp(authPage, authServer.url);
        await authPage.waitForSelector('#auth-shell:not(.hidden)', { visible: true, timeout: 15000 });
        await captureShot(authPage, 'welcome-auth-shell.png', { fullPage: true });
    } finally {
        if (authBrowser) await authBrowser.close();
        await cleanupServer(authServer);
    }
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
    const fullPage = options.fullPage ?? true;

    // Non-full-page captures are disabled by default.
    // To temporarily enable for debugging, change this guard or remove fullPage: false.
    if (!fullPage) {
        console.log(`[CAPTURE] Skipped non-full-page: ${filename}`);
        return;
    }

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
        '-vf', 'scale=900:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5',
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
    // Shot 1: the arrival screen with both cards visible.
    await captureShot(page, 'welcome-welcome.png', { fullPage: true });

    // Shot 2: click "New Server Wizard" and capture step 0 of the wizard so the
    // two screenshots tell a clear before→after story.
    const spawnBtn = await page.$('#setup-spawn-wizard-btn');
    if (spawnBtn) {
        try {
            await spawnBtn.click();
        } catch {
            // Fallback click via DOM when Puppeteer thinks it’s not clickable.
            await page.evaluate(() => {
                (document.getElementById('setup-spawn-wizard-btn')
                    || document.querySelector('#view-setup button:has-text("New Server Wizard")'))
                    ?.click();
            });
        }
        await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 }).catch(() => {
            console.log('[CAPTURE] Wizard overlay did not open; falling back to welcome shot');
        });
        // Hide the binary prereq banner for a clean wizard shot.
        await page.evaluate(() => {
            const banner = document.getElementById('wizard-binary-prereq');
            if (banner) banner.style.display = 'none';
        });
        await sleep(500);
        await captureShot(page, 'welcome-spawn-wizard-btn.png', { fullPage: true });
        // Close wizard before proceeding.
        await page.keyboard.press('Escape');
        await sleep(300);
    } else {
        console.log('[CAPTURE] #setup-spawn-wizard-btn not found; skipping wizard-open shot');
    }

    const authPort = await findAvailablePort(DEFAULT_PORT + 1);
    await captureAuthShell(authPort, options.viewport);
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
    // File menu
    const fileBtn = await page.$('#chat-file-btn');
    if (fileBtn) {
        try {
            await fileBtn.click();
            await page.waitForSelector('#chat-file-menu:not(.hidden)', { timeout: 2000 });
            await captureCloseUp(page, '#chat-file-menu', 'chat-file-menu.png', options);
            await page.keyboard.press('Escape');
            await sleep(300);
        } catch {
            console.log('[CAPTURE] File menu open failed, skipping...');
        }
    }

    // Focus mode
    try {
        await page.locator('#chat-focus-mode-btn').click();
        await page.waitForFunction(() => document.body.classList.contains('chat-focus-mode'), { timeout: 2000 });
        await sleep(400);
        await captureShot(page, 'chat-focus-mode.png', { fullPage: true });
        await captureCloseUp(page, '#focus-mode-exit-pill', 'chat-focus-mode-pill.png', options);
        await page.locator('#focus-mode-exit-beacon').click();
        await page.waitForFunction(() => !document.body.classList.contains('chat-focus-mode'), { timeout: 2000 });
        await sleep(300);
    } catch {
        console.log('[CAPTURE] Focus mode capture failed, skipping...');
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
    // Use a short prompt to set context (best-effort; continue if server is slow)
    await sendChatPrompt(page, 'Brainstorm 3 product names for a CLI tool that monitors GPUs.');
    try {
        await waitForChatComplete(page);
    } catch (e) {
        console.log(`[CAPTURE] guided-gen: chat complete timed out, continuing with UI captures... ${e.message}`);
    }
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
    if (options.closeUp) {
        await captureElementScreenshot(page, '#categories-builtin-list', 'guided-gen-categories-builtin-list.png', { padding: 12 });
    }
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

// ── Models modal ─────────────────────────────────────────────────────────────
// Seeds fake GGUF files so the models modal has real cards to show.

// scenarioModels is called with a pre-seeded models dir passed via --models-dir
// CLI flag. The runCli() path seeds fake .gguf files and passes the dir when
// spawning the server — see the 'models' entry in SCENARIOS for the wrapper.
async function scenarioModels(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }
    // Open models modal via the global helper
    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(1500);
    await captureShot(page, 'panels-models-modal.png', { fullPage: true });
}

async function scenarioModelsV2(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    // 1. Open models modal and capture initial discovery view.
    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(1500);
    await captureShot(page, 'models-discovery-overview.png', { fullPage: true });

    // 2. Show third-party scan section if present (expand for richer context).
    await page.evaluate(() => {
        const section = document.querySelector('#models-modal .third-party-section');
        if (section) {
            section.setAttribute('open', '');
            section.style.maxHeight = 'none';
        }
    });
    await sleep(500);
    await captureShot(page, 'models-third-party-scan.png', { fullPage: true });

    // 3. Simulate HF file selection and show HF download panel inside models modal.
    await page.evaluate(() => {
        const panel = document.getElementById('mm-hf-download-panel');
        if (!panel) return;
        const idle = document.getElementById('mm-hf-dlp-idle');
        const fileName = document.getElementById('mm-hf-dlp-file-name');
        const destPath = document.getElementById('mm-hf-dlp-dest-path');

        if (idle && panel) {
            panel.style.display = 'block';
            panel.style.maxHeight = 'none';
            if (fileName) fileName.textContent = 'llama-3.1-8b-instruct-Q4_K_M.gguf';
            if (destPath) destPath.textContent = '~/.config/llama-monitor/models/';
        }
    });
    await sleep(500);
    await captureShot(page, 'models-hf-download-panel.png', { fullPage: true });
}

async function scenarioPresetEditor(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    // 1. Open preset editor via "New"
    const newBtn = await page.$('#preset-new-btn');
    if (!newBtn) {
        console.log('[CAPTURE] #preset-new-btn not found; skipping preset-editor scenario');
        return;
    }
    await newBtn.click();
    await page.waitForSelector('#preset-modal', { timeout: 6000 });
    await sleep(800);

    // Capture Model+Context section (default active)
    await captureShot(page, 'preset-editor-model-tab.png', { fullPage: true });

    // 2. Capture GPU section
    await page.evaluate(() => {
        const gpuNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="gpu"]');
        if (gpuNav) gpuNav.click();
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-gpu-tab.png', { fullPage: true });

    // 3. Capture Advanced section
    await page.evaluate(() => {
        const advNav = document.querySelector('#preset-modal .preset-editor-nav [data-section="advanced"]');
        if (advNav) advNav.click();
    });
    await sleep(500);
    await captureShot(page, 'preset-editor-advanced-tab.png', { fullPage: true });

    // 4. Close preset modal
    await page.evaluate(() => {
        const close = document.getElementById('preset-modal-close');
        if (close) close.click();
    });
    await sleep(300);
}

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
    // Wait for agent first poll (2s interval) + some render time.
    await sleep(3500);
    // Scroll to top, capture the control bar + inference section viewport.
    await page.evaluate(() => {
        const pg = document.querySelector('.page.active');
        if (pg) pg.scrollTop = 0;
    });
    await sleep(300);
    await captureShot(page, 'dashboard-inference-section.png');
    await captureShot(page, 'settings-server-tab.png', { fullPage: true });

    // Wait up to 6s for hardware data to arrive (remote agent dependent).
    const gpuVisible = await page.evaluate(() => new Promise(resolve => {
        const check = () => {
            const gpu = document.getElementById('gpu-section');
            const sys = document.getElementById('system-section');
            if ((gpu && gpu.style.display !== 'none') || (sys && sys.style.display !== 'none')) {
                resolve(true);
            }
        };
        check();
        const obs = new MutationObserver(check);
        obs.observe(document.body, { attributes: true, subtree: true, attributeFilter: ['style'] });
        setTimeout(() => { obs.disconnect(); resolve(false); }, 6000);
    }));

    if (gpuVisible) {
        await page.evaluate(() => {
            const gpu = document.getElementById('gpu-section') || document.getElementById('system-section');
            const page = gpu?.closest('.page') || document.querySelector('.page.active');
            if (page) page.scrollTop = gpu.offsetTop - 8;
        });
        await sleep(600);
    } else {
        console.log('[CAPTURE] Hardware section not visible; capturing at current scroll position.');
    }
    await captureShot(page, 'dashboard-gpu-section.png');
}

// Validation pass for sparkline layouts and clipped section captures.
// The individual SVG clip captures are only useful for debugging sparkline rendering;
// require --close-up to generate them.
async function scenarioSparkline(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    console.log('[CAPTURE] Waiting for metrics to populate...');
    await sleep(4000);
    await captureShot(page, 'sparkline-sparkline-validate-full.png', { fullPage: true });
    await captureElementScreenshot(page, '#gpu-section', 'sparkline-sparkline-validate-gpu-section.png', { padding: 24 });
    await captureElementScreenshot(page, '#system-section', 'sparkline-sparkline-validate-system-section.png', { padding: 24 });
    if (options.closeUp) {
        await captureSparklineClips(page, 'svg.metric-sparkline, svg.hw-sparkline, svg.hw-metric-sparkline, svg.hw-clock-footer-spark');
    }
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
        // Wait for agent hardware data if we haven't already (gpuOnly path skips inference wait).
        if (options.gpuOnly) await sleep(3500);
        await page.evaluate(() => {
            const target = document.getElementById('gpu-section') || document.getElementById('system-section');
            if (target) {
                const pg = target.closest('.page') || document.querySelector('.page.active');
                if (pg) pg.scrollTop = target.offsetTop - 8;
            }
        });
        await sleep(1200);
        await captureFrames(page, 'gpu', totalFrames, fps);
        framesToGif('gpu', join(ARTIFACTS_DIR, 'gpu-metrics.gif'), fps);
        cleanupFrames();
    }
}

// Sidebar features capture: expanded panel, collapsed strip, FTS search, context menu
// ── Sidebar ─────────────────────────────────────────────────────────────────────
// Chat sidebar, FTS search flyout, context menu, title filter.

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
        for (let i = 0; i < 8; i++) {
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

        const repeated = [
            'The ledger vanished into the rain before dawn.',
            'Rain washed the alley clean, but the ledger stayed hidden.',
            'She traced the ledger route through rain-soaked streets.',
        ];

        if (tabs[0]) {
            tabs[0].name = 'Noir Scene';
            tabs[0].messages = [
                { role: 'user', content: 'Write a noir scene in progress.' },
                { role: 'assistant', content: 'The rain fell like needles on the pavement, each drop a tiny hammer against the silence. She stood in the shadow of the alley, her trench coat soaked through, her eyes scanning the street for the man who had promised to deliver the ledger.' },
                { role: 'user', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
            ];
            tabs[0].pinned = true;
        }
        if (tabs[1]) {
            tabs[1].name = 'Debug Session';
            tabs[1].messages = [
                { role: 'user', content: 'Help me debug a slow HTTP endpoint.' },
                { role: 'assistant', content: 'Start by profiling the request duration, then inspect database queries, external service calls, and any retries or timeouts in the logs.' },
                { role: 'user', content: repeated[1] },
                { role: 'assistant', content: repeated[2] },
            ];
        }
        if (tabs[2]) {
            tabs[2].name = 'CI Pipeline';
            tabs[2].messages = [
                { role: 'user', content: 'Outline a simple CI pipeline for a Rust backend.' },
                { role: 'assistant', content: 'Use GitHub Actions: run cargo fmt, cargo clippy, cargo test, then build a release binary and upload artifacts.' },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[3]) {
            tabs[3].name = 'Recipe Ideas';
            tabs[3].messages = [
                { role: 'user', content: 'Suggest 3 quick dinner recipes using chicken and rice.' },
                { role: 'assistant', content: '- Chicken and rice skillet with vegetables.\n- One-pot lemon herb chicken rice.\n- Stir-fried chicken with soy-ginger rice.' },
                { role: 'user', content: repeated[2] },
            ];
        }
        if (tabs[4]) {
            tabs[4].name = 'GPU Monitoring';
            tabs[4].messages = [
                { role: 'user', content: 'How can I monitor GPU temperature and utilization from the CLI?' },
                { role: 'assistant', content: 'Use tools like nvidia-smi, nvtop, or custom scripts reading from /sys/class/thermal and GPU management APIs.' },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[5]) {
            tabs[5].name = 'Rain Ledger Notes';
            tabs[5].messages = [
                { role: 'user', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
                { role: 'user', content: repeated[2] },
            ];
        }
        if (tabs[6]) {
            tabs[6].name = 'Shadow Draft';
            tabs[6].messages = [
                { role: 'assistant', content: repeated[1] },
                { role: 'assistant', content: repeated[0] },
            ];
        }
        if (tabs[7]) {
            tabs[7].name = 'Archive Search';
            tabs[7].messages = [
                { role: 'user', content: repeated[2] },
                { role: 'assistant', content: repeated[0] },
                { role: 'assistant', content: repeated[1] },
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
            const _auth = window.__API_TOKEN
                    ? { 'Authorization': `Bearer ${window.__API_TOKEN}` }
                    : {};
                await fetch('/api/chat/tabs', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json', ..._auth },
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
                        headers: { 'Content-Type': 'application/json', ..._auth },
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

   // Capture sidebar element detail (close-up only)

    if (options.closeUp) {
        await captureElementScreenshot(page, '#chat-sessions-panel', 'sidebar-sidebar-panel-detail.png', { padding: 16 });
    }

    // Collapse the sidebar (directly set class for reliability)
    await page.evaluate(() => {
        const panel = document.getElementById('chat-sessions-panel');
        if (panel) panel.classList.add('collapsed');
        localStorage.setItem('csp-collapsed', 'true');
    });
    await sleep(500);

    // Capture collapsed strip
    await captureShot(page, 'sidebar-sidebar-collapsed.png', { fullPage: true });

    // Capture collapsed strip detail (close-up only)
    if (options.closeUp) {
        await captureElementScreenshot(page, '#csp-collapsed-strip', 'sidebar-sidebar-collapsed-detail.png', { padding: 12 });
    }

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
    const searchBtn = await page.$('#csp-message-search-btn');
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
            await page.type('#csp-search-input', 'ledger');
            await sleep(1200);

            // Ensure results area is visible
            const resultsVisible = await page.evaluate(() => {
                const results = document.querySelector('.csp-search-results');
                return results ? (results.style.display !== 'none') : false;
            });
            console.log('[CAPTURE] Search results visible:', resultsVisible);

            // Capture full-page with search mode and results
            await captureShot(page, 'sidebar-fts-search-active.png', { fullPage: true });

            // Capture close-up of search results (close-up only)
            if (options.closeUp) {
                const searchResults = await page.$('.csp-search-panel');
                if (searchResults) {
                    await captureElementScreenshot(page, '.csp-search-panel', 'sidebar-fts-search-results.png', { padding: 12 });
                    await captureCloseUp(page, '.csp-search-panel', 'sidebar-fts-search-results.png', options);
                }
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
    // Capture context menu detail (close-up only)
    if (options.closeUp) {
        const menu = await page.$('.csp-context-menu');
        if (menu) {
            await captureElementScreenshot(page, '.csp-context-menu', 'chat-context-menu-detail.png', { padding: 12 });
        }
    }
    // Close menu
    await page.keyboard.press('Escape');
    await sleep(300);

    // Sidebar resize handle — capture with hover state (close-up only)
    if (options.closeUp) {
        try {
            const resizeHandle = await page.$('#sidebar-resize-handle');
            if (resizeHandle) {
                const box = await resizeHandle.boundingBox();
                if (box) {
                    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
                    await sleep(300);
                    await captureElementScreenshot(page, '.sidebar-nav', 'sidebar-resize-handle.png', { padding: 0 });
                }
            }
        } catch {
            console.log('[CAPTURE] Sidebar resize handle capture failed, skipping...');
        }
    }

    // Test search filter (title filter, not FTS)
    await page.type('#csp-search', 'Noir');
    await sleep(500);
    await captureShot(page, 'sidebar-sidebar-title-filter.png', { fullPage: true });

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
        const advancedTab = await page.$('.settings-tab[data-tab="advanced"]');
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

        // Ensure Browse button is visible and scroll into view if needed
        const browseBtn = await page.$('#config-browse-server-path');
        if (!browseBtn) {
            console.log('[CAPTURE] Browse button for server path not found');
            return;
        }

        // Scroll the button into view within the modal
        await page.evaluate(() => {
            const btn = document.getElementById('config-browse-server-path');
            if (btn) {
                btn.scrollIntoView({ behavior: 'instant', block: 'center' });
            }
        });
        await sleep(300);

        // Use evaluate to trigger click directly (more robust)
        await page.evaluate(() => {
            const btn = document.getElementById('config-browse-server-path');
            if (btn) {
                btn.click();
            }
        });
        await sleep(1000);

        // Ensure file browser modal is visible
        const fileBrowserModal = await page.$('#file-browser-modal');
        if (!fileBrowserModal) {
            console.log('[CAPTURE] File browser modal not found');
            return;
        }

        // Wait for entries to load so hint and list are visible
        await page.waitForSelector('#fb-entries .fb-entry', { timeout: 3000 }).catch(() => {});
        await sleep(400);

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

// ── Tune Panel ────────────────────────────────────────────────────────────────
// Performance benchmark panel on the server tab.

async function scenarioTunePanel(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'server');
    await sleep(3000);

    // The tune panel appears after attach; try to capture it in idle state.
    const tunePanelVisible = await page.evaluate(() => {
        const panel = document.getElementById('tune-panel');
        if (!panel) return false;
        return getComputedStyle(panel).display !== 'none';
    });

    if (tunePanelVisible) {
        await captureShot(page, 'tune-panel-open.png', { fullPage: true });
        await captureCloseUp(page, '#tune-panel', 'tune-panel-open.png', options);
    } else {
        console.log('[CAPTURE] Tune panel not visible; may require a spawned session. Capturing server tab anyway.');
        await captureShot(page, 'tune-panel-server-tab.png', { fullPage: true });
    }
}

// ── Llama Updater ─────────────────────────────────────────────────────────────
// Llama-server binary update pill and release notes panel.

async function scenarioLlamaUpdater(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    // Simulate an available llama.cpp binary update pill
    await page.evaluate(() => {
        const pill = document.getElementById('llama-pill');
        const verSpan = document.getElementById('llama-pill-version');
        if (pill && verSpan) {
            verSpan.textContent = 'llama.cpp · ↑ b4620';
            pill.classList.remove('llama-pill-idle');
            pill.classList.add('llama-pill-update');
            pill.style.display = 'flex';
            pill.title = 'Update available: b4500 → b4620. Click to update.';
        }
    });
    await sleep(600);
    await captureShot(page, 'llama-updater-pill.png', { fullPage: true });

    // Also simulate an app update pill and open release notes
    await page.evaluate(() => {
        const pill = document.getElementById('update-pill');
        const text = document.getElementById('update-pill-text');
        if (pill && text) {
            text.textContent = 'v0.3.0 available';
            pill.style.display = 'flex';
        }
    });
    await sleep(400);

    // Open the release notes panel via JS
    await page.evaluate(async () => {
        const { openReleaseNotes } = await import('/js/features/updates.js');
        window._pendingRelease = {
            tag_name: 'v0.3.0',
            body: '## New Features\n- Spawn wizard with guided setup\n- VRAM estimator improvements\n\n## Fixes\n- Fixed several race conditions in chat storage',
            html_url: 'https://github.com/example/llama-monitor/releases/tag/v0.3.0',
        };
        openReleaseNotes();
    });
    await page.waitForSelector('#release-notes-panel.open', { timeout: 5000 }).catch(() => {});
    await sleep(800);
    await captureShot(page, 'llama-updater-release-notes.png', { fullPage: true });

    // Open the llama.cpp version modal — shows release list + notes panel
    await page.evaluate(async () => {
        const pill = document.getElementById('llama-pill');
        if (pill) pill.click();
    });
    await page.waitForSelector('#llama-version-modal.open', { timeout: 8000 }).catch(() => {
        console.warn('[CAPTURE] llama-version-modal did not open; may not have binary installed.');
    });
    // Wait for release list to populate (real GitHub API call)
    await sleep(2000);

    // Click on the second release row to show its notes (latest is auto-selected)
    await page.evaluate(() => {
        const rows = document.querySelectorAll('.llama-version-row');
        if (rows.length > 1) {
            rows[1].click();
        }
    });
    await sleep(800);
    await captureShot(page, 'llama-updater-version-modal.png', { fullPage: true });

    // Close the modal so later captures aren't obscured
    await page.evaluate(() => {
        const closeBtn = document.getElementById('llama-version-modal-close');
        if (closeBtn) closeBtn.click();
    });
    await sleep(300);
}

// ── Chat History Q&A Panel ────────────────────────────────────────────────────
// Slide-in panel for asking questions about chat history.

async function scenarioChatHistoryQA(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(500);

    // Create a chat with some content so the panel has context
    await createFreshChat(page);
    await page.evaluate(async () => {
        const { chat } = await import('/js/core/app-state.js');
        const { switchChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        const tab = chat.tabs[chat.tabs.length - 1];
        if (tab) {
            tab.messages = [
                { role: 'user', content: 'Start a detective story set in 1940s Chicago.' },
                { role: 'assistant', content: 'The rain drummed against the window as Detective Malone stared at the case file. Another missing person, another dead end.' },
                { role: 'user', content: 'Introduce a suspect with a hidden motive.' },
                { role: 'assistant', content: 'Victor Crane stepped into the office, his trench coat dripping wet. He claimed he was looking for his sister, but Malone noticed the fear in his eyes.' },
            ];
            await switchChatTab(tab.id);
            renderChatTabs();
            renderChatMessages();
        }
    });
    await sleep(600);

    // Open the History Q&A panel by clicking its button
    await page.evaluate(() => {
        const btn = document.getElementById('chat-history-qa-btn');
        if (btn) btn.click();
    });
    await page.waitForSelector('#chat-history-qa-panel.slide-panel-open', { timeout: 5000 }).catch(() => {
        // Fallback: wait for panel to be visible via display/transform
        page.waitForFunction(() => {
            const panel = document.getElementById('chat-history-qa-panel');
            if (!panel) return false;
            const style = getComputedStyle(panel);
            return style.display !== 'none' && (panel.classList.contains('slide-panel-open') || style.transform !== 'translateX(100%)');
        }, { timeout: 5000 });
    }).catch(() => {
        console.log('[CAPTURE] History Q&A panel did not become visible; capturing anyway.');
    });
    await sleep(800);
    await captureShot(page, 'chat-history-qa-panel.png', { fullPage: true });
    await captureCloseUp(page, '#chat-history-qa-panel', 'chat-history-qa-panel.png', options);

    // Close panel
    await page.evaluate(() => {
        const btn = document.getElementById('chqa-close-btn');
        if (btn) btn.click();
    });
    await sleep(300);

    await cleanupScreenshotTabs(page);
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

// ── Spawn Wizard ────────────────────────────────────────────────────────────────
// Step 1 profile selection, step 2 HF source with discover pills / community
// picks / quant advisor, step 3 VRAM panel. Does not require a remote attach.

async function scenarioSpawnWizard(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Open spawn wizard. Attach is not required; wizard works from the welcome screen.
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await sleep(600);

    // Hide the binary prereq banner so it doesn't clutter every shot.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(200);

    // ── Step 0: profile + use-case — capture AFTER selections so state is visible ─
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-step1-profiles.png', { fullPage: true });

    // ── Advance to Step 1: Model ──────────────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => {});
    await sleep(500);

    // ── Step 1: model source cards — capture before selecting HF ─────────────
    await captureShot(page, 'spawn-wizard-step2-source-cards.png', { fullPage: true });

    // Select HuggingFace source.
    await page.evaluate(() => {
        document.querySelector('.model-source-card[data-source="hf"]')?.click();
    });
    await sleep(400);

    // Capture the base HF panel: discover pills + quickpick row, no search started.
    await captureShot(page, 'spawn-wizard-step2-hf-base.png', { fullPage: true });

    // Helper: wait up to 20 s for real result cards; continues silently if none arrive.
    const waitForResults = () => page.waitForFunction(() => {
        const r = document.getElementById('hf-search-results');
        return r && r.style.display !== 'none'
            && !r.querySelector('.hf-search-loading')
            && r.querySelector('.hf-search-result') !== null;
    }, { timeout: 20000 }).catch(() => {});

    // Helper: scroll wizard body so the search results area is visible.
    const scrollToResults = () => page.evaluate(() => {
        const results = document.getElementById('hf-search-results');
        if (results && results.style.display !== 'none') {
            results.scrollIntoView({ behavior: 'instant', block: 'start' });
        } else {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 240;
        }
    });

    // ── Discover pill: Trending ───────────────────────────────────────────────
    const trendingPill = await page.$('.hf-discover-pill[data-cat-id="trending"]');
    if (trendingPill) {
        await trendingPill.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-discover-trending.png', { fullPage: true });
    }

    // ── Discover pill: Qwen3 ─────────────────────────────────────────────────
    const qwen3Pill = await page.$('.hf-discover-pill[data-cat-id="qwen3"]');
    if (qwen3Pill) {
        await qwen3Pill.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-discover-qwen3.png', { fullPage: true });
    }

    // ── Quantizer quick-pick: bartowski ──────────────────────────────────────
    const bartowskiBtn = await page.$('.hf-qp-btn[data-author="bartowski"]');
    if (bartowskiBtn) {
        await bartowskiBtn.click();
        await waitForResults();
        await sleep(400);
        await scrollToResults();
        await sleep(200);
        await captureShot(page, 'spawn-wizard-step2-quantizer-bartowski.png', { fullPage: true });
    }

    // ── Community picks panel ─────────────────────────────────────────────────
    await page.evaluate(() => {
        const r = document.getElementById('hf-search-results');
        if (r) { r.style.display = 'none'; r.innerHTML = ''; }
        document.querySelectorAll('.hf-discover-pill, .hf-qp-btn')
            .forEach(p => p.classList.remove('active'));
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(300);

    const cpToggle = await page.$('#hf-cp-toggle');
    if (cpToggle) {
        await cpToggle.click();
        await page.waitForFunction(
            () => document.getElementById('hf-cp-toggle')?.getAttribute('aria-expanded') === 'true',
            { timeout: 3000 }
        ).catch(() => {});
        await sleep(500);
        await page.evaluate(() => {
            const picks = document.getElementById('hf-community-picks');
            if (picks) picks.scrollIntoView({ behavior: 'instant', block: 'start' });
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-step2-community-picks.png', { fullPage: true });

        // Second tab (MoE / Offload picks) if present.
        await page.evaluate(() => {
            const tabs = document.querySelectorAll('.hf-cp-tab');
            if (tabs.length > 1) tabs[1].click();
        });
        await sleep(300);
        await captureShot(page, 'spawn-wizard-step2-community-picks-moe.png', { fullPage: true });
    }

    // ── Quant advisor: type a known repo so file list populates reliably ──────
    await page.evaluate(() => {
        const cp = document.getElementById('hf-community-picks');
        if (cp) cp.style.display = 'none';
        const body = document.querySelector('.wizard-body');
        if (body) body.scrollTop = 0;
    });
    await sleep(200);

    const repoInput = await page.$('#spawn-hf-repo');
    if (repoInput) {
        await repoInput.click({ clickCount: 3 });
        await repoInput.type('bartowski/Llama-3.2-1B-Instruct-GGUF', { delay: 20 });
        await page.keyboard.press('Enter');
        await page.waitForFunction(() => {
            const fl = document.getElementById('spawn-hf-file-list');
            return fl && fl.classList.contains('visible') && fl.querySelector('.hf-file-item') !== null;
        }, { timeout: 20000 }).catch(() => {});
        await sleep(500);
        await captureShot(page, 'spawn-wizard-step2-quant-advisor.png', { fullPage: true });

        // Select Q4_K_M so validation passes on Next.
        await page.evaluate(() => {
            const q4 = [...document.querySelectorAll('.hf-file-item')]
                .find(el => el.textContent.includes('Q4_K_M') || el.textContent.includes('Q4'));
            (q4 || document.querySelector('.hf-file-item'))?.click();
        });
        await sleep(300);
    }

    // ── Step 2: Hardware / VRAM ───────────────────────────────────────────────
    // Inject a visually impressive model so the VRAM bar fills meaningfully.
    // Llama-3.1-8B Q4_K_M: ~4.9 GB weights on 64 GB → bar is well-proportioned.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.source   = 'hf';
        wizardState.model.delivery = 'stream_hf';
        wizardState.model.hfRepo   = 'bartowski/Meta-Llama-3.1-8B-Instruct-GGUF';
        wizardState.model.hfFile   = 'Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf';
        wizardState.model.paramB   = 8;
        wizardState.model.modelBytes = 4_920_000_000;
        wizardState.vram.available = 64 * 1024 * 1024 * 1024; // fallback if API absent
    });

    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 8000 }
    ).catch(() => console.log('[CAPTURE] Step 2 (Hardware) wait timed out; continuing.'));
    await sleep(600);

    // Dismiss the HF download panel so the VRAM display is unobscured.
    await page.evaluate(() => {
        document.getElementById('hf-dlp-use-hf-btn')?.click();
        const panel = document.getElementById('hf-download-panel');
        if (panel) panel.style.display = 'none';
    });

    // Force a VRAM refresh so the bar renders with the injected state.
    await page.evaluate(async () => {
        const { scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        scheduleVramUpdate();
    });
    await page.waitForFunction(
        () => parseFloat(document.getElementById('vseg-weights')?.style.width || '0') > 1,
        { timeout: 6000 }
    ).catch(() => {});
    await sleep(500);
    await captureShot(page, 'spawn-wizard-step3-vram.png', { fullPage: true });

    // ── Step 3: Summary ───────────────────────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-3')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 3 (Summary) wait timed out; continuing.'));
    await sleep(800);
    // Scroll to the config list — the most informative part of the summary step.
    await page.evaluate(() => {
        const list = document.getElementById('spawn-summary-list');
        if (list) list.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(300);
    await captureShot(page, 'spawn-wizard-step4-summary.png', { fullPage: true });

    // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(400);
}

// Spawn wizard HF download panel: idle options + simulated progress.
async function scenarioSpawnWizardHfDownload(ctx, options) {
    const { page, baseUrl } = ctx;

    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    // Open wizard via JS (safer than DOM click in headless).
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 });
    await sleep(400);

    // Choose a profile quickly.
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(200);

    // Next to step 1 (model), then directly jump to step 2 (VRAM) with injected HF model.
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 6000 }
    ).catch(() => {});

    // Inject model so VRAM bar and HF download panel behave.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.source   = 'hf';
        wizardState.model.delivery = 'download';
        wizardState.model.hfRepo   = 'bartowski/Meta-Llama-3.1-8B-Instruct-GGUF';
        wizardState.model.hfFile   = 'Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf';
        wizardState.model.paramB   = 8;
        wizardState.model.modelBytes = 4_920_000_000;
        wizardState.vram.available = 64 * 1024 * 1024 * 1024;
    });

    // Move to step 2 (VRAM).
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 6000 }
    ).catch(() => {
        console.log('[CAPTURE] Step 2 (Hardware) wait timed out; continuing.');
    });
    await sleep(400);

    // Ensure the HF download panel is visible in "idle" state.
    await page.evaluate(() => {
        const panel = document.getElementById('hf-download-panel');
        const idle = document.getElementById('hf-dlp-idle');
        if (panel && idle) {
            panel.style.display = 'block';
        }
    });
    await sleep(400);

    // 1) Capture idle HF download panel.
    await captureShot(page, 'spawn-wizard-hf-download-idle.png', { fullPage: true });

    // 2) Simulate a progress state for a second shot.
    await page.evaluate(() => {
        const panel = document.getElementById('hf-download-panel');
        const progress = document.getElementById('hf-dlp-progress');
        const bar = document.getElementById('hf-dlp-bar');
        const pct = document.getElementById('hf-dlp-progress-pct');
        const fileEl = document.getElementById('hf-dlp-progress-file');
        const stats = document.getElementById('hf-dlp-stats');

        if (panel && progress) {
            panel.style.display = 'block';
            progress.style.display = 'block';
            if (bar) bar.style.width = '64%';
            if (pct) pct.textContent = '64%';
            if (fileEl) fileEl.textContent = 'Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf';
            if (stats) stats.textContent = '3.18 GB / 4.92 GB · 98 MiB/s · 17m left';
        }
    });
    await sleep(400);

    // Capture simulated progress.
    await captureShot(page, 'spawn-wizard-hf-download-progress.png', { fullPage: true });

    // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(400);
}

// Animated GIF walking through the spawn wizard: welcome → profile → model → VRAM → summary.
//
// Design: fully sequential — each wizard state is fully reached before frames are captured.
// This guarantees every frame reflects real UI state, with no race between capture and interaction.
//
// VRAM panel uses injected state so the bar renders reliably without a GPU or HF API.
async function scenarioSpawnWizardGif(ctx, _options) {
    const { page, baseUrl } = ctx;
    const fps = 10;
    let frameIdx = 0;

    await gotoApp(page, baseUrl);
    fs.mkdirSync(FRAME_DIR, { recursive: true });
    console.log('[CAPTURE] Starting spawn-wizard-gif sequential capture...');

    // Capture N milliseconds of the current page state.
    const capture = async (durationMs) => {
        const frameMs = 1000 / fps;
        const n = Math.max(1, Math.round(durationMs / frameMs));
        for (let i = 0; i < n; i++) {
            const path = join(FRAME_DIR, `spawn-wizard-gif_${String(frameIdx).padStart(3, '0')}.png`);
            await page.screenshot({ path });
            frameIdx++;
            if (i < n - 1) await sleep(frameMs);
        }
    };

    // ── Welcome screen ────────────────────────────────────────────────────────
    // Brief hold so viewer registers we are at the app entry point.
    await capture(1500);

    // ── Open wizard ───────────────────────────────────────────────────────────
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 });
    // Hide the binary prereq banner — it's expected info but clutters the GIF.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(400);
    await capture(800);

    // ── Step 0: Profile ───────────────────────────────────────────────────────
    // Show the initial cards, then make selections with dwell time between.
    await capture(600);

    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await capture(700);

    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(200);
    await capture(1000); // Dwell on selections before advancing.

    // ── Step 0 → Step 1: Model ────────────────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 1 wait timed out; continuing.'));
    await sleep(300);
    await capture(600);

    // ── Step 1: Model source — select HuggingFace ─────────────────────────────
    await page.evaluate(() => {
        document.querySelector('.model-source-card[data-source="hf"]')?.click();
    });
    await sleep(300);
    await capture(700); // HF panel opens.

    // Show the discover pills row (static content, no network needed).
    await capture(600);

    // Show community picks if available (loaded from local community-picks.json).
    const cpToggle = await page.$('#hf-cp-toggle');
    if (cpToggle) {
        await cpToggle.click();
        await page.waitForFunction(
            () => document.getElementById('hf-cp-toggle')?.getAttribute('aria-expanded') === 'true',
            { timeout: 3000 }
        ).catch(() => {});
        await sleep(300);
        // Scroll down to reveal community picks content.
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 400;
        });
        await sleep(200);
        await capture(1200);
        // Scroll back to top for next steps.
        await page.evaluate(() => {
            const body = document.querySelector('.wizard-body');
            if (body) body.scrollTop = 0;
        });
        // Collapse picks so the HF repo input is visible again.
        await cpToggle.click();
        await sleep(200);
    }

    // Inject the full model state now so validation passes on Next.
    // Using Llama-3.3-70B Q4_K_M: weights ~39.6 GB fills the 64 GB VRAM bar at ~60%,
    // which makes a visually impressive and realistic demo.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        wizardState.model.source   = 'hf';
        wizardState.model.delivery = 'stream_hf';
        wizardState.model.hfRepo   = 'bartowski/Meta-Llama-3.3-70B-Instruct-GGUF';
        wizardState.model.hfFile   = 'Meta-Llama-3.3-70B-Instruct-Q4_K_M.gguf';
        wizardState.model.paramB   = 70;
        wizardState.model.modelBytes = 39_600_000_000; // Q4_K_M ~39.6 GB
        // Fallback VRAM if fetchGpuVram fails (matches a 64 GB M4 Max).
        wizardState.vram.available = 64 * 1024 * 1024 * 1024;
    });

    // Show the repo input filled in for visual context.
    await page.$eval('#spawn-hf-repo', el => {
        el.value = 'bartowski/Meta-Llama-3.3-70B-Instruct-GGUF';
        el.dispatchEvent(new Event('input', { bubbles: true }));
    }).catch(() => {});
    await sleep(200);
    await capture(1000); // Dwell on filled-in HF panel.

    // ── Step 1 → Step 2: Hardware / VRAM ─────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-2')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 2 wait timed out; continuing.'));
    await sleep(400);
    await capture(500);

    // The HF download panel may appear when entering step 2 with an HF file selected.
    // Dismiss it so the VRAM display is unobscured.
    const dlpUseBtn = await page.$('#hf-dlp-use-hf-btn');
    if (dlpUseBtn) {
        await dlpUseBtn.click();
        await sleep(300);
    }

    // Ensure the VRAM display is triggered (scheduleVramUpdate is called by showStep, but
    // vram.available is read from wizardState at display time so it may need a nudge).
    await page.evaluate(async () => {
        const { scheduleVramUpdate } = await import('/js/features/spawn-wizard.js');
        scheduleVramUpdate();
    });

    // Wait for the weights bar to render (confirms VRAM math ran).
    await page.waitForFunction(
        () => parseFloat(document.getElementById('vseg-weights')?.style.width || '0') > 1,
        { timeout: 6000 }
    ).catch(() => console.log('[CAPTURE] VRAM weights bar not populated; continuing.'));
    await sleep(300);

    // ── VRAM panel — initial state (default 8 K ctx) ──────────────────────────
    await capture(2500); // Dwell — this is the centrepiece of the GIF.

    // ── VRAM panel — bump context to 32 K to show KV growing ─────────────────
    await page.evaluate(() => {
        const input = document.getElementById('spawn-context-size');
        if (input) {
            input.value = '32768';
            input.dispatchEvent(new Event('input', { bubbles: true }));
        }
    });
    await sleep(350); // Let debounce fire and bar animate.
    await capture(2000);

    // ── VRAM panel — max GPU layers (all layers on GPU) ───────────────────────
    await page.evaluate(() => {
        const sel = document.getElementById('spawn-gpu-layers');
        if (sel) {
            sel.value = '-1';
            sel.dispatchEvent(new Event('change', { bubbles: true }));
        }
    });
    await sleep(350);
    await capture(1500);

    // ── Step 2 → Step 3: Summary ──────────────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-3')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 3 wait timed out; continuing.'));
    // Wait for renderSummary() to populate the list (also applies sampling defaults).
    await sleep(800);
    // Capture the top of the summary step (sampling + network fields).
    await capture(1000);
    // Scroll down to show the config summary list.
    await page.evaluate(() => {
        const list = document.getElementById('spawn-summary-list');
        if (list) list.scrollIntoView({ behavior: 'instant', block: 'start' });
        else {
            const body = document.querySelector('#wizard-step-3 .wizard-main');
            if (body) body.scrollTop = body.scrollHeight;
        }
    });
    await sleep(300);
    await capture(2500); // Hold on the summary list so viewer can read key choices.

    // ── Step 3 → Step 4: Preset Parameters ───────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-4')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 4 wait timed out; continuing.'));
    // _renderPresetParamsStep() runs synchronously inside showStep; give the DOM a tick.
    await sleep(400);
    // Capture the top of the params page (Model + Hardware sections).
    await capture(1500);
    // Scroll to reveal Sampling and Network sections.
    await page.evaluate(() => {
        const main = document.querySelector('#wizard-step-4 .wizard-main');
        if (main) main.scrollTop = 340;
    });
    await sleep(250);
    await capture(1500);
    // Scroll to the Save Preset row at the bottom.
    await page.evaluate(() => {
        const row = document.getElementById('spawn-save-preset-row');
        if (row) row.scrollIntoView({ behavior: 'instant', block: 'center' });
        else {
            const main = document.querySelector('#wizard-step-4 .wizard-main');
            if (main) main.scrollTop = main.scrollHeight;
        }
    });
    await sleep(250);
    await capture(2000); // Hold so viewer sees the preset name + Save button.

    // ── Step 4 → Step 5: Ready to Launch ─────────────────────────────────────
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-5')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => console.log('[CAPTURE] Step 5 wait timed out; continuing.'));
    // Wait for _renderSpawnConfigCard() to populate the card.
    await sleep(600);
    await capture(2500); // Hold on the Spawn step — config card + Spawn Server button.

    // ── Convert frames → GIF ──────────────────────────────────────────────────
    // Scale to 900 px wide so the GIF fits GitHub README content width without
    // clipping. Height is computed proportionally (-1).
    console.log(`[CAPTURE] Converting ${frameIdx} frames to GIF at ${fps} fps (scaled to 900 px)...`);
    execFileSync('ffmpeg', [
        '-y',
        '-framerate', String(fps),
        '-i', join(FRAME_DIR, `spawn-wizard-gif_%03d.png`),
        '-vf', 'scale=900:-1:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5',
        join(ARTIFACTS_DIR, 'spawn-wizard-flow.gif'),
    ], { stdio: 'inherit' });
    cleanupFrames();
    console.log('[CAPTURE] spawn-wizard-flow.gif complete.');
}

const SCENARIOS = {
    // Core
    welcome: scenarioWelcome,
    chat: scenarioChat,
    // Chat features
    'guided-gen': scenarioGuidedGen,
    sidebar: scenarioSidebar,
    // Models and presets
    'models-v2': scenarioModelsV2,
    'preset-editor': scenarioPresetEditor,
    // Configuration
    settings: scenarioSettings,
    tls: scenarioTls,
    filebrowser: scenarioFilebrowser,
    panels: scenarioPanels,
    models: scenarioModels,
    dashboard: scenarioDashboard,
    // Spawn wizard
    'spawn-wizard': scenarioSpawnWizard,
    'spawn-wizard-gif': scenarioSpawnWizardGif,
    'spawn-wizard-hf-download': scenarioSpawnWizardHfDownload,
    // New features (spawn-llama-server-v2)
    'tune-panel': scenarioTunePanel,
    'llama-updater': scenarioLlamaUpdater,
    'chat-history-qa': scenarioChatHistoryQA,
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

    let server = null;
    let browser = null;
    let baseUrl;

    if (RUNNING_PORT) {
        baseUrl = `http://127.0.0.1:${RUNNING_PORT}`;
        console.log(`[CAPTURE] Using running llama-monitor at ${baseUrl} for scenario "${scenarioName}"...`);
    } else {
        seedConfig();
        const port = await findAvailablePort();
        const extraArgs = [];

        // For the models scenario: seed fake .gguf files and pass --models-dir.
        if (scenarioName === 'models') {
            const modelsDir = join(TEMP_APP_CONFIG_DIR, 'models');
            fs.mkdirSync(modelsDir, { recursive: true });
            const fakeFiles = [
                'Llama-3.3-70B-Instruct-Q4_K_M.gguf',
                'Qwen3-30B-A3B-Q5_K_M.gguf',
                'mistral-nemo-instruct-2407-Q4_K_M.gguf',
                'gemma-3-12b-it-Q8_0.gguf',
                'Devstral-Small-2-24B-UD-Q4_K_S.gguf',
                'Meta-Llama-3.1-8B-Instruct-i1-Q4_K_M.gguf',
            ];
            for (const f of fakeFiles) fs.writeFileSync(join(modelsDir, f), '');
            extraArgs.push('--models-dir', modelsDir);
        }

        console.log(`[CAPTURE] Spawning llama-monitor on port ${port} for scenario "${scenarioName}"...`);
        server = await spawnLlamaMonitor(port, extraArgs);
        baseUrl = server.url;
    }

    try {
        const launched = await launchBrowser(options.viewport);
        browser = launched.browser;
        const page = launched.page;
        await scenario({ page, baseUrl, browser }, options);
        console.log(`[CAPTURE] Scenario "${scenarioName}" complete.`);
    } catch (err) {
        console.error(err.stack || err.message);
        process.exitCode = 1;
    } finally {
        cleanupFrames();
        if (browser) await browser.close();
        if (server) await cleanupServer(server);
        if (!RUNNING_PORT) cleanupTempHome();
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    await runCli();
}
