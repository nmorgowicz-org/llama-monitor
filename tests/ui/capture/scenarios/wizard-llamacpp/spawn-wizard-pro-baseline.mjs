// Scenario: spawn-wizard-pro-baseline
// SCENARIO INTENT: Preserve the current explicit Pro-not-implemented state as a diagnostic baseline, not a golden Pro UI.
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function({ page, baseUrl }) {
    await gotoApp(page, baseUrl);
    await page.evaluate(async () => {
        sessionStorage.clear();
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
        const select = document.getElementById('view-mode-select');
        select.value = 'pro';
        select.dispatchEvent(new Event('change', { bubbles: true }));
    });
    await sleep(250);
    // INTENT: The view selector visibly says Pro while the current layout remains unimplemented.
    await captureShot(page, 'spawn-wizard-pro-baseline-not-implemented.png', { fullPage: true, expandSelector: '.wizard-body' });
}
