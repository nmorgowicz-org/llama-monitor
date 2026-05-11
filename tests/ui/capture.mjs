/**
 * Consolidated screenshot and GIF capture harness for llama-monitor.
 *
 * This is the single entrypoint for repo-owned visual capture automation.
 * Prefer adding new scenarios here instead of creating new one-off scripts.
 *
 * Quick examples:
 *   node tests/ui/capture.mjs --list-scenarios
 *   SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario artifacts
 *   SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario new-features
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
        chatOnly: false,
        gpuOnly: false,
        inferenceOnly: false,
        listScenarios: false,
        noAttach: false,
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
  artifacts      Welcome/chat/logs artifact screenshots
  new-features   Guided generation artifact screenshots
  docs           Docs/UI review still screenshots
  sparkline      Sparkline validation screenshots
  gifs           Inference/GPU animated GIF capture

Options:
  --chat-only        For artifacts scenario, capture only chat
  --gpu-only         For gifs scenario, capture only GPU/system animation
  --inference-only   For gifs scenario, capture only inference animation
  --no-attach        Skip remote attach for scenarios that do not require it
  --list-scenarios   Print available scenarios

Examples:
  SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario artifacts
  SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario new-features
  SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
  REMOTE_SERVER=http://127.0.0.1:8001 node tests/ui/capture.mjs --scenario sparkline
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

async function attachToServer(page, remoteServer = REMOTE_SERVER) {
    console.log(`[CAPTURE] Attaching to remote server at ${remoteServer}...`);
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

async function gotoApp(page, baseUrl) {
    await page.goto(baseUrl, { waitUntil: 'networkidle0' });
    await sleep(1500);
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

async function clearExistingChats(page) {
    await page.evaluate(() => {
        document.getElementById('btn-clear')?.click();
    });
    await sleep(200);
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
        const tabs = Array.from(document.querySelectorAll('#chat-tab-bar .chat-tab'));
        const target = tabs.find(tab => tab.textContent?.includes(prefix));
        target?.click();
    }, SCREENSHOT_TAB_PREFIX);
    await sleep(500);
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

async function waitForChatResponse(page, timeoutMs = 180000) {
    await page.waitForFunction(() => {
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        const thinkingBlocks = Array.from(document.querySelectorAll('#chat-messages .chat-thinking'));
        return assistantMessages.length > 0 || thinkingBlocks.length > 0;
    }, { timeout: timeoutMs });
}

async function waitForSuggestionsSettled(page, timeoutMs = 60000) {
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

async function captureElementScreenshot(page, selector, filename, options = {}) {
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

async function scenarioArtifacts(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await captureShot(page, '01-welcome.png', { fullPage: true });
    if (options.noAttach) return;

    await attachToServer(page);
    await createFreshChat(page);
    await sendChatPrompt(
        page,
        'Reply with a compact final answer only: explain what llama.cpp GPU offload via -ngl does and why prompt processing and generation often run at different token speeds.'
    );
    await waitForChatResponse(page);
    await sleep(1500);
    await captureShot(page, '03-chat.png', { fullPage: true });

    const telemetryToggle = await page.$('#chat-telemetry-btn');
    if (telemetryToggle) {
        await telemetryToggle.click();
        await sleep(500);
        await captureShot(page, '03b-chat-telemetry.png', { fullPage: true });
        const telemetryPin = await page.$('#chat-telemetry-pin-btn');
        if (telemetryPin) {
            await telemetryPin.click();
            await sleep(500);
            await captureShot(page, '03c-chat-telemetry-pinned.png', { fullPage: true });
        }
    }
    await cleanupScreenshotTabs(page);

    if (!options.chatOnly) {
        await switchTab(page, 'logs');
        await page.waitForSelector('#logs-empty-state.visible', { timeout: 10000 });
        await sleep(1200);
        await captureShot(page, '05-logs.png', { fullPage: true });
    }
}

// Guided generation capture flow for context notes, suggestions, quick guide,
// and explicit-mode chrome. Keep feature-specific diagnostics here so UI
// regressions are easy to pinpoint from the capture logs.
async function scenarioNewFeatures(ctx) {
    const { page, baseUrl } = ctx;
    const detachSuggestionsLogger = attachSuggestionsResponseLogger(page);
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    await enableGuidedGeneration(page);

    await createFreshChat(page);
    await sendChatPrompt(
        page,
        'Reply with two concise paragraphs about a tense noir scene in progress, leaving room for the next beat to escalate.'
    );
    await waitForChatResponse(page, 120000);
    await sleep(1500);

    console.log('[CAPTURE] Creating test chat tabs...');
    await page.evaluate(() => {
        const addBtn = document.querySelector('.chat-tab-add');
        if (addBtn) {
            addBtn.click();
            addBtn.click();
            addBtn.click();
        }
    });
    await sleep(1000);
    await captureShot(page, '06-chat-tabs.png', { fullPage: true });
    await activateScreenshotChat(page);

    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('./js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(1000);
    await captureShot(page, '08-context-notes-expanded.png', { fullPage: true });
    await page.evaluate(async () => {
        const { toggleContextSidebar } = await import('./js/features/chat-notes.js');
        toggleContextSidebar();
    });
    await sleep(500);

    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(1000);
    console.log('[CAPTURE] Suggestions pre-generate state:', JSON.stringify(
        {
            ...(await describePopover(page, '#suggestions-toggle', '#suggestions-dropdown')),
            ...(await describeSuggestionsPanel(page)),
        }
    ));
    await captureShot(page, '09-suggestions-dropdown.png', { fullPage: true });
    await page.click('#suggestions-generate-btn');
    await waitForSuggestionsSettled(page, 90000);
    await sleep(1200);
    console.log('[CAPTURE] Suggestions generated state:', JSON.stringify(
        await describeSuggestionsPanel(page)
    ));
    await captureShot(page, '09b-suggestions-results.png', { fullPage: true });
    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(500);

    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('./js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(1000);
    console.log('[CAPTURE] Quick guide state:', JSON.stringify(
        await describePopover(page, '#quick-guide-toggle', '#quick-guide-container')
    ));
    await captureShot(page, '10-quick-guide-dropdown.png', { fullPage: true });
    await page.evaluate(async () => {
        const { toggleQuickGuide } = await import('./js/features/chat-quick-guide.js');
        toggleQuickGuide();
    });
    await sleep(500);

    await captureShot(page, '11-chat-input-buttons.png', { fullPage: true });

    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, '12a-explicit-unlocked.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, '12b-explicit-unrestricted.png', { fullPage: false });
    await page.evaluate(() => document.getElementById('chat-explicit-toggle-footer')?.click());
    await sleep(800);
    await captureShot(page, '12c-explicit-locked.png', { fullPage: false });

    await page.evaluate(async () => {
        const { toggleSuggestionsDropdown } = await import('./js/features/chat-suggestions.js');
        toggleSuggestionsDropdown();
    });
    await sleep(800);
    await captureShot(page, '13a-suggestions-tag-cloud.png', { fullPage: false });
    await page.type('#suggestion-search-input', 'horror');
    await sleep(500);
    await captureShot(page, '13b-suggestions-search-filter.png', { fullPage: false });

    await cleanupScreenshotTabs(page);
    detachSuggestionsLogger();
}

// Documentation stills for settings/modals/panels used in README and docs.
async function scenarioDocs(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'chat');
    await sleep(1000);

    const styleBtn = await page.$('#chat-style-btn');
    if (styleBtn) {
        await styleBtn.click();
        await sleep(500);
        await captureShot(page, '06-chat-style.png', { fullPage: true });
        await page.keyboard.press('Escape');
        await sleep(300);
    }

    const compactBtn = await page.$('#chat-compact-btn');
    if (compactBtn) {
        await compactBtn.click();
        await sleep(500);
        await captureShot(page, '07-compact-settings.png', { fullPage: true });
        await page.keyboard.press('Escape');
        await sleep(300);
    }

    const behaviorBtn = await page.$('#chat-behavior-btn');
    if (behaviorBtn) {
        await behaviorBtn.click();
        await sleep(500);
        await captureShot(page, '08-behavior-settings.png', { fullPage: true });
        await page.keyboard.press('Escape');
        await sleep(300);
    }

    const settingsBtn = await page.$('button[name="settings"]');
    if (settingsBtn) {
        await settingsBtn.click();
        await sleep(800);
        await captureShot(page, '09-settings-modal.png', { fullPage: true });

        const perfTab = await page.$('#settings-tab-performance');
        if (perfTab) {
            await perfTab.click();
            await sleep(500);
            await captureShot(page, '09b-settings-performance.png', { fullPage: true });
        }

        const advTab = await page.$('#settings-tab-advanced');
        if (advTab) {
            await advTab.click();
            await sleep(500);
            await captureShot(page, '09c-settings-advanced.png', { fullPage: true });
        }

        await page.keyboard.press('Escape');
        await sleep(300);
    }

    const userBtn = await page.$('#user-menu-btn');
    if (userBtn) {
        await userBtn.click();
        await sleep(300);
        const prefsBtn = await page.$('#user-menu-prefs-btn');
        if (prefsBtn) {
            await prefsBtn.click();
            await sleep(500);
            await captureShot(page, '10-user-preferences.png', { fullPage: true });
            await page.keyboard.press('Escape');
            await sleep(300);
        }
    }

    await page.keyboard.down('Control');
    await page.keyboard.press('/');
    await page.keyboard.up('Control');
    await sleep(500);
    await captureShot(page, '11-keyboard-shortcuts.png', { fullPage: true });
    await page.keyboard.press('Escape');
    await sleep(300);

    await switchTab(page, 'server');
    await sleep(1000);
    await captureShot(page, '12-server-tab.png', { fullPage: true });
    await page.evaluate(() => {
        const gpu = document.getElementById('gpu-section') || document.getElementById('system-section');
        gpu?.scrollIntoView({ behavior: 'instant', block: 'start' });
    });
    await sleep(500);
    await captureShot(page, '13-gpu-section.png', { fullPage: true });
}

// Validation pass for sparkline layouts and clipped section captures.
async function scenarioSparkline(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    console.log('[CAPTURE] Waiting for metrics to populate...');
    await sleep(4000);
    await captureShot(page, 'sparkline-validate-full.png', { fullPage: true });
    await captureElementScreenshot(page, '#gpu-section', 'sparkline-validate-gpu-section.png', { padding: 24 });
    await captureElementScreenshot(page, '#system-section', 'sparkline-validate-system-section.png', { padding: 24 });
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
        framesToGif('inference', join(SCREENSHOTS_DIR, '02-inference-metrics.gif'), fps);
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
        framesToGif('gpu', join(SCREENSHOTS_DIR, '04-gpu-metrics.gif'), fps);
        cleanupFrames();
    }
}

const SCENARIOS = {
    artifacts: scenarioArtifacts,
    'new-features': scenarioNewFeatures,
    docs: scenarioDocs,
    sparkline: scenarioSparkline,
    gifs: scenarioGifs,
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

    // Default to the core artifact flow so an unqualified invocation still
    // produces something useful for docs work.
    const scenarioName = forcedScenario || options.scenario || 'artifacts';
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
