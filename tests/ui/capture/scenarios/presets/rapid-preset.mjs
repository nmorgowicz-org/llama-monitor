// Scenario: rapid-preset
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { DEFAULT_VIEWPORT, sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await page.waitForSelector('.launch-card[data-preset-id="capture-rapid-mlx"]', {
        visible: true,
        timeout: 10000,
    });
    await sleep(250);
    await captureShot(page, 'welcome-rapid-mlx-preset.png', { fullPage: true });

    await page.click('.launch-card[data-preset-id="capture-rapid-mlx"] .launch-card-btn-edit');
    await page.waitForSelector('#preset-modal.open.preset-editor--rapid-mlx', { visible: true });
    await page.waitForFunction(() => {
        const strip = document.getElementById('preset-vram-strip');
        const display = document.getElementById('preset-vram-display');
        return strip && getComputedStyle(strip).display !== 'none'
            && display && !display.textContent.includes('Estimating');
    }, { timeout: 10000 }).catch(() => {});
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-editor-model.png', { fullPage: true });

    // MLX-native editor: each capture name matches the visible section.
    await page.click('#preset-modal .preset-editor-nav [data-section="generation"]');
    await page.waitForSelector('.preset-editor-section[data-section="generation"].active');
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-editor-generation.png', { fullPage: true });

    await page.click('#preset-modal .preset-editor-nav [data-section="context"]');
    await page.waitForSelector('.preset-editor-section[data-section="context"].active');
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-editor-cache-performance.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-editor-cache-performance-light.png', { fullPage: true });

    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await page.evaluate(() => {
        const navBtn = document.querySelector('#preset-modal .preset-editor-nav [data-section="advanced"]');
        if (navBtn) navBtn.click();
    });
    await page.waitForFunction(
        () => document.querySelector('.preset-editor-section[data-section="advanced"].active'),
        { timeout: 5000 }
    );
    await sleep(250);
    await captureShot(page, 'rapid-mlx-preset-editor-server-safety.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(200);
    await captureShot(page, 'rapid-mlx-preset-editor-server-safety-light.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(200);
    await captureShot(page, 'rapid-mlx-preset-editor-server-safety-narrow.png', { fullPage: true });
    await page.setViewport(DEFAULT_VIEWPORT);

    await page.evaluate(() => {
        const companions = document.querySelector('[data-mlx-group="companions"]');
        if (companions) companions.open = true;
        const enabled = document.getElementById('modal-rapid-speculative-enabled');
        if (enabled && !enabled.checked) enabled.click();
        const content = document.querySelector('#preset-modal .preset-editor-content');
        if (content) content.scrollTop = content.scrollHeight;
    });
    await page.waitForFunction(() => {
        const display = document.getElementById('preset-vram-display');
        return display && !display.textContent.includes('Estimating');
    }, { timeout: 10000 });
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-server-safety-speculative-dark.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-server-safety-speculative-light.png', { fullPage: true });
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await page.evaluate(() => {
        const content = document.querySelector('#preset-modal .preset-editor-content');
        if (content) content.scrollTop = content.scrollHeight;
    });
    await sleep(150);
    await captureShot(page, 'rapid-mlx-preset-server-safety-speculative-narrow.png', { fullPage: true });
    await page.setViewport(DEFAULT_VIEWPORT);
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await page.evaluate(() => document.getElementById('preset-modal-close')?.click());
    await page.waitForSelector('#preset-modal.open', { hidden: true });

    // Open typed-source preset in editor (no redundant "before-edit" card screenshot — welcome-rapid-mlx-preset.png already shows cards).
    const typedCardState = await page.evaluate(() => {
        const card = document.querySelector('.launch-card[data-preset-id="capture-rapid-mlx-typed"]');
        if (!card) return null;
        card.scrollIntoView({ block: 'center' });
        return {
            id: card.dataset.presetId,
            name: card.querySelector('.launch-card-name')?.textContent?.trim() || '',
            model: card.querySelector('.launch-card-model')?.textContent?.trim() || '',
        };
    });
    if (!typedCardState) throw new Error('rapid-preset: typed source card was not rendered');
    await page.evaluate(() => {
        document.querySelector('.launch-card[data-preset-id="capture-rapid-mlx-typed"] .launch-card-btn-edit')?.click();
    });
    await page.waitForSelector('#preset-modal.open.preset-editor--rapid-mlx', { visible: true });
    await sleep(200);
    const openedTypedState = await page.evaluate(() => ({
        id: document.getElementById('modal-preset-id')?.value || '',
        name: document.getElementById('modal-name')?.value || '',
        model: document.getElementById('modal-model-path')?.value || '',
    }));
    console.log(`[rapid-preset] typed card requested ${typedCardState.id}; modal opened ${openedTypedState.id || '(none)'}`);
    await captureShot(page, 'rapid-mlx-typed-card-edit-result.png', { fullPage: true });
    await page.evaluate(() => document.getElementById('preset-modal-close')?.click());

    const promptState = await page.evaluate(async () => {
        const { showPromptDialog } = await import('/js/features/toast.js');
        void showPromptDialog(
            'Restore protected model',
            'Enter the API key for this session. It is used only for this launch and is not saved.',
            '',
            { type: 'password', confirmLabel: 'Restore' },
        );
        const input = document.querySelector('.modal-overlay .modal input[type="password"]');
        return { shown: !!input, inputType: input?.type || null };
    });
    if (!promptState.shown || promptState.inputType !== 'password') {
        throw new Error(`Protected restore modal did not open correctly: ${JSON.stringify(promptState)}`);
    }
    await sleep(250);
    await captureShot(page, 'welcome-rapid-mlx-restore-key.png', { fullPage: true });
    await page.evaluate(() => {
        document.documentElement.dataset.theme = 'light';
    });
    await sleep(200);
    await captureShot(page, 'welcome-rapid-mlx-restore-key-light.png', { fullPage: true });
}
