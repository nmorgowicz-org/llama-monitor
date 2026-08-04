// Scenario: free-cache
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    const dialogState = await page.evaluate(async () => {
        try {
            const { confirmFreeCacheCleanup } = await import('/js/features/setup-view.js');
            void confirmFreeCacheCleanup(12.4 * 1024 ** 3);
            const dialog = document.querySelector('.app-confirm-dialog');
            const overlay = document.querySelector('.app-confirm-overlay');
            const rect = dialog?.getBoundingClientRect();
            return {
                shown: !!dialog,
                dialogDisplay: dialog ? getComputedStyle(dialog).display : null,
                dialogOpacity: dialog ? getComputedStyle(dialog).opacity : null,
                dialogWidth: rect?.width || 0,
                dialogHeight: rect?.height || 0,
                overlayDisplay: overlay ? getComputedStyle(overlay).display : null,
                overlayVisibility: overlay ? getComputedStyle(overlay).visibility : null,
            };
        } catch (error) {
            return { shown: false, error: error?.message || String(error) };
        }
    });
    if (!dialogState.shown) {
        throw new Error(`Free Cache confirmation did not open: ${dialogState.error || 'dialog missing'}`);
    }
    if (dialogState.overlayDisplay === 'none' || dialogState.overlayVisibility === 'hidden'
        || dialogState.dialogWidth === 0 || dialogState.dialogHeight === 0) {
        throw new Error(`Free Cache confirmation is hidden: ${JSON.stringify(dialogState)}`);
    }
    await sleep(250);
    await captureShot(page, 'welcome-free-cache-confirm.png', { fullPage: true });
    await page.evaluate(() => {
        document.documentElement.dataset.theme = 'light';
    });
    await sleep(200);
    await captureShot(page, 'welcome-free-cache-confirm-light.png', { fullPage: true });
}
