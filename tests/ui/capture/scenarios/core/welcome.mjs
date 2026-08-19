// Scenario: welcome
// Extracted from tests/ui/capture.mjs (Phase A3).
import { captureAuthShell } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { DEFAULT_PORT, sleep } from '../../harness/paths.mjs';
import { findAvailablePort } from '../../harness/server.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    // Shot 1: the arrival screen with both cards visible.
    await captureShot(page, 'welcome-welcome.png', { fullPage: true });

    // Shot 2: verify the backend-specific Rapid-MLX attach fields in place.
    const backendSelect = await page.$('#setup-endpoint-backend');
    if (backendSelect) {
        await page.select('#setup-endpoint-backend', 'rapid_mlx');
        await sleep(200);
        await captureShot(page, 'welcome-rapid-mlx-attach.png', { fullPage: true });
        await page.select('#setup-endpoint-backend', 'llama_cpp');
    }

    // Shot 3: Open setup wizard and capture step 0 so the screenshots
    // tell a clear before→after story.
    const spawnBtn = await page.$('#setup-spawn-wizard-btn');
    if (spawnBtn) {
        try {
            await spawnBtn.click();
        } catch {
            // Fallback click via DOM when Puppeteer thinks it’s not clickable.
            await page.evaluate(() => {
                (document.getElementById('setup-spawn-wizard-btn')
                    || document.querySelector('#view-setup button:has-text("Setup wizard")'))
                    ?.click();
            });
        }
        await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 8000 }).catch(() => {
            console.log('[CAPTURE] Wizard overlay did not open; falling back to welcome shot');
        });
        // Hide the binary prereq banner for a clean wizard shot.
        await page.evaluate(() => {
            const banner = document.getElementById('wizard-binary-prereq');
            if (banner) banner.style.display = 'none';
        });
        await sleep(500);
        await captureShot(page, 'welcome-spawn-wizard-btn.png', { fullPage: true });
        // Close wizard before proceeding.
        await page.keyboard.press('Escape');
        await sleep(300);
    } else {
        console.log('[CAPTURE] #setup-spawn-wizard-btn not found; skipping wizard-open shot');
    }

    const authPort = await findAvailablePort(DEFAULT_PORT + 1);
    await captureAuthShell(authPort, options.viewport);
}
