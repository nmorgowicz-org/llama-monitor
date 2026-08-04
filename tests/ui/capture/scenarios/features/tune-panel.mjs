// Scenario: tune-panel
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

    // Force show the benchmark pill and its parent group (workaround for display:none override).
    await page.evaluate(() => {
        const pill = document.getElementById('benchmark-pill');
        const group = document.getElementById('inference-log-tail-group');
        if (pill) pill.classList.add('show');
        if (group) group.style.display = 'inline-flex';
    });
    await sleep(500);

    // Click the pill to open the dropdown
    await page.evaluate(() => {
        const pill = document.getElementById('benchmark-pill');
        if (pill) pill.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
    });
    await sleep(800);

    await captureShot(page, 'tune-panel-open.png', { fullPage: true });
    await captureCloseUp(page, '#benchmark-dropdown-wrap', 'tune-panel-open.png', options);
}

// ── Benchmark Results ─────────────────────────────────────────────────────────
// Runs a real benchmark via the tune panel and captures the results view.
