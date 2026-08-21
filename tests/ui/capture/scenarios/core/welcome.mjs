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

    // Shot 2: keep the notification center visible over setup content. This
    // catches regressions where the menu's child z-index is trapped beneath
    // the welcome view's stacking context.
    await page.evaluate(() => {
        const now = Date.now();
        localStorage.setItem('llama-monitor-notifications', JSON.stringify({
            active: [
                {
                    id: 'capture-warning',
                    type: 'warning',
                    title: 'Model cache needs attention',
                    message: 'Free cache before starting a larger model.',
                    createdAt: now - 120000,
                    updatedAt: now - 120000,
                    actions: [],
                },
                {
                    id: 'capture-info',
                    type: 'info',
                    title: 'Ready for local inference',
                    message: 'Choose a model to continue.',
                    createdAt: now - 60000,
                    updatedAt: now - 60000,
                    actions: [],
                },
            ],
            archived: [],
        }));
    });
    await page.reload({ waitUntil: 'networkidle0' });
    await page.waitForSelector('#nav-notifications-btn');
    await page.click('#nav-notifications-btn');
    await page.waitForFunction(() => (
        document.getElementById('nav-notifications-menu')?.hidden === false
    ));
    const notificationLayout = await page.evaluate(() => {
        const menu = document.getElementById('nav-notifications-menu');
        const nav = document.querySelector('.top-nav-bar');
        if (!menu || !nav) return null;
        const menuRect = menu.getBoundingClientRect();
        const menuStyle = getComputedStyle(menu);
        const navStyle = getComputedStyle(nav);
        return {
            display: menuStyle.display,
            hidden: menu.hidden,
            left: Math.round(menuRect.left),
            top: Math.round(menuRect.top),
            right: Math.round(menuRect.right),
            bottom: Math.round(menuRect.bottom),
            width: Math.round(menuRect.width),
            viewportWidth: window.innerWidth,
            visibility: menuStyle.visibility,
            opacity: menuStyle.opacity,
            pointerEvents: menuStyle.pointerEvents,
            navZIndex: navStyle.zIndex,
            paintedAtCenter: document.elementsFromPoint(
                Math.round(menuRect.left + menuRect.width / 2),
                Math.round(menuRect.top + menuRect.height / 2),
            ).slice(0, 5).map((element) => element.id || element.className || element.tagName),
        };
    });
    console.log(`[CAPTURE] Notification menu layout: ${JSON.stringify(notificationLayout)}`);
    const menuPainted = notificationLayout?.paintedAtCenter?.some((value) => (
        value === 'nav-notifications-menu' || value.includes('nav-notification')
    ));
    if (!notificationLayout
        || notificationLayout.display === 'none'
        || notificationLayout.width === 0
        || notificationLayout.right <= 0
        || notificationLayout.left >= notificationLayout.viewportWidth
        || !menuPainted) {
        throw new Error('Notification menu is not visible in the viewport');
    }
    await captureShot(page, 'welcome-notifications-menu.png', {
        fullPage: false,
        expandSelector: '#nav-notifications-menu',
    });
    await page.keyboard.press('Escape');
    await sleep(250);

    // Shot 3: verify the backend-specific Rapid-MLX attach fields in place.
    const backendSelect = await page.$('#setup-endpoint-backend');
    if (backendSelect) {
        await page.select('#setup-endpoint-backend', 'rapid_mlx');
        await sleep(200);
        await captureShot(page, 'welcome-rapid-mlx-attach.png', { fullPage: true });
        await page.select('#setup-endpoint-backend', 'llama_cpp');
    }

    // Shot 4: Open setup wizard and capture step 0 so the screenshots
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
