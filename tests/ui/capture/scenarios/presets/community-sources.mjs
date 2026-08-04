// Scenario: community-sources
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await page.evaluate(async () => {
        const { openModelsModal } = await import('/js/features/models.js');
        openModelsModal();
    });
    await page.waitForSelector('#models-modal.open', { visible: true });
    await page.click('#models-modal .mm-tab[data-tab="sources"]');
    await page.waitForFunction(() => {
        const status = document.getElementById('mm-sources-status');
        return status && !status.textContent.includes('Loading');
    });
    await sleep(200);
    await captureShot(page, 'community-sources-list-dark.png', { fullPage: true });

    await page.click('#mm-sources-add');
    await page.waitForSelector('#mm-sources-editor:not([hidden])', { visible: true });
    await page.type('#mm-source-username', 'example-org');
    await page.type('#mm-source-display-name', 'Example Organization');
    await page.type('#mm-source-description', 'Capture fixture for a user-managed source.');
    await sleep(150);
    await captureShot(page, 'community-sources-editor-dark.png', { fullPage: true });

    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await captureShot(page, 'community-sources-editor-light.png', { fullPage: true });

    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await page.evaluate(() => {
        const panel = document.getElementById('mm-sources-panel');
        if (panel) panel.scrollTop = panel.scrollHeight;
    });
    await sleep(150);
    await captureShot(page, 'community-sources-editor-narrow.png', { fullPage: true });
}

// ── Core Chat ───────────────────────────────────────────────────────────────────
