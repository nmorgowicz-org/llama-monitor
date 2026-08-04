// Browser/page lifecycle: launch, navigate, wait-for-app, tab switching.
// Extracted from tests/ui/capture.mjs (Phase A1).
import puppeteer from 'puppeteer';
import { DEFAULT_VIEWPORT, sleep } from './paths.mjs';

export async function launchBrowser(viewport = DEFAULT_VIEWPORT) {
    const browser = await puppeteer.launch({
        headless: 'new',
        args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage', '--disable-cache', '--disable-service-workers'],
    });
    const page = await browser.newPage();
    await page.setCacheEnabled(false);
    await page.setViewport(viewport);
    return { browser, page };
}

export async function waitForMonitor(page) {
    await page.waitForFunction(() => {
        const setup = document.getElementById('view-setup');
        const monitor = document.getElementById('view-monitor');
        if (!setup || !monitor) return false;
        return getComputedStyle(monitor).display !== 'none' && getComputedStyle(setup).display === 'none';
    }, { timeout: 30000 });
}

export async function switchTab(page, tabName) {
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

export async function gotoApp(page, baseUrl, waitUntil = 'networkidle0') {
    await page.goto(baseUrl, { waitUntil });
    // See the scoped CSS rule in spawn-wizard.css. This marker prevents
    // Chromium's capture-only :focus-visible heuristic from outlining the
    // entire programmatically focused wizard step.
    await page.evaluate(() => { document.documentElement.dataset.screenshotCapture = 'true'; });
    await sleep(1500);
}

export async function loadAppDocument(page, baseUrl) {
    try {
        await page.goto(baseUrl, { waitUntil: 'domcontentloaded', timeout: 10000 });
    } catch (error) {
        console.log('[CAPTURE] app navigation fallback:', error.message);
    }

    const hasAppShell = await page.$('#page-server') !== null;
    if (hasAppShell) {
        await page.evaluate(() => { document.documentElement.dataset.screenshotCapture = 'true'; });
        await sleep(1500);
        return;
    }

    // Inject base tag via page.evaluate so relative URLs resolve
    // (avoids regex-based script stripping that CodeQL flags).
    await page.goto(baseUrl, { waitUntil: 'domcontentloaded', timeout: 10000 });
    await page.evaluate((base) => {
        const baseTag = document.createElement('base');
        baseTag.href = base;
        document.head.insertBefore(baseTag, document.head.firstChild);
    }, baseUrl);
    await page.waitForSelector('#page-server', { timeout: 10000 });
    await page.evaluate(() => { document.documentElement.dataset.screenshotCapture = 'true'; });
    await page.evaluate(() => {
        document.getElementById('auth-shell')?.classList.add('hidden');
        document.body.classList.remove('auth-required');
    });
    await sleep(1500);
}

export async function openAppearancePaneForCapture(page) {
    await page.evaluate(() => {
        if (typeof window.openSettingsModal === 'function') {
            window.openSettingsModal();
            return;
        }
        const modal = document.getElementById('settings-modal');
        if (modal) {
            modal.classList.add('open');
            modal.removeAttribute('inert');
            modal.setAttribute('aria-hidden', 'false');
        }
    });
    await page.waitForSelector('#settings-modal.open', { timeout: 5000 });
    await page.evaluate(() => {
        document.querySelectorAll('.settings-tab').forEach(tab => {
            const active = tab.dataset.tab === 'appearance';
            tab.classList.toggle('active', active);
            tab.setAttribute('aria-selected', String(active));
        });
        document.querySelectorAll('.settings-pane').forEach(pane => {
            pane.classList.toggle('active', pane.id === 'settings-appearance');
        });
    });
}
