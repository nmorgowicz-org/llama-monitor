// Scenario: sparkline
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureElementScreenshot, captureShot, captureSparklineClips } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await attachToServer(page);
    console.log('[CAPTURE] Waiting for metrics to populate...');
    await sleep(4000);
    await captureShot(page, 'sparkline-sparkline-validate-full.png', { fullPage: true });
    await captureElementScreenshot(page, '#gpu-section', 'sparkline-sparkline-validate-gpu-section.png', { padding: 24 });
    await captureElementScreenshot(page, '#system-section', 'sparkline-sparkline-validate-system-section.png', { padding: 24 });
    if (options.closeUp) {
        await captureSparklineClips(page, 'svg.metric-sparkline, svg.hw-sparkline, svg.hw-metric-sparkline, svg.hw-clock-footer-spark');
    }
}

// Animated capture flow for inference and hardware metric GIFs.
