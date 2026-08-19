// Scenario: dashboard
// Extracted from tests/ui/capture.mjs (Phase A3).
import { connectSource } from '../../harness/source.mjs';
import { gotoApp, switchTab } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureElementScreenshot, captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    await gotoApp(ctx.page, ctx.baseUrl);
    const source = await connectSource(ctx.page, options);
    try {
        await runDashboard(ctx, options);
    } finally {
        await source.teardown();
    }
}

async function runDashboard(ctx, options) {
    const { page, baseUrl } = ctx;

    await switchTab(page, 'server');
    // Wait for agent first poll (2s interval) + some render time.
    await sleep(3500);
    // Element-scoped capture: scrolls "Performance & metrics" to the top of frame and
    // clips to the inference section's own bounds, instead of a full-page shot (which
    // captures the whole scrollable document regardless of scroll position — using
    // captureShot's default fullPage:true here previously made this identical to
    // settings-server-tab.png).
    await captureElementScreenshot(page, '#inference-section', 'dashboard-performance-section.png', { padding: 0 });
    await captureShot(page, 'settings-server-tab.png', { fullPage: true });

    // Wait up to 6s for hardware data to arrive (remote agent dependent).
    const gpuVisible = await page.evaluate(() => new Promise(resolve => {
        const check = () => {
            const gpu = document.getElementById('gpu-section');
            const sys = document.getElementById('system-section');
            if ((gpu && gpu.style.display !== 'none') || (sys && sys.style.display !== 'none')) {
                resolve(true);
            }
        };
        check();
        const obs = new MutationObserver(check);
        obs.observe(document.body, { attributes: true, subtree: true, attributeFilter: ['style'] });
        setTimeout(() => { obs.disconnect(); resolve(false); }, 6000);
    }));

    if (gpuVisible) {
        await page.evaluate(() => {
            const gpu = document.getElementById('gpu-section') || document.getElementById('system-section');
            const page = gpu?.closest('.page') || document.querySelector('.page.active');
            if (page) page.scrollTop = gpu.offsetTop - 8;
        });
        await sleep(600);
    } else {
        console.log('[CAPTURE] Hardware section not visible; capturing at current scroll position.');
    }
    await captureShot(page, 'dashboard-gpu-section.png');
}
