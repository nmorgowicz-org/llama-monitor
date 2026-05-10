import puppeteer from 'puppeteer';
import fs from 'fs';
import { spawn } from 'child_process';
import net from 'net';

const OUTPUT_DIR = '/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/screenshots/artifacts';
const BINARY = '/Users/nick/SCRIPTS/CLAUDE/llama-monitor/target/release/llama-monitor';
const REMOTE_SERVER = 'http://192.168.2.16:8001';
const sleep = ms => new Promise(r => setTimeout(r, ms));

async function findPort(start = 8891) {
    for (let p = start; p < start + 200; p++) {
        const ok = await new Promise(r => {
            const s = net.createServer(); s.unref();
            s.on('error', () => r(false));
            s.listen(p, '127.0.0.1', () => { s.close(() => r(true)); });
        });
        if (ok) return p;
    }
    throw new Error('No port');
}

async function waitHttp(url, timeout = 30000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
        try { await fetch(url); return; } catch {}
        await sleep(250);
    }
    throw new Error(`Timeout: ${url}`);
}

async function spawnApp(port) {
    const proc = spawn(BINARY, ['--port', String(port), '--headless'], {
        stdio: ['ignore', 'pipe', 'pipe']
    });
    proc.stdout.on('data', d => console.log(`[app] ${d.toString().trim()}`));
    await waitHttp(`http://127.0.0.1:${port}`);
    return proc;
}

(async () => {
    const port = await findPort();
    console.log(`Port: ${port}`);
    const proc = await spawnApp(port);
    const browser = await puppeteer.launch({ headless: 'new', args: ['--no-sandbox', '--disable-setuid-sandbox'] });
    const page = await browser.newPage();
    await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: 1 });
    
    try {
        await page.goto(`http://127.0.0.1:${port}`, { waitUntil: 'networkidle0' });
        await page.waitForSelector('#setup-endpoint-url', { visible: true });
        await page.$eval('#setup-endpoint-url', (el, url) => { el.value = url; el.dispatchEvent(new Event('input', {bubbles:true})); }, REMOTE_SERVER);
        await page.click('#setup-attach-btn');
        await page.waitForFunction(() => {
            const m = document.getElementById('view-monitor');
            return m && getComputedStyle(m).display !== 'none';
        }, { timeout: 30000 });
        await sleep(2000);
        
        // Switch to chat
        await page.click('button[data-tab="chat"]');
        await sleep(1000);
        
        // 1. Chat Style Panel
        console.log('1. Chat style panel...');
        const styleBtn = await page.$('#chat-style-btn');
        if (styleBtn) {
            await styleBtn.click();
            await sleep(500);
            await page.screenshot({ path: `${OUTPUT_DIR}/06-chat-style.png`, fullPage: true });
            await page.keyboard.press('Escape');
            await sleep(300);
        }
        
        // 2. Compact Settings Panel
        console.log('2. Compact settings...');
        const compactBtn = await page.$('#chat-compact-btn');
        if (compactBtn) {
            await compactBtn.click();
            await sleep(500);
            await page.screenshot({ path: `${OUTPUT_DIR}/07-compact-settings.png`, fullPage: true });
            await page.keyboard.press('Escape');
            await sleep(300);
        }
        
        // 3. Behavior Settings Panel
        console.log('3. Behavior panel...');
        const behaviorBtn = await page.$('#chat-behavior-btn');
        if (behaviorBtn) {
            await behaviorBtn.click();
            await sleep(500);
            await page.screenshot({ path: `${OUTPUT_DIR}/08-behavior-settings.png`, fullPage: true });
            await page.keyboard.press('Escape');
            await sleep(300);
        }
        
        // 4. Settings Modal
        console.log('4. Settings modal...');
        const settingsBtn = await page.$('button[name="settings"]');
        if (settingsBtn) {
            await settingsBtn.click();
            await sleep(800);
            await page.screenshot({ path: `${OUTPUT_DIR}/09-settings-modal.png`, fullPage: true });
            
            const perfTab = await page.$('#settings-tab-performance');
            if (perfTab) {
                await perfTab.click();
                await sleep(500);
                await page.screenshot({ path: `${OUTPUT_DIR}/09b-settings-performance.png`, fullPage: true });
            }
            
            const advTab = await page.$('#settings-tab-advanced');
            if (advTab) {
                await advTab.click();
                await sleep(500);
                await page.screenshot({ path: `${OUTPUT_DIR}/09c-settings-advanced.png`, fullPage: true });
            }
            
            await page.keyboard.press('Escape');
            await sleep(300);
        }
        
        // 5. User Preferences
        console.log('5. User preferences...');
        const userBtn = await page.$('#user-menu-btn');
        if (userBtn) {
            await userBtn.click();
            await sleep(300);
            const prefsBtn = await page.$('#user-menu-prefs-btn');
            if (prefsBtn) {
                await prefsBtn.click();
                await sleep(500);
                await page.screenshot({ path: `${OUTPUT_DIR}/10-user-preferences.png`, fullPage: true });
                await page.keyboard.press('Escape');
                await sleep(300);
            }
        }
        
        // 6. Keyboard Shortcuts
        console.log('6. Keyboard shortcuts...');
        await page.keyboard.down('Control');
        await page.keyboard.press('/');
        await page.keyboard.up('Control');
        await sleep(500);
        await page.screenshot({ path: `${OUTPUT_DIR}/11-keyboard-shortcuts.png`, fullPage: true });
        await page.keyboard.press('Escape');
        await sleep(300);
        
        // 7. Server tab
        console.log('7. Server tab...');
        await page.click('button[data-tab="server"]');
        await sleep(1000);
        await page.screenshot({ path: `${OUTPUT_DIR}/12-server-tab.png`, fullPage: true });
        
        await page.evaluate(() => {
            const gpu = document.getElementById('gpu-section') || document.getElementById('system-section');
            gpu?.scrollIntoView({ behavior: 'instant', block: 'start' });
        });
        await sleep(500);
        await page.screenshot({ path: `${OUTPUT_DIR}/13-gpu-section.png`, fullPage: true });
        
        console.log(`Screenshots saved to ${OUTPUT_DIR}`);
    } catch (err) {
        console.error(err.stack || err.message);
    } finally {
        await browser.close();
        proc.kill('SIGTERM');
        await sleep(500);
        proc.kill('SIGKILL');
    }
})();
