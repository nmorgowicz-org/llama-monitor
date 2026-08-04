// Scenario: benchmark-results
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureCloseUp, captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);

    await switchTab(page, 'server');
    await sleep(2000);

    // Force show the benchmark pill
    await page.evaluate(() => {
        const pill = document.getElementById('benchmark-pill');
        const group = document.getElementById('inference-log-tail-group');
        if (pill) pill.classList.add('show');
        if (group) group.style.display = 'inline-flex';
    });
    await sleep(500);

    // Open the dropdown
    await page.evaluate(() => {
        const pill = document.getElementById('benchmark-pill');
        if (pill) pill.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });
    await sleep(800);

    // Click "Run" button inside the dropdown
    const runBtn = await page.$('#tune-run-btn');
    if (!runBtn) {
        throw new Error('[CAPTURE] #tune-run-btn not found in benchmark dropdown');
    }
    await runBtn.click();

    // Wait for the results view: #tune-results visible AND #tune-running gone
    await page.waitForFunction(() => {
        const results = document.getElementById('tune-results');
        const running = document.getElementById('tune-running');
        if (!results || !running) return false;
        const rStyle = getComputedStyle(results);
        const runStyle = getComputedStyle(running);
        return (
            rStyle.display !== 'none' &&
            rStyle.visibility !== 'hidden' &&
            runStyle.display === 'none'
        );
    }, { timeout: 90000 });

    await sleep(2000);

    // Capture full page then close-up of the dropdown with results
    await captureShot(page, 'benchmark-results.png', { fullPage: true });
    await captureCloseUp(page, '#benchmark-dropdown-wrap', 'benchmark-results.png', options);
}

// ── Llama Updater ─────────────────────────────────────────────────────────────
// Llama-server binary update pill and version modal.
