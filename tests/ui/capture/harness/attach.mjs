// Remote-attach helpers (connect capture browser to an already-running llama-monitor).
// Extracted from tests/ui/capture.mjs (Phase A1).
import { gotoApp, launchBrowser, waitForMonitor } from './browser.mjs';
import { CAPTURE_FORM_AUTH, DEFAULT_VIEWPORT, REMOTE_SERVER, sleep } from './paths.mjs';
import { cleanupServer, spawnLlamaMonitor } from './server.mjs';
import { captureShot } from './shot.mjs';

export async function attachToServer(page, remoteServer = REMOTE_SERVER) {
    console.log(`[CAPTURE] Attaching to remote server at ${remoteServer}...`);

    // Set up listener for /api/attach response.
    const attachPromise = new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
            reject(new Error('Attach API request timed out (no /api/attach response within 45s)'));
        }, 45000);
        const handler = async (response) => {
            if (!response.url().includes('/api/attach')) return;
            clearTimeout(timeout);
            page.off('response', handler);
            try {
                const body = await response.text();
                console.log(`[CAPTURE] /api/attach response ${response.status()}: ${body.trim()}`);
            } catch (e) {
                console.log(`[CAPTURE] /api/attach response ${response.status()} (read: ${e.message})`);
            }
            resolve();
        };
        page.on('response', handler);
    });

    // Fill the endpoint URL.
    await page.waitForSelector('#setup-endpoint-url', { visible: true });
    await page.$eval('#setup-endpoint-url', (input, url) => {
        input.value = url;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, remoteServer);
    await sleep(200);

    // Instead of relying on a DOM click, call the SPA's doAttachFromSetup() directly
    // so the attach flow is deterministic in headless environments.
    console.log('[CAPTURE] Invoking doAttachFromSetup()...');
    const attachResult = await page.evaluate(async () => {
        try {
            const { doAttachFromSetup } = await import('/js/features/attach-detach.js');
            if (typeof doAttachFromSetup === 'function') {
                await doAttachFromSetup();
                return 'called_doAttachFromSetup';
            }
        } catch (err) {
            console.error('[CAPTURE] doAttachFromSetup error:', err);
            return 'doAttachFromSetup_error: ' + (err.message || err);
        }
        return 'doAttachFromSetup_not_found';
    });
    console.log('[CAPTURE] doAttachFromSetup result:', attachResult);

    // Wait for both the API response and the monitor view.
    await Promise.all([
        attachPromise,
        waitForMonitor(page),
    ]);

    // Optional: confirm endpoint displayed.
    await page.waitForFunction(
        endpoint => document.getElementById('endpoint-url')?.textContent?.includes(endpoint),
        { timeout: 5000 }
    ).catch(() => {
        console.log('[CAPTURE] Server URL not confirmed in display (non-fatal)');
    });

    await sleep(1500);
    console.log('[CAPTURE] Attach successful.');
}

export async function captureAuthShell(port, viewport = DEFAULT_VIEWPORT) {
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
