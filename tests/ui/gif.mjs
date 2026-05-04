import puppeteer from 'puppeteer';
import fs from 'fs';
import { execFileSync, spawn } from 'child_process';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import net from 'net';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const SCREENSHOTS_DIR = join(__dirname, '../../docs/screenshots');
const FRAME_DIR = join(__dirname, './frames');

const REMOTE_SERVER = process.env.REMOTE_SERVER || 'http://192.168.2.16:8001';
const SCREENSHOT_PORT = parseInt(process.env.SCREENSHOT_PORT || '8891', 10);
const FPS = 10;
const DURATION_SEC = 6;
const TOTAL_FRAMES = FPS * DURATION_SEC;

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

fs.mkdirSync(FRAME_DIR, { recursive: true });
fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });

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

async function attachToServer(page, remoteServer) {
    await page.waitForSelector('#setup-endpoint-url', { visible: true });
    await page.$eval('#setup-endpoint-url', (input, url) => {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, remoteServer);
    await page.click('#setup-attach-btn');
    await page.waitForFunction(() => {
        const monitor = document.getElementById('view-monitor');
        return monitor && getComputedStyle(monitor).display !== 'none';
    }, { timeout: 30000 });
    await sleep(1500);
}

async function switchTab(page, tabName) {
    await page.click(`button[data-tab="${tabName}"]`);
    await page.waitForFunction((name) => {
        const tab = document.querySelector(`button[data-tab="${name}"]`);
        const pageEl = document.getElementById(`page-${name}`);
        if (!tab || !pageEl) return false;
        return pageEl.classList.contains('active') &&
            (tab.classList.contains('active') || tab.classList.contains('selected'));
    }, { timeout: 10000 }, tabName);
}

async function startLiveGeneration() {
    return fetch(`${REMOTE_SERVER}/v1/chat/completions`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            model: 'default',
            stream: false,
            temperature: 0.7,
            max_tokens: 800,
            messages: [{
                role: 'user',
                content: 'Write a dense explanation of transformer inference performance, token throughput, KV cache behavior, and GPU offload tradeoffs.'
            }],
        }),
    }).then(async response => {
        if (!response.ok) {
            throw new Error(`Generation request failed: ${response.status} ${response.statusText}`);
        }
        await response.text();
    });
}

async function captureFrames(page, prefix) {
    for (let i = 0; i < TOTAL_FRAMES; i++) {
        const path = `${FRAME_DIR}/${prefix}_${String(i).padStart(3, '0')}.png`;
        await page.screenshot({ path });
        await sleep(1000 / FPS);
    }
}

function framesToGif(prefix, output) {
    const pattern = `${FRAME_DIR}/${prefix}_%03d.png`;
    execFileSync('ffmpeg', [
        '-y',
        '-framerate', String(FPS),
        '-i', pattern,
        '-vf', 'split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5',
        output,
    ], { stdio: 'inherit' });
}

function cleanupFrames() {
    fs.rmSync(FRAME_DIR, { recursive: true, force: true });
    fs.mkdirSync(FRAME_DIR, { recursive: true });
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
        await page.goto(server.url, { waitUntil: 'networkidle0' });
        await attachToServer(page, REMOTE_SERVER);

        console.log('Capturing inference metrics GIF...');
        await switchTab(page, 'server');
        const generationPromise = startLiveGeneration();
        await sleep(1500);
        await captureFrames(page, 'inference');
        await generationPromise;
        framesToGif('inference', `${SCREENSHOTS_DIR}/02-inference-metrics.gif`);

        console.log('Capturing GPU/system metrics GIF...');
        await page.evaluate(() => {
            const gpuSection = document.getElementById('gpu-section') || document.getElementById('system-section');
            gpuSection?.scrollIntoView({ behavior: 'instant', block: 'start' });
        });
        await sleep(1200);
        await captureFrames(page, 'gpu');
        framesToGif('gpu', `${SCREENSHOTS_DIR}/04-gpu-metrics.gif`);

        cleanupFrames();
        console.log(`GIFs saved to ${SCREENSHOTS_DIR}`);
    } catch (err) {
        console.error(err.stack || err.message);
        process.exitCode = 1;
    } finally {
        await browser.close();
        await cleanupServer(server);
    }
})();
