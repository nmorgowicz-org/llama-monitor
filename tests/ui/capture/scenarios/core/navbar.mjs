// Scenario: navbar
// Extracted from tests/ui/capture.mjs (Phase A3).
import { join } from 'path';
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { ARTIFACTS_DIR, sleep } from '../../harness/paths.mjs';
import { captureElementScreenshot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    if (!options.closeUp) {
        console.log('[CAPTURE] navbar scenario gated to --close-up; skipping');
        return;
    }

    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch (e) {
            console.log('[CAPTURE] navbar: attach failed (non-fatal):', e.message);
        }
    }
    await sleep(1500);

    // 1. Connected / idle — navbar at rest (dark theme)
    const navEl = await page.$('.top-nav-bar');
    const navBox = await navEl.boundingBox();
    await page.screenshot({
        path: join(ARTIFACTS_DIR, 'navbar-idle-dark.png'),
        clip: { x: 0, y: navBox.y, width: navBox.width, height: navBox.height },
    });
    console.log('[CAPTURE] Saved navbar-idle-dark.png');

    if (options.closeUp) {
        await captureElementScreenshot(page, '.nav-monitoring-chip', 'navbar-sleep-pill-idle.png', { padding: 8 });
        await captureElementScreenshot(page, '.nav-theme-toggle', 'navbar-theme-toggle.png', { padding: 8 });
    }

    // 2. Low-power active state
    await page.evaluate(async () => {
        const token = window.__API_TOKEN ? `Bearer ${window.__API_TOKEN}` : '';
        await fetch('/api/sleep-mode/toggle', { method: 'POST', headers: { Authorization: token } });
    });
    await sleep(800);
    await page.screenshot({
        path: join(ARTIFACTS_DIR, 'navbar-low-power-active.png'),
        clip: { x: 0, y: navBox.y, width: navBox.width, height: navBox.height },
    });
    console.log('[CAPTURE] Saved navbar-low-power-active.png');

    if (options.closeUp) {
        await captureElementScreenshot(page, '.nav-monitoring-chip', 'navbar-sleep-pill-active.png', { padding: 8 });
    }

    // Restore to normal. Use the explicit /set endpoint (not another
    // /toggle call) since sleep-mode cycles through off -> logs-only ->
    // sleep rather than a simple on/off flip.
    await page.evaluate(async () => {
        const token = window.__API_TOKEN ? `Bearer ${window.__API_TOKEN}` : '';
        await fetch('/api/sleep-mode/set', {
            method: 'POST',
            headers: { Authorization: token, 'Content-Type': 'application/json' },
            body: JSON.stringify({ mode: 'off' }),
        });
    });
    await sleep(600);

    // 3. Light theme — click the theme toggle
    await page.click('#nav-theme-toggle');
    await sleep(400);
    await page.screenshot({
        path: join(ARTIFACTS_DIR, 'navbar-idle-light.png'),
        clip: { x: 0, y: navBox.y, width: navBox.width, height: navBox.height },
    });
    console.log('[CAPTURE] Saved navbar-idle-light.png');

    // Restore dark theme
    await page.click('#nav-theme-toggle');
    await sleep(300);
}
