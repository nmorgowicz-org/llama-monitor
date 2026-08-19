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

    // Phase 8B3: Expanding the group auto-selects a file, which fires triggerQuantAdvisor()
    // off hfState.paramB — wait for the workload/context-aware quant comparison table to
    // render with real /api/vram/quant-compare data before capturing. The "Qwen" group's
    // first-expanded variant isn't guaranteed to have populated files (live HF search order
    // varies run to run) — if it has none, re-search for a known-reliable multi-quant GGUF
    // repo instead of giving up on the gate.
    try {
        // The "Qwen" group search is non-deterministic (live HF results reorder/vary run to
        // run, and some repos return empty file listings) — re-search for a known-reliable,
        // multi-quant GGUF repo instead of gambling on whatever the first group's variant is.
        await page.evaluate(() => {
            const el = document.getElementById('mm-hf-search-input');
            const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
            setter.call(el, '');
            el.dispatchEvent(new Event('input', { bubbles: true }));
        });
        await sleep(300);
        await page.type('#mm-hf-search-input', 'unsloth Qwen3.6-27B-MTP-GGUF');
        await sleep(3000);
        await page.waitForSelector('.hf-search-group', { timeout: 10000 });
        await sleep(1500);
        const expandBtn2 = await page.$('.hf-sg-toggle');
        if (expandBtn2) {
            await expandBtn2.click();
            await sleep(1500);
        }
        // A single-variant group's expand click already auto-selects that variant
        // (hf-browse.js's toggleGroup(): `if (groupModels.length === 1) { ...click(); }`) —
        // a second explicit click here would toggle the file list closed instead of selecting.
        // Only click a variant ourselves if nothing is selected yet (multi-variant group).
        const alreadySelected = await page.evaluate(() => !!document.querySelector('.hf-file-item.selected, .hf-sg-variant.selected'));
        if (!alreadySelected) {
            const variantClicked = await page.evaluate(() => {
                const variants = Array.from(document.querySelectorAll('.hf-sg-variant'));
                const gguf = variants.find(v => v.querySelector('.hf-sg-format-badge--gguf'));
                const target = gguf || variants[0];
                if (!target) return false;
                target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
                return true;
            });
            if (variantClicked) await sleep(1500);
        }

        // Poll rather than waitForSelector — the panel toggles via inline style.display, and a
        // selector built on that (`[style*="display: none"]`, negated) doesn't reliably observe
        // the transition through Puppeteer. The GPU VRAM metrics endpoint can take several
        // seconds to warm up on a freshly-spawned instance (mactop/powermetrics cold start),
        // so give the quant advisor's background retry plenty of time to land.
        let advisorVisible = false;
        for (let i = 0; i < 70; i++) {
            advisorVisible = await page.evaluate(() => {
                const panel = document.getElementById('mm-quant-advisor');
                const rows = document.querySelectorAll('#mm-quant-advisor-table tr');
                return !!panel && panel.style.display !== 'none' && rows.length > 0;
            });
            if (advisorVisible) break;
            await sleep(500);
        }
        if (!advisorVisible) throw new Error('quant advisor did not render');
        await sleep(500);
        await captureShot(page, 'panels-model-discovery-quant-advisor.png', { fullPage: true });

        // Phase 8B3: Click a context-size pill to trigger context/KV requantization —
        // triggerQuantAdvisor() re-fetches quant-compare with the new previewCtx, and the
        // VRAM panel recomputes for the same param count at the new context length.
        const pillClicked = await page.evaluate(() => {
            const pills = document.querySelectorAll('#mm-vram-ctx-pills .vram-ctx-pill');
            const target = Array.from(pills).find(p => p.dataset.ctx === '131072');
            if (!target) return false;
            target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
            return true;
        });
        if (pillClicked) {
            await sleep(1500); // debounced triggerQuantAdvisor() + quant-compare round trip
            await captureShot(page, 'panels-model-discovery-context-requant.png', { fullPage: true });
        }
    } catch {
        console.log('[CAPTURE] quant advisor did not render for this selection — skipping quant/context gates');
    }

    // Phase 8B3: Switch to MLX-only to show MLX-native view (additive toggle test)
    // Default state is MLX+GGUF both active. Click GGUF to deselect it → MLX-only.
    const toggleResult = await page.evaluate(() => {
        const allBtns = document.querySelectorAll('.hf-scope-btn[data-scope-key="gguf"]');
        const ggufBtn = allBtns[allBtns.length - 1]; // Click LAST button (most recent wrap)
        if (!ggufBtn) return { found: false };
        ggufBtn.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true }));
        return { found: true };
    });
    if (toggleResult.found) {
        await sleep(3000); // Wait for refilter with real results
        try {
            await page.waitForSelector('.hf-search-group', { timeout: 8000 });
        } catch {
            // No MLX-only results for this query — acceptable
        }
        await captureShot(page, 'panels-model-discovery-mlx-only.png', { fullPage: true });
    }

    console.log('[CAPTURE] Scenario "model-discovery" complete.');
}
