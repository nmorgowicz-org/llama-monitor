// Scenario: model-browser
// Intent-aware filesystem model picker from the Spawn Wizard.
import { loadAppDocument } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await page.setViewport({ width: 1280, height: 900, deviceScaleFactor: 1 });
    await loadAppDocument(page, baseUrl);

    await page.evaluate(() => {
        const originalFetch = window.fetch.bind(window);
        window.fetch = (input, init) => {
            const url = new URL(typeof input === 'string' ? input : input.url, window.location.origin);
            if (url.pathname === '/api/rapid-mlx/runtime/status') {
                return Promise.resolve(new Response(JSON.stringify({
                    runtime: { supported: true, active: { version: '0.10.10' } },
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            return originalFetch(input, init);
        };
    });

    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await page.waitForSelector('#spawn-browse-model-btn', { timeout: 5000 });

    await page.evaluate(() => document.querySelector('.wizard-engine-card[data-engine="rapid_mlx"]')?.click());
    await sleep(250);
    await page.click('#spawn-browse-model-btn');
    await page.waitForSelector('#file-browser-modal.open.file-browser-modal--model', { timeout: 8000 });
    await page.waitForSelector('#fb-model-toolbar:not([hidden])', { timeout: 8000 });
    await page.waitForFunction(
        () => document.getElementById('fb-model-intent')?.textContent.includes('Rapid-MLX'),
        { timeout: 5000 },
    );
    await sleep(400);
    await captureShot(page, 'model-browser-rapid-mlx.png', { fullPage: true, expandSelector: '.wizard-body' });

    await page.keyboard.press('Escape');
    await sleep(250);
    await page.evaluate(() => document.querySelector('.wizard-engine-card[data-engine="llama_cpp"]')?.click());
    await sleep(250);
    await page.click('#spawn-browse-model-btn');
    await page.waitForFunction(
        () => document.getElementById('fb-model-intent')?.textContent.includes('llama.cpp'),
        { timeout: 8000 },
    );
    await sleep(400);
    await captureShot(page, 'model-browser-llama-cpp.png', { fullPage: true, expandSelector: '.wizard-body' });
}
