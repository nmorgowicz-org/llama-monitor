// Scenario: model-discovery
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Only mock endpoints that don't depend on HF data (platform, VRAM, RAM, download-dir)
    await page.evaluate(() => {
        window.__captureRapidMlxAvailable = true;
        const originalFetch = window.fetch.bind(window);
        window.fetch = (input, init) => {
            const url = new URL(typeof input === 'string' ? input : input.url, location.href);
            if (url.pathname === '/api/hf/download-dir') {
                return Promise.resolve(new Response(JSON.stringify({ dir: '/models', configured: true }), {
                    status: 200, headers: { 'Content-Type': 'application/json' },
                }));
            }
            if (url.pathname === '/api/llama-binary/platform-info') {
                return Promise.resolve(new Response(JSON.stringify({
                    os: 'macos', arch: 'aarch64',
                    rapid_mlx_local_available: true,
                    rapid_mlx_local_requirement: 'Rapid-MLX local execution requires macOS on Apple Silicon',
                }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
            }
            if (url.pathname === '/api/gpu/vram') {
                return Promise.resolve(new Response(JSON.stringify({ total_bytes: 48 * 1024 ** 3, free_bytes: 32 * 1024 ** 3 }), {
                    status: 200, headers: { 'Content-Type': 'application/json' },
                }));
            }
            if (url.pathname === '/api/llama-binary/system-ram') {
                return Promise.resolve(new Response(JSON.stringify({ total_bytes: 64 * 1024 ** 3 }), {
                    status: 200, headers: { 'Content-Type': 'application/json' },
                }));
            }
            return originalFetch(input, init);
        };
    });

    // Open models modal and switch to Download tab
    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(800);

    const downloadTab = await page.$('.mm-tab[data-tab="download"]');
    if (downloadTab) {
        await downloadTab.click();
        await sleep(600);
    }

    // Ensure download tab content is visible
    await page.waitForSelector('.mm-tab-panel--download', { timeout: 5000 });
    await sleep(1000);

    // Wait for scope/sort controls to render
    await page.waitForSelector('.hf-scope-selector', { timeout: 8000 });

    // Search for Qwen models (will return real data via HF API)
    await page.type('#mm-hf-search-input', 'Qwen');
    await sleep(1500);
    // Phase 8B2: wait for group hierarchy (hf-search-group) instead of flat search results
    await page.waitForSelector('.hf-search-group', { timeout: 10000 });

    // Wait for identity/qualification badges to resolve from real APIs
    await sleep(2000);

    // Phase 8B2: Capture baseline discovery view (Phase 8B1 controls + Phase 8B2 group hierarchy)
    await captureShot(page, 'panels-model-discovery.png', { fullPage: true });

    // Expand first group to show variants with qualification badges
    const expandBtn = await page.$('.hf-sg-toggle');
    if (expandBtn) {
        await expandBtn.click();
        await sleep(1500); // Wait for MLX lineage/qual badges to load from real APIs
    }

    // Phase 8B2: Capture expanded group with qualification badges on variants
    await captureShot(page, 'panels-model-discovery-qualification-badges.png', { fullPage: true });

    // Phase 8B3: Switch to MLX-only to show MLX-native view (additive toggle test)
    // Default state is MLX+GGUF both active. Click GGUF to deselect it → MLX-only.
    const toggleResult = await page.evaluate(() => {
        const scopeContainer = document.getElementById('mm-hf-scope-container');
        const allWraps = document.querySelectorAll('.hf-scope-selector');
        const allBtns = document.querySelectorAll('.hf-scope-btn[data-scope-key="gguf"]');
        console.log('[DEBUG] before click:', {
            scopeWraps: allWraps.length,
            ggufBtns: allBtns.length,
            wrapParents: Array.from(allWraps).map(w => w.parentElement.id),
        });
        const ggufBtn = allBtns[allBtns.length - 1]; // Click LAST button (most recent wrap)
        if (!ggufBtn) return { found: false };
        ggufBtn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
        const afterWraps = document.querySelectorAll('.hf-scope-selector');
        const wrapDatasets = Array.from(afterWraps).map(w => ({ mlx: w.dataset.hfScopeMlx, gguf: w.dataset.hfScopeGguf }));
        return {
            found: true,
            mlxActive: document.querySelector('.hf-scope-btn[data-scope-key="mlx"]')?.classList.contains('hf-scope-btn--active'),
            ggufActive: document.querySelector('.hf-scope-btn[data-scope-key="gguf"]')?.classList.contains('hf-scope-btn--active'),
            containerMlx: scopeContainer ? scopeContainer.dataset.hfScopeMlx : 'N/A',
            containerGguf: scopeContainer ? scopeContainer.dataset.hfScopeGguf : 'N/A',
            wrapCount: afterWraps.length,
            wrapDatasets,
        };
    });
    console.log('[CAPTURE] MLX-only toggle result:', JSON.stringify(toggleResult));
    if (toggleResult.found) {
        await sleep(3000); // Wait for refilter with real results
        try {
            await page.waitForSelector('.hf-search-group', { timeout: 8000 });
        } catch {
            // No MLX-only results for this query — acceptable
        }
        await captureShot(page, 'panels-model-discovery-mlx-only.png', { fullPage: true });
    } else {
        console.log('[CAPTURE] GGUF scope button not found');
    }

    console.log('[CAPTURE] Scenario "model-discovery" complete.');
}
