// Scenario: appearance-palette
// Extracted from tests/ui/capture.mjs (Phase A3).
import { join } from 'path';
import { attachToServer } from '../../harness/attach.mjs';
import { loadAppDocument, openAppearancePaneForCapture } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await loadAppDocument(page, baseUrl);

    if (!options.noAttach) {
        try {
            await attachToServer(page);
        } catch (e) {
            console.log('[CAPTURE] appearance-palette: attach failed (non-fatal):', e.message);
        }
    }
    await sleep(500);

    await openAppearancePaneForCapture(page);
    await sleep(400);

    const paletteGridInfo = await page.evaluate(() => {
        const grid = document.getElementById('settings-palette-grid');
        if (!grid) return 'grid NOT FOUND';
        const btns = grid.querySelectorAll('.palette-swatch');
        return `grid found, ${btns.length} swatches: ` + [...btns].map(b => b.dataset.palette || '(empty)').join(', ');
    });
    console.log('[CAPTURE] DOM check:', paletteGridInfo);

    await captureShot(page, 'appearance-palette-carbon-mint.png', { fullPage: true });

    const palettes = [
        { value: 'cyber-rose', label: 'cyber-rose' },
        { value: 'solar-violet', label: 'solar-violet' },
        { value: 'lava-core', label: 'lava-core' },
    ];
    for (const { value, label } of palettes) {
        await page.evaluate((palette) => {
            const html = document.documentElement;
            html.classList.add('palette-changing');
            setTimeout(() => html.classList.remove('palette-changing'), 350);
            if (palette) html.dataset.palette = palette;
            else delete html.dataset.palette;
            document.querySelectorAll('#settings-palette-grid .palette-swatch').forEach(btn => {
                const active = (btn.dataset.palette || '') === palette;
                btn.classList.toggle('active', active);
                btn.setAttribute('aria-pressed', String(active));
            });
        }, value);
        console.log(`[CAPTURE] Palette ${label}`);
        await sleep(500);
        await captureShot(page, `appearance-palette-${label}.png`, { fullPage: true });
    }

    await page.evaluate(() => {
        delete document.documentElement.dataset.palette;
        document.querySelectorAll('#settings-palette-grid .palette-swatch').forEach(btn => {
            const active = (btn.dataset.palette || '') === '';
            btn.classList.toggle('active', active);
            btn.setAttribute('aria-pressed', String(active));
        });
    });
    await sleep(300);

    await page.evaluate(() => {
        document.getElementById('settings-modal-close')?.click();
        document.getElementById('settings-modal')?.classList.remove('open');
        const saved = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
        localStorage.setItem('llama-monitor-preferences', JSON.stringify({
            ...saved,
            theme: 'light',
            palette: '',
        }));
        delete document.documentElement.dataset.palette;
        document.documentElement.dataset.theme = 'light';
        document.querySelectorAll('.sidebar-btn').forEach(btn => {
            btn.classList.toggle('active', btn.dataset.tab === 'server');
        });
        document.querySelectorAll('.page').forEach(pageEl => {
            pageEl.classList.toggle('active', pageEl.id === 'page-server');
        });
    });
    await page.waitForSelector('#page-server.active', { timeout: 5000 });
    await sleep(700);
    await captureShot(page, 'appearance-light-dashboard.png', { fullPage: true });

    await page.evaluate(() => {
        const saved = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
        localStorage.setItem('llama-monitor-preferences', JSON.stringify({
            ...saved,
            theme: 'dark',
            palette: '',
        }));
        document.documentElement.dataset.theme = 'dark';
    });
    console.log('[CAPTURE] Scenario "appearance-palette" complete.');
}

// ── Rapid-MLX Live Runtime ────────────────────────────────────────────────────
// Developer-only scenario requiring rapid-mlx on PATH and a cached model.
// Does NOT run in CI. Validates end-to-end: spawn → health → telemetry → chat → stop.
