// Scenario: models
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
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
