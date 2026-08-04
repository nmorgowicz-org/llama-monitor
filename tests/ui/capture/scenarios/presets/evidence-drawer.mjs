// Scenario: evidence-drawer
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    const evidence = {
        title: 'Rapid-MLX launch policy evidence',
        status: 'blocked',
        summary: 'Speculative decoding is not recommended for normal coding-agent traffic in the qualified runtime.',
        consequence: 'Sampled and constrained-tool requests fall through without MTP acceleration.',
        remediation: 'Leave speculative decoding off unless you are deliberately reproducing the greedy qualification lane.',
        evidence: [
            'rapid-mlx 0.11.1 bypassed MTP for nonzero-temperature requests.',
            'Normal constrained-tool requests showed no speculative activity.',
        ],
        adjustments: ['KV cache dtype: int4 → int8 because the Rapid reasoning-quality policy is always enabled.'],
        fallthroughs: ['Sampling or a logits processor disables MTP in the qualified build.'],
        warnings: ['The 95%+ user-impact figure is a product-owner estimate, not a population measurement.'],
        provenance: ['Runtime: rapid-mlx 0.11.1', 'Evidence status: measured qualification receipt'],
    };
    await page.evaluate(data => window.openEvidenceDrawer(data), evidence);
    await page.waitForSelector('#evidence-drawer.open', { visible: true });
    await sleep(200);
    await captureShot(page, 'evidence-drawer-dark.png', { fullPage: true });

    await page.click('.evidence-drawer-details > summary');
    await sleep(150);
    await captureShot(page, 'evidence-drawer-expanded-dark.png', { fullPage: true });

    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await captureShot(page, 'evidence-drawer-expanded-light.png', { fullPage: true });

    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(100);
    await captureShot(page, 'evidence-drawer-narrow-reduced-motion.png', { fullPage: true });
}
