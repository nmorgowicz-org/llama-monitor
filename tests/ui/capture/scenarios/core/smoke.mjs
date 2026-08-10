// Scenario: smoke
// Extracted from tests/ui/capture.mjs (Phase A3).
import { connectSource } from '../../harness/source.mjs';
import { gotoApp, waitForMonitor } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';

export default async function({ page, baseUrl }, options) {
    await runSmoke({ page, baseUrl }, options);
}

async function runSmoke({ page, baseUrl }, options) {
    const criticalPatterns = [
        'import',
        'Cannot set properties of (null|undefined)',
        'is not defined',
        'TypeError',
        'SyntaxError',
        'Failed to fetch module',
        'Failed to load module script',
    ];

    const logs = {
        error: [],
        warn: [],
    };

    page.on('console', msg => {
        const level = msg.type();
        const text = msg.text();
        if (level === 'error') logs.error.push(text);
        if (level === 'warn') logs.warn.push(text);
    });

    page.on('pageerror', err => {
        logs.error.push(err.message || String(err));
    });

    await gotoApp(page, baseUrl);
    const source = options.noAttach ? null : await connectSource(page, options);

    if (options.noAttach) {
        // attachToServer normally triggers the setup->monitor view
        // transition; force it directly since we're skipping attach.
        await page.evaluate(async () => {
            const { ensureMonitorView } = await import('/js/features/setup-view.js');
            ensureMonitorView();
        });
        await waitForMonitor(page);
    }

    await sleep(2000);

    const hasCritical = logs.error.some(line =>
        criticalPatterns.some(p => line.includes(p))
    );

    console.log('[SMOKE] Console warnings:', logs.warn.length);
    console.log('[SMOKE] Console errors:', logs.error.length);

    if (logs.error.length > 0) {
        console.log('[SMOKE] Error details:');
        logs.error.forEach(e => console.log('  -', e));
    }

    if (hasCritical) {
        throw new Error(
            'SMOKE FAIL: critical console errors detected on startup. ' +
            'Check for import/export issues, missing symbols, or runtime failures.'
        );
    }

    console.log('[SMOKE] PASS: no critical console errors on startup.');
    await source?.teardown();
}

// ── Setup wizard ────────────────────────────────────────────────────────────────
// Step 1 profile selection, step 2 HF source with discover pills / community
// picks / quant advisor, step 3 VRAM panel. Does not require a remote attach.
