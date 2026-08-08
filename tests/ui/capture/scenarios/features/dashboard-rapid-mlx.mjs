// Scenario: dashboard-rapid-mlx
// Extracted from tests/ui/capture.mjs (Phase A3).
import { join } from 'path';
import { gotoApp } from '../../harness/browser.mjs';
import { DEFAULT_VIEWPORT, sleep } from '../../harness/paths.mjs';
import { captureElementScreenshot, captureShot } from '../../harness/shot.mjs';

export default async function(ctx) {
    const { page, baseUrl } = ctx;
    console.log('[CAPTURE] dashboard-rapid-mlx: using DETERMINISTIC FAKE telemetry via renderRapidMlxCards(). ' +
        'This tests card rendering logic, NOT real runtime behavior.');
    await gotoApp(page, baseUrl);
    await page.evaluate(async () => {
        const { switchView } = await import('/js/features/setup-view.js');
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        switchView('monitor');
        document.querySelectorAll('.sidebar-btn').forEach(button => {
            button.classList.toggle('active', button.dataset.tab === 'server');
        });
        document.querySelectorAll('.page').forEach(pageElement => {
            pageElement.classList.toggle('active', pageElement.id === 'page-server');
        });
        // Render a short synthetic poll sequence so throughput sparklines/peak badges
        // (which need >=2 samples of session history) are populated for the still capture,
        // instead of showing their empty state as they would after a single render.
        const samples = [
            { prompt: 640.1, gen: 30.2 },
            { prompt: 780.5, gen: 35.9 },
            { prompt: 812.4, gen: 38.7 },
        ];
        samples.forEach((sampleValues, index) => {
            renderRapidMlxCards({
                health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 3723,
                prompt_tokens_per_second: sampleValues.prompt, generation_tokens_per_second: sampleValues.gen,
                running_requests: 1, waiting_requests: 0,
                active_memory_bytes: 12884901888, peak_memory_bytes: 15032385536, cache_memory_bytes: 536870912,
                global_cache_hit_rate: 0.82, global_cache_entries: 184,
                cache_metrics: { hit_rate: 0.82, entry_count: 184, current_memory_bytes: 536870912 },
                completed_requests_total: 247, prompt_tokens_total: 182430, completion_tokens_total: 58420,
            }, index + 1, false, 'capture-rapid');
        });
        const ids = [...document.querySelectorAll('#rapid-mlx-card-grid [data-card-id]')].map(card => card.dataset.cardId);
        const expected = ['runtime', 'throughput', 'queue', 'memory', 'cache', 'totals'];
        if (JSON.stringify(ids) !== JSON.stringify(expected)) throw new Error('Unexpected full Rapid card composition: ' + ids.join(','));
    });
    // Allow the shared premium card entrance sequence to settle before the dark still.
    await sleep(1200);
    await captureElementScreenshot(page, '#inference-section', 'dashboard-rapid-mlx-dark.png', { padding: 24 });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(300);
    await captureElementScreenshot(page, '#inference-section', 'dashboard-rapid-mlx-light.png', { padding: 24 });
    await page.evaluate(async () => {
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        document.documentElement.dataset.theme = 'dark';
        renderRapidMlxCards({
            health: 'Ok', ready: true, model: 'mlx-community/Small-4bit',
            running_requests: 0, waiting_requests: 0,
            active_memory_bytes: 1073741824,
        }, 1, false, 'capture-rapid-partial');
        const ids = [...document.querySelectorAll('#rapid-mlx-card-grid [data-card-id]')].map(card => card.dataset.cardId);
        const expected = ['runtime', 'queue', 'memory'];
        if (JSON.stringify(ids) !== JSON.stringify(expected)) throw new Error('Unexpected partial Rapid card composition: ' + ids.join(','));
    });
    await sleep(300);
    await captureElementScreenshot(page, '#inference-section', 'dashboard-rapid-mlx-partial.png', { padding: 24 });

    // DOM stability + accessibility: re-render the same card set and confirm nodes are
    // patched in place (identity preserved, per the in-place-patching contract) and each
    // card exposes an aria-labelledby wired to its own heading id.
    await page.evaluate(async () => {
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        const sample = {
            health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 3800,
            prompt_tokens_per_second: 820.1, generation_tokens_per_second: 39.0,
            running_requests: 1, waiting_requests: 0,
            active_memory_bytes: 12884901888, peak_memory_bytes: 15032385536, cache_memory_bytes: 536870912,
            global_cache_hit_rate: 0.82, global_cache_entries: 184,
            cache_metrics: { hit_rate: 0.82, entry_count: 184, current_memory_bytes: 536870912 },
            completed_requests_total: 248, prompt_tokens_total: 182600, completion_tokens_total: 58500,
        };
        renderRapidMlxCards(sample, 10, false, 'capture-rapid-stability');
        const before = new Map([...document.querySelectorAll('#rapid-mlx-card-grid [data-card-id]')].map(el => [el.dataset.cardId, el]));
        renderRapidMlxCards({ ...sample, completed_requests_total: 249 }, 11, false, 'capture-rapid-stability');
        const after = [...document.querySelectorAll('#rapid-mlx-card-grid [data-card-id]')];
        for (const el of after) {
            if (before.get(el.dataset.cardId) !== el) {
                throw new Error('Card node identity not preserved across re-render for id=' + el.dataset.cardId);
            }
            const heading = el.querySelector('.widget-metric-label');
            if (!heading || el.getAttribute('aria-labelledby') !== heading.id) {
                throw new Error('Card missing aria-labelledby wiring for id=' + el.dataset.cardId);
            }
        }
    });
    console.log('[CAPTURE] dashboard-rapid-mlx: DOM identity + aria-labelledby check passed.');

    // Zero/no-data state: cache hit rate ring must render its dashed "no data yet" state
    // (not a filled 0% ring, which would be visually indistinguishable from "no lookups yet").
    await page.evaluate(async () => {
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        renderRapidMlxCards({
            health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 10,
            running_requests: 0, waiting_requests: 0,
            cache_metrics: { hit_rate: 0.0, entry_count: 0, current_memory_bytes: 0 },
        }, 1, false, 'capture-rapid-zero');
        const ring = document.querySelector('[data-card-id="cache"] .rapid-ring-row');
        if (ring && !ring.classList.contains('is-no-data')) {
            throw new Error('Cache ring should show "no data yet" state when hits+misses=0');
        }
    });
    console.log('[CAPTURE] dashboard-rapid-mlx: zero/no-data cache state check passed.');

    // Stale state: a failed poll on an already-rendered card set must mark it stale rather
    // than reverting to an empty/unavailable card.
    await page.evaluate(async () => {
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        renderRapidMlxCards({
            health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 3800,
        }, 1, false, 'capture-rapid-stale');
        renderRapidMlxCards({
            health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 3800,
        }, 2, true, 'capture-rapid-stale');
        const runtimeCard = document.querySelector('[data-card-id="runtime"]');
        if (!runtimeCard || !runtimeCard.classList.contains('is-stale')) {
            throw new Error('Runtime card should be marked is-stale after a failed poll');
        }
    });
    await sleep(200);
    await captureElementScreenshot(page, '#inference-section', 'dashboard-rapid-mlx-stale.png', { padding: 24 });
    console.log('[CAPTURE] dashboard-rapid-mlx: stale-poll state check passed.');

    // Narrow viewport + reduced motion, restoring the full card set for a readable still.
    await page.evaluate(async () => {
        const { renderRapidMlxCards } = await import('/js/features/rapid-mlx-cards.js');
        renderRapidMlxCards({
            health: 'Ok', ready: true, model: 'mlx-community/Qwen3-30B-A3B-4bit', uptime_seconds: 3800,
            prompt_tokens_per_second: 820.1, generation_tokens_per_second: 39.0,
            running_requests: 1, waiting_requests: 0,
            active_memory_bytes: 12884901888, peak_memory_bytes: 15032385536, cache_memory_bytes: 536870912,
            global_cache_hit_rate: 0.82, global_cache_entries: 184,
            cache_metrics: { hit_rate: 0.82, entry_count: 184, current_memory_bytes: 536870912 },
            completed_requests_total: 248, prompt_tokens_total: 182600, completion_tokens_total: 58500,
        }, 20, false, 'capture-rapid-narrow');
    });
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(250);
    await captureShot(page, 'dashboard-rapid-mlx-narrow-reduced-motion.png', { fullPage: true });
    await page.setViewport(DEFAULT_VIEWPORT);
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);
}

// Validation pass for sparkline layouts and clipped section captures.
// The individual SVG clip captures are only useful for debugging sparkline rendering;
// require --close-up to generate them.
